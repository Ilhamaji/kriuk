//! Acoustic echo cancellation for the mic path.
//!
//! Partitioned block frequency-domain adaptive filter (PBFDAF): the reference —
//! what the speakers actually play — is estimated and subtracted from the mic
//! before denoising. 4 partitions × 480 samples ≈ 40 ms of echo path, which
//! covers the direct path plus early reflections of a laptop speaker.
//!
//! Cost is a handful of 960-point FFTs per 10 ms block, and nothing at all
//! while the feature is off: `process` is never called.

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// One block of audio: 10 ms at 48 kHz, same as the denoiser frame.
pub const BLOCK: usize = 480;
const N: usize = BLOCK * 2;
const BINS: usize = BLOCK + 1;
const PARTS: usize = 4;
const MU: f32 = 0.4;
/// Leaky weights keep the filter from drifting during long silence.
const LEAK: f32 = 0.9995;
const EPS: f32 = 1e-6;

pub struct EchoCanceller {
    fwd: Arc<dyn Fft<f32>>,
    inv: Arc<dyn Fft<f32>>,
    /// Filter spectra, PARTS × BINS.
    weights: Vec<Complex32>,
    /// Reference spectra history, PARTS × BINS.
    history: Vec<Complex32>,
    /// Previous reference block, for overlap-save.
    prev: Vec<f32>,
    /// Newest reference spectrum.
    current: Vec<Complex32>,
    error: Vec<Complex32>,
    a: Vec<Complex32>,
    b: Vec<Complex32>,
    power: f32,
}

impl EchoCanceller {
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        Self {
            fwd: planner.plan_fft_forward(N),
            inv: planner.plan_fft_inverse(N),
            weights: vec![Complex32::default(); PARTS * BINS],
            history: vec![Complex32::default(); PARTS * BINS],
            prev: vec![0.0; BLOCK],
            current: vec![Complex32::default(); BINS],
            error: vec![Complex32::default(); BINS],
            a: vec![Complex32::default(); N],
            b: vec![Complex32::default(); N],
            power: 0.0,
        }
    }

    /// Drops the learned filter without reallocating: the caller uses this when a
    /// block comes back out of range and the filter has to start over. Clearing
    /// in place keeps the audio callback allocation-free.
    pub fn reset(&mut self) {
        self.weights.fill(Complex32::default());
        self.history.fill(Complex32::default());
        self.prev.fill(0.0);
        self.current.fill(Complex32::default());
        self.error.fill(Complex32::default());
        self.a.fill(Complex32::default());
        self.b.fill(Complex32::default());
        self.power = 0.0;
    }

    /// `reference` is what the speaker plays, `mic` is cleaned in place.
    pub fn process(&mut self, reference: &[f32], mic: &mut [f32]) {
        debug_assert_eq!(reference.len(), BLOCK);
        debug_assert_eq!(mic.len(), BLOCK);

        for i in 0..BLOCK {
            self.a[i] = Complex32::new(self.prev[i], 0.0);
            self.a[BLOCK + i] = Complex32::new(reference[i], 0.0);
        }
        self.prev.copy_from_slice(reference);
        self.fwd.process(&mut self.a);
        self.current.copy_from_slice(&self.a[..BINS]);

        let mut power = 0.0f32;
        for bin in self.current.iter() {
            power += bin.norm_sqr();
        }
        power /= BINS as f32;
        self.power = 0.9 * self.power + 0.1 * power;

        // No reference energy means nothing to cancel: skip the whole filter so
        // silence costs a single branch per block.
        if self.power < 1e-8 {
            self.history.fill(Complex32::default());
            return;
        }

        // The current block is the newest entry, so a filter tap at lag 0 lines
        // up with the block we are cleaning. Shifting after the estimate would
        // leave the canceller a block behind the echo.
        for part in (1..PARTS).rev() {
            self.history
                .copy_within((part - 1) * BINS..part * BINS, part * BINS);
        }
        self.history[..BINS].copy_from_slice(&self.current);

        // Echo estimate: sum of the partitioned reference × filter spectra.
        self.b.fill(Complex32::default());
        for part in 0..PARTS {
            let base = part * BINS;
            for bin in 0..BINS {
                self.b[bin] += self.history[base + bin] * self.weights[base + bin];
            }
        }
        for i in 0..N {
            self.a[i] = self.b[i];
        }
        self.inv.process(&mut self.a);
        let scale = 1.0 / N as f32;
        let mut mic_peak = 0f32;
        for i in 0..BLOCK {
            mic_peak = mic_peak.max(mic[i].abs());
            mic[i] -= self.a[BLOCK + i].re * scale;
        }

        // Double talk (Geigel): a voice peak far above anything the speaker can
        // leak means the residual says nothing about the filter, so keep the
        // weights instead of learning from the speaker's own voice.
        let ref_peak = reference.iter().fold(0f32, |m, s| m.max(s.abs()));
        if mic_peak > 2.0 * ref_peak {
            return;
        }

        // Error spectrum, zero padded to the front.
        for i in 0..BLOCK {
            self.a[i] = Complex32::default();
            self.a[BLOCK + i] = Complex32::new(mic[i], 0.0);
        }
        self.fwd.process(&mut self.a);
        self.error.copy_from_slice(&self.a[..BINS]);

        // Constrained gradient update: project the correlation back to a
        // linear filter before applying it, or the tail wraps around and the
        // filter diverges.
        let step = MU / (self.power * PARTS as f32 + EPS) / N as f32;
        let inv = Arc::clone(&self.inv);
        let fwd = Arc::clone(&self.fwd);
        for part in 0..PARTS {
            let base = part * BINS;
            for bin in 0..BINS {
                self.b[bin] = self.history[base + bin].conj() * self.error[bin];
            }
            self.a.copy_from_slice(&self.b);
            inv.process(&mut self.a);
            for i in BLOCK..N {
                self.a[i] = Complex32::default();
            }
            fwd.process(&mut self.a);
            for bin in 0..BINS {
                self.weights[base + bin] =
                    self.weights[base + bin] * LEAK + self.a[bin] * step;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct Noise(u64);

    impl Noise {
        fn next(&mut self) -> f32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 40) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
        }
    }

    fn erle_for(delay: usize, blocks: usize) -> f32 {
        let mut noise = Noise(7);
        let mut aec = EchoCanceller::new();
        let mut line: VecDeque<f32> = VecDeque::from(vec![0.0; delay]);
        let (mut before, mut after) = (0f64, 0f64);

        for block in 0..blocks {
            let mut reference = [0f32; BLOCK];
            for s in reference.iter_mut() {
                *s = noise.next() * 0.3;
            }
            let mut mic = [0f32; BLOCK];
            for i in 0..BLOCK {
                if delay == 0 {
                    mic[i] = 0.5 * reference[i];
                } else {
                    mic[i] = 0.5 * line.pop_front().unwrap();
                    line.push_back(reference[i]);
                }
            }

            let dry: f64 = mic.iter().map(|s| (s * s) as f64).sum();
            aec.process(&reference, &mut mic);
            let wet: f64 = mic.iter().map(|s| (s * s) as f64).sum();

            if block >= blocks / 2 {
                before += dry;
                after += wet;
            }
        }
        (10.0 * (before / after).log10()) as f32
    }

    #[test]
    fn cancels_a_delayed_echo() {
        let erle = erle_for(700, 400);
        assert!(erle > 10.0, "echo reduction only {erle:.1} dB");
    }

    /// Every partition has to be usable, not just block-aligned delays.
    #[test]
    fn handles_each_partition() {
        for delay in [0usize, 200, 480, 700, 1_400, 1_900] {
            let erle = erle_for(delay, 400);
            assert!(erle > 10.0, "delay {delay}: only {erle:.1} dB");
        }
    }

    /// Talking over the echo must not wreck the filter or swallow the voice.
    #[test]
    fn double_talk_keeps_the_local_voice() {
        let mut noise = Noise(11);
        let mut aec = EchoCanceller::new();
        let mut line: VecDeque<f32> = VecDeque::from(vec![0.0; 700]);
        let mut reference = [0f32; BLOCK];

        for _ in 0..200 {
            for s in reference.iter_mut() {
                *s = noise.next() * 0.3;
            }
            let mut mic = [0f32; BLOCK];
            for i in 0..BLOCK {
                mic[i] = 0.5 * line.pop_front().unwrap();
                line.push_back(reference[i]);
            }
            aec.process(&reference, &mut mic);
        }

        let (mut local, mut out) = (0f64, 0f64);
        for _ in 0..50 {
            for s in reference.iter_mut() {
                *s = noise.next() * 0.3;
            }
            let mut mic = [0f32; BLOCK];
            for i in 0..BLOCK {
                let voice = noise.next() * 0.9;
                local += (voice * voice) as f64;
                mic[i] = 0.5 * line.pop_front().unwrap() + voice;
                line.push_back(reference[i]);
            }
            aec.process(&reference, &mut mic);
            for s in mic.iter() {
                assert!(s.is_finite());
                out += (*s * *s) as f64;
            }
        }

        assert!(out > 0.5 * local, "local voice lost: {out:.2} vs {local:.2}");
    }

    #[test]
    fn silence_stays_silent() {        let mut aec = EchoCanceller::new();
        let reference = [0f32; BLOCK];
        let mut mic = [0f32; BLOCK];
        aec.process(&reference, &mut mic);
        assert!(mic.iter().all(|s| s.abs() < 1e-6));
    }

    /// The canceller must stay bounded whatever the input level; an amplitude the
    /// unit tests never covered is how a diverging filter would reach the denoiser.
    #[test]
    fn stays_bounded_across_amplitudes() {
        for amp in [0.02f32, 0.1, 0.3, 1.0, 3.0] {
            let mut noise = Noise(3);
            let mut aec = EchoCanceller::new();
            let mut line: VecDeque<f32> = VecDeque::from(vec![0.0; 500]);
            let mut peak = 0f32;
            for _ in 0..600 {
                let mut reference = [0f32; BLOCK];
                for s in reference.iter_mut() {
                    *s = noise.next() * amp;
                }
                let mut mic = [0f32; BLOCK];
                for i in 0..BLOCK {
                    let echo = 0.8 * line.pop_front().unwrap();
                    let local = noise.next() * amp * 0.1;
                    mic[i] = echo + local;
                    line.push_back(reference[i]);
                }
                aec.process(&reference, &mut mic);
                for s in mic.iter() {
                    assert!(s.is_finite(), "amp {amp}: non-finite output");
                    peak = peak.max(s.abs());
                }
            }
            assert!(peak < 10.0, "amp {amp}: output peaked at {peak}");
        }
    }

    /// A reset must give back a clean filter, since that is what the callback
    /// relies on when a block comes back out of range.
    #[test]
    fn reset_clears_the_learned_filter() {
        let mut noise = Noise(5);
        let mut aec = EchoCanceller::new();
        let mut reference = [0f32; BLOCK];
        for _ in 0..50 {
            for s in reference.iter_mut() {
                *s = noise.next() * 0.3;
            }
            let mut mic = [0f32; BLOCK];
            aec.process(&reference, &mut mic);
        }
        aec.reset();
        let silent = [0f32; BLOCK];
        let mut mic = [0f32; BLOCK];
        aec.process(&silent, &mut mic);
        assert!(mic.iter().all(|s| s.abs() < 1e-6));
    }
}
