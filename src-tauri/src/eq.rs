//! Voice equalizer.
//!
//! Three fixed presets plus a slow "auto" curve that measures the long-term
//! band balance of the incoming voice and tilts it towards a speech target.
//! Everything runs at 48 kHz on the audio thread; preset 0 is a true bypass, so
//! the neutral setting costs one atomic load per block.

const SR: f32 = 48_000.0;

pub const NEUTRAL: u8 = 0;
pub const CLEAR: u8 = 1;
pub const WARM: u8 = 2;
pub const AUTO: u8 = 3;

/// Auto mode looks at ~500 ms of audio before moving the curve.
const AUTO_BLOCKS: u32 = 50;

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// Coefficients are stored pre-divided by a0, so `process` stays 5 multiplies.
    fn set(&mut self, b: [f32; 3], a: [f32; 3]) {
        let a0 = a[0];
        self.b0 = b[0] / a0;
        self.b1 = b[1] / a0;
        self.b2 = b[2] / a0;
        self.a1 = a[1] / a0;
        self.a2 = a[2] / a0;
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Retune without clearing the state, so changing the curve does not click.
    fn retune(&mut self, b: [f32; 3], a: [f32; 3]) {
        let (x1, x2, y1, y2) = (self.x1, self.x2, self.y1, self.y2);
        self.set(b, a);
        self.x1 = x1;
        self.x2 = x2;
        self.y1 = y1;
        self.y2 = y2;
    }

    fn peaking(freq: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * std::f32::consts::PI * freq / SR;
        let alpha = w.sin() / (2.0 * q);
        let cos = w.cos();
        let mut out = Self::default();
        out.set(
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        );
        out
    }

    fn high_pass(freq: f32, q: f32) -> Self {
        let w = 2.0 * std::f32::consts::PI * freq / SR;
        let alpha = w.sin() / (2.0 * q);
        let cos = w.cos();
        let mut out = Self::default();
        out.set(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        );
        out
    }

    fn low_shelf(freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * std::f32::consts::PI * freq / SR;
        let cos = w.cos();
        let alpha = w.sin() / 2.0 * std::f32::consts::SQRT_2;
        let sqrt_a = a.sqrt();
        let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
        let mut out = Self::default();
        out.set(
            [
                a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha),
            ],
            [
                (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha,
            ],
        );
        out
    }

    fn high_shelf(freq: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * std::f32::consts::PI * freq / SR;
        let cos = w.cos();
        let alpha = w.sin() / 2.0 * std::f32::consts::SQRT_2;
        let sqrt_a = a.sqrt();
        let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
        let mut out = Self::default();
        out.set(
            [
                a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha),
            ],
            [
                (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha,
            ],
        );
        out
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// One-pole helpers for the auto analyser.
struct OnePole {
    a: f32,
    y: f32,
}

impl OnePole {
    fn low(freq: f32) -> Self {
        let a = 1.0 - (-2.0 * std::f32::consts::PI * freq / SR).exp();
        Self { a, y: 0.0 }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

pub struct Eq {
    preset: u8,
    stages: [Biquad; 3],
    count: usize,
    low: OnePole,
    high: OnePole,
    low_energy: f32,
    mid_energy: f32,
    high_energy: f32,
    blocks: u32,
    mud_db: f32,
    presence_db: f32,
}

impl Eq {
    pub fn new() -> Self {
        Self {
            preset: NEUTRAL,
            stages: [Biquad::default(); 3],
            count: 0,
            low: OnePole::low(300.0),
            high: OnePole::low(3_000.0),
            low_energy: 0.0,
            mid_energy: 0.0,
            high_energy: 0.0,
            blocks: 0,
            mud_db: 0.0,
            presence_db: 0.0,
        }
    }

    pub fn set_preset(&mut self, preset: u8) {
        if preset == self.preset {
            return;
        }
        self.preset = preset;
        self.mud_db = 0.0;
        self.presence_db = 0.0;
        self.blocks = 0;
        match preset {
            CLEAR => {
                self.stages[0] = Biquad::high_pass(100.0, 0.707);
                self.stages[1] = Biquad::peaking(320.0, 1.0, -2.5);
                self.stages[2] = Biquad::peaking(3_200.0, 0.9, 3.5);
                self.count = 3;
            }
            WARM => {
                self.stages[0] = Biquad::high_pass(70.0, 0.707);
                self.stages[1] = Biquad::low_shelf(220.0, 2.5);
                self.stages[2] = Biquad::high_shelf(6_000.0, -2.0);
                self.count = 3;
            }
            AUTO => {
                self.stages[0] = Biquad::high_pass(100.0, 0.707);
                self.stages[1] = Biquad::peaking(320.0, 1.0, 0.0);
                self.stages[2] = Biquad::peaking(3_000.0, 0.9, 0.0);
                self.count = 3;
            }
            _ => {
                self.count = 0;
            }
        }
    }

    pub fn process(&mut self, block: &mut [f32]) {
        if self.count == 0 {
            return;
        }
        if self.preset == AUTO {
            self.analyze(block);
        }
        for stage in self.stages[..self.count].iter_mut() {
            for sample in block.iter_mut() {
                *sample = stage.process(*sample);
            }
        }
    }

    /// Splits the block into rough low/mid/high energy and tilts the curve.
    /// Deliberately slow and clamped: it should never sound like it is hunting.
    fn analyze(&mut self, block: &[f32]) {
        for sample in block {
            let low = self.low.process(*sample);
            let high = *sample - self.high.process(*sample);
            let mid = *sample - low - high;
            self.low_energy += low * low;
            self.mid_energy += mid * mid;
            self.high_energy += high * high;
        }
        self.blocks += 1;
        if self.blocks < AUTO_BLOCKS {
            return;
        }
        self.blocks = 0;

        let total = self.low_energy + self.mid_energy + self.high_energy + 1e-9;
        let low_share = self.low_energy / total;
        let high_share = self.high_energy / total;
        // Speech sits around a quarter of the energy below 300 Hz and a fifth
        // above 3 kHz; push gently towards that.
        let mud = ((0.25 - low_share) * 40.0).clamp(-4.0, 3.0);
        let presence = ((0.20 - high_share) * 40.0).clamp(-3.0, 5.0);

        self.mud_db = 0.8 * self.mud_db + 0.2 * mud;
        self.presence_db = 0.8 * self.presence_db + 0.2 * presence;

        let mud_stage = Biquad::peaking(320.0, 1.0, self.mud_db);
        let presence_stage = Biquad::peaking(3_000.0, 0.9, self.presence_db);
        self.stages[1].retune(
            [mud_stage.b0, mud_stage.b1, mud_stage.b2],
            [1.0, mud_stage.a1, mud_stage.a2],
        );
        self.stages[2].retune(
            [presence_stage.b0, presence_stage.b1, presence_stage.b2],
            [1.0, presence_stage.a1, presence_stage.a2],
        );

        self.low_energy = 0.0;
        self.mid_energy = 0.0;
        self.high_energy = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn energy_at(freq: f32, preset: u8, with_eq: bool) -> f32 {
        let mut eq = Eq::new();
        eq.set_preset(preset);
        let mut block = vec![0f32; 4_800];
        for (i, s) in block.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * freq * i as f32 / SR).sin() * 0.25;
        }
        if with_eq {
            eq.process(&mut block);
        }
        block[2_400..].iter().map(|s| s * s).sum()
    }

    #[test]
    fn neutral_is_bypass() {
        let plain = energy_at(1_000.0, NEUTRAL, false);
        let neutral = energy_at(1_000.0, NEUTRAL, true);
        assert!((plain - neutral).abs() < 1e-6);
    }

    #[test]
    fn clear_boosts_presence_and_cuts_mud() {
        let presence = energy_at(3_200.0, CLEAR, true) / energy_at(3_200.0, CLEAR, false);
        let mud = energy_at(320.0, CLEAR, true) / energy_at(320.0, CLEAR, false);
        assert!(presence > 1.6, "presence only x{presence:.2}");
        assert!(mud < 0.75, "mud only x{mud:.2}");
    }

    #[test]
    fn warm_lifts_lows_and_drops_highs() {
        let low = energy_at(150.0, WARM, true) / energy_at(150.0, WARM, false);
        let high = energy_at(9_000.0, WARM, true) / energy_at(9_000.0, WARM, false);
        assert!(low > 1.2, "low only x{low:.2}");
        assert!(high < 0.85, "high only x{high:.2}");
    }

    #[test]
    fn high_pass_kills_rumble() {
        let rumble = energy_at(40.0, CLEAR, true) / energy_at(40.0, CLEAR, false);
        assert!(rumble < 0.2, "rumble only x{rumble:.2}");
    }
}
