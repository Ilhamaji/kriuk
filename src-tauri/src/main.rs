#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aec;
mod eq;
mod settings;
mod ui;

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aec::EchoCanceller;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use glutin::config::Config as GlConfig;
use glutin::context::PossiblyCurrentContext;
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, WindowSurface};
use nnnoiseless::DenoiseState;
use raw_window_handle::HasWindowHandle;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowId, WindowLevel};

const GAP: i32 = 10;
/// Blur right after showing is Windows handing focus back; ignore it.
const BLUR_GRACE: Duration = Duration::from_millis(600);
/// Denoiser only works at this rate (RNNoise is trained on 48 kHz).
const RATE: u32 = 48_000;
const FRAME: usize = 480; // 10 ms, DenoiseState::FRAME_SIZE
const QUEUE: usize = 9_600; // 200 ms of monitor audio
/// The monitor keeps this much audio in flight. The mic and the speaker run on
/// two independent clocks, so without a target depth the queue random-walks
/// into an empty buffer (clicks) or a full one (dropped blocks).
const MON_TARGET: usize = 1_920; // 40 ms
/// Deadband around the target; inside it nothing is stretched.
const MON_DEAD: usize = 240; // 5 ms

static METER_RUNNING: AtomicBool = AtomicBool::new(false);
/// 0 = light, 1 = balanced, 2 = maximum. Wet mix read by the audio callback.
static REDUCTION: AtomicU8 = AtomicU8::new(1);
/// Denoiser on/off, mirrors the Microphone toggle.
static DENOISE: AtomicBool = AtomicBool::new(true);
/// Monitor playback is toggled live; the output stream is only built while it is on.
static MONITOR_ON: AtomicBool = AtomicBool::new(false);
/// Output device for the processed audio. None = system default.
static MONITOR_DEVICE: Mutex<Option<String>> = Mutex::new(None);
/// Echo cancellation, mirrors the Echo toggle. Off = no loopback stream at all.
static AEC_ON: AtomicBool = AtomicBool::new(false);
/// Equalizer preset; 0 is a true bypass.
static EQ_PRESET: AtomicU8 = AtomicU8::new(0);
/// Closing the window hides it instead of exiting, so the chain survives.
static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);
/// UI language for the strings Rust owns: tray menu and tooltip.
static LANG_ID: AtomicBool = AtomicBool::new(false);
/// True while the popover exists, so the audio thread only wakes a visible UI.
static PANEL_OPEN: AtomicBool = AtomicBool::new(false);

pub type Shared = Arc<Mutex<State>>;

/// Everything the panel draws. Written by the audio thread and by the UI, so it
/// is the single place both sides look at.
pub struct State {
    pub devices: Vec<String>,
    pub outputs: Vec<String>,
    pub mic: Option<String>,
    pub spk: Option<String>,
    pub spk_on: bool,
    pub aec_on: bool,
    pub denoise: bool,
    pub reduction: u8,
    pub eq: u8,
    pub lang_id: bool,
    pub dark: bool,
    pub dark_set: bool,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub listening: bool,
    pub level: f32,
    pub peak: f32,
    /// Session-only: the focus mode is simulated, like it was in the webview.
    pub focus: bool,
    pub notice: Option<&'static str>,
    pub err: Option<String>,
    pub tab: u8,
    /// Peak dB and the clock time of each mic test, newest last.
    pub tests: Vec<(f32, String)>,
    /// Which dropdown is open; a native panel has to track it itself.
    pub open: Option<u8>,
}

impl State {
    fn from_saved(saved: settings::Saved, dark: bool) -> Self {
        Self {
            devices: Vec::new(),
            outputs: Vec::new(),
            mic: saved.mic,
            spk: saved.spk,
            spk_on: false,
            aec_on: saved.aec_on,
            denoise: saved.denoise,
            reduction: saved.reduction.min(2),
            eq: saved.eq.min(eq::AUTO),
            lang_id: saved.lang_id,
            dark,
            dark_set: saved.dark.is_some(),
            close_to_tray: saved.close_to_tray,
            autostart: false,
            listening: false,
            level: 0.0,
            peak: 0.0,
            focus: true,
            notice: None,
            err: None,
            tab: 0,
            tests: Vec::new(),
            open: None,
        }
    }

    fn saved(&self) -> settings::Saved {
        settings::Saved {
            chain: self.listening,
            mic: self.mic.clone(),
            spk: self.spk.clone(),
            aec_on: self.aec_on,
            denoise: self.denoise,
            reduction: self.reduction,
            eq: self.eq,
            lang_id: self.lang_id,
            dark: if self.dark_set { Some(self.dark) } else { None },
            close_to_tray: self.close_to_tray,
        }
    }
}

enum UserEvent {
    Wake,
    TrayClick,
    Menu(String),
}

fn wake(proxy: &EventLoopProxy<UserEvent>) {
    let _ = proxy.send_event(UserEvent::Wake);
}

fn wet_mix(level: u8) -> f32 {
    match level {
        0 => 0.25,
        2 => 1.0,
        _ => 0.6,
    }
}

/// Wall clock for the activity list, the way the webview printed it.
fn clock() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut t: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    format!("{:02}:{:02}", t.wHour, t.wMinute)
}

/// One row per finished mic test, newest last. It is a session log, so it is
/// capped instead of growing for as long as the app runs.
fn log_test(st: &mut State) {
    if st.listening && st.peak > 0.0 {
        st.tests.push((20.0 * st.peak.log10(), clock()));
        if st.tests.len() > 20 {
            st.tests.remove(0);
        }
    }
}

fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

fn list_outputs() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// WASAPI enumeration can take seconds, so it runs off the UI thread and the panel
/// redraws when the answer lands.
fn refresh_devices(shared: Shared, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let devices = list_devices();
        let outputs = list_outputs();
        {
            let mut st = shared.lock().unwrap();
            st.devices = devices;
            st.outputs = outputs;
        }
        wake(&proxy);
    });
}

fn start_meter(shared: Shared, proxy: EventLoopProxy<UserEvent>, device: Option<String>) {
    if METER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    {
        let mut st = shared.lock().unwrap();
        st.listening = true;
        st.err = None;
        st.notice = None;
        st.peak = 0.0;
    }
    wake(&proxy);
    std::thread::spawn(move || {
        if let Err(err) = run_meter(shared.clone(), proxy.clone(), device.as_deref()) {
            let mut st = shared.lock().unwrap();
            st.err = Some(err);
        }
        MONITOR_ON.store(false, Ordering::Relaxed);
        METER_RUNNING.store(false, Ordering::SeqCst);
        {
            let mut st = shared.lock().unwrap();
            st.listening = false;
            st.spk_on = false;
            st.level = 0.0;
        }
        wake(&proxy);
    });
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Kriuk";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn get_autostart() -> bool {
    Command::new("reg")
        .args(["query", RUN_KEY, "/v", RUN_VALUE])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn set_autostart(on: bool) -> Result<(), String> {
    let mut cmd = Command::new("reg");
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // --hidden starts in the tray at sign-in instead of opening the popover.
        cmd.args([
            "add",
            RUN_KEY,
            "/v",
            RUN_VALUE,
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{}\" --hidden", exe.display()),
            "/f",
        ]);
    } else {
        cmd.args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
    }
    let out = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// A Run entry holds an absolute path, so moving the exe leaves it dead. Rewrite
/// it with the current path on every launch: one manual start repairs autostart.
/// Returns the state after the repair, so startup only asks the registry once.
fn refresh_autostart() -> bool {
    let on = get_autostart();
    if on {
        let _ = set_autostart(true);
    }
    on
}

fn pick_device(host: &cpal::Host, wanted: Option<&str>) -> Result<cpal::Device, String> {
    if let Some(name) = wanted.filter(|n| !n.is_empty()) {
        if let Ok(devices) = host.input_devices() {
            if let Some(found) = devices
                .filter_map(|d| d.name().ok().map(|n| (n, d)))
                .find(|(n, _)| n == name)
                .map(|(_, d)| d)
            {
                return Ok(found);
            }
        }
        return Err(format!("input device not found: {name}"));
    }
    host.default_input_device()
        .ok_or_else(|| "no input device".to_string())
}

/// True when the monitor plays on the device the echo reference captures. Feeding
/// the processed mic back into the canceller would subtract the user's own voice.
fn monitor_is_reference(host: &cpal::Host, target: Option<&str>) -> bool {
    let default = host.default_output_device().and_then(|d| d.name().ok());
    same_device(target, default.as_deref())
}

fn same_device(target: Option<&str>, default: Option<&str>) -> bool {
    match (target, default) {
        (None, _) => true, // no explicit target: the monitor uses the default device
        (Some(name), Some(def)) => name.eq_ignore_ascii_case(def),
        (Some(_), None) => false,
    }
}

/// Windows names one box twice: "Speakers (ME6S)" and "Microphone (ME6S)". The
/// bracketed part is the hardware, so a match means the speaker and the mic are
/// the same physical device.
fn endpoint_box(name: &str) -> &str {
    match (name.find('('), name.rfind(')')) {
        (Some(open), Some(close)) if close > open => name[open + 1..close].trim(),
        _ => name.trim(),
    }
}

fn same_endpoint(a: &str, b: &str) -> bool {
    let (a, b) = (endpoint_box(a), endpoint_box(b));
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

/// A headset puts the speaker on your ear and the mic at your mouth, so the same
/// box is safe there. A speakerphone or laptop speaker is not.
fn plays_in_your_ear(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["headset", "headphone", "earphone"].iter().any(|k| name.contains(k))
}

fn run_meter(
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
    wanted: Option<&str>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = pick_device(&host, wanted)?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    if supported.sample_rate().0 != RATE {
        return Err(format!(
            "device runs at {} Hz, denoiser needs {} Hz",
            supported.sample_rate().0,
            RATE
        ));
    }

    let channels = supported.channels() as usize;
    let config = supported.config();
    let level = Arc::new(Mutex::new(0f32));
    let queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(QUEUE)));
    // Speaker audio used as the echo reference; only filled while AEC is on.
    let reference: Arc<Mutex<VecDeque<f32>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(REF_QUEUE)));

    let stream = match supported.sample_format() {
        SampleFormat::F32 => capture::<f32>(
            &device,
            &config,
            channels,
            level.clone(),
            queue.clone(),
            reference.clone(),
            shared.clone(),
            proxy.clone(),
        ),
        SampleFormat::I16 => capture::<i16>(
            &device,
            &config,
            channels,
            level.clone(),
            queue.clone(),
            reference.clone(),
            shared.clone(),
            proxy.clone(),
        ),
        SampleFormat::U16 => capture::<u16>(
            &device,
            &config,
            channels,
            level.clone(),
            queue.clone(),
            reference.clone(),
            shared.clone(),
            proxy.clone(),
        ),
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    // The output stream is built on demand: a silent render stream would keep the
    // audio engine awake on a laptop for nothing.
    let mut monitor: Option<cpal::Stream> = None;
    let mut monitor_target: Option<String> = None;
    let mut reported = false;
    let mut loopback: Option<std::thread::JoinHandle<()>> = None;

    while METER_RUNNING.load(Ordering::SeqCst) {
        // The loopback stream only exists while echo cancellation is on.
        if AEC_ON.load(Ordering::Relaxed) {
            if loopback.is_none() {
                loopback = Some(spawn_loopback(shared.clone(), proxy.clone(), reference.clone()));
            }
        } else if loopback.take().is_some() {
            reference.lock().unwrap().clear();
        }

        let target = MONITOR_DEVICE.lock().unwrap().clone();
        let want = MONITOR_ON.load(Ordering::Relaxed);
        if want && (monitor.is_none() || target != monitor_target) {
            monitor = build_monitor(&host, queue.clone(), target.as_deref());
            monitor_target = target;
            if monitor.is_none() {
                // build_monitor prefills the queue when it succeeds.
                queue.lock().unwrap().clear();
                MONITOR_ON.store(false, Ordering::Relaxed);
            }
        } else if !want && monitor.is_some() {
            monitor = None;
            queue.lock().unwrap().clear();
        }
        let live = monitor.is_some();
        if live != reported {
            reported = live;
            shared.lock().unwrap().spk_on = live;
            wake(&proxy);
        }

        // Same box for the mic and the monitor: the speaker sits inches from the
        // capsule, so the chain feeds itself. That loop is acoustic, no canceller
        // can see it, so refuse the output instead of howling.
        if live {
            let mic = device.name().unwrap_or_default();
            let out = monitor_target
                .clone()
                .or_else(|| host.default_output_device().and_then(|d| d.name().ok()));
            if let Some(out) = out.as_deref() {
                if same_endpoint(&mic, out)
                    && !plays_in_your_ear(out)
                    && MONITOR_ON.swap(false, Ordering::Relaxed)
                {
                    let mut st = shared.lock().unwrap();
                    st.notice = Some("same_box");
                    st.spk_on = false;
                    drop(st);
                    wake(&proxy);
                }
            }
        }

        // Same device for monitor and reference: the canceller would hear its own
        // output, so refuse the pair instead of eating the voice.
        if live && monitor_is_reference(&host, monitor_target.as_deref()) {
            if AEC_ON.swap(false, Ordering::Relaxed) {
                let mut st = shared.lock().unwrap();
                st.notice = Some("aec_same");
                st.aec_on = false;
                drop(st);
                wake(&proxy);
            }
        }

        std::thread::sleep(Duration::from_millis(50));
        let value = {
            let mut slot = level.lock().unwrap();
            let v = *slot;
            *slot = 0.0;
            v
        };
        {
            let mut st = shared.lock().unwrap();
            st.level = value;
            if value > st.peak {
                st.peak = value;
            }
        }
        if PANEL_OPEN.load(Ordering::Relaxed) {
            wake(&proxy);
        }
    }
    Ok(())
}

/// Converts a mic sample to the scale RNNoise expects. Its inverse FFT has an
/// unchecked path that is UB on a non-finite spectrum, so anything non-finite
/// becomes silence and the rest is clamped well below the f32 overflow point.
fn denoise_input(acc: f32) -> f32 {
    let v = acc * 32_768.0;
    if v.is_finite() {
        v.clamp(-131_072.0, 131_072.0)
    } else {
        0.0
    }
}

/// Capture → downmix → denoise → wet mix → level + monitor queue.
/// Everything is allocated up front; the audio callback only writes into it.
#[allow(clippy::too_many_arguments)]
fn capture<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    level: Arc<Mutex<f32>>,
    queue: Arc<Mutex<VecDeque<f32>>>,
    reference: Arc<Mutex<VecDeque<f32>>>,
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + Sample,
    f32: FromSample<T>,
{
    let mut denoiser = DenoiseState::new();
    let mut echo = EchoCanceller::new();
    let mut equalizer = eq::Eq::new();
    let mut eq_preset = eq::NEUTRAL;
    let mut acc = vec![0f32; FRAME];
    let mut ref_frame = vec![0f32; FRAME];
    let mut scaled = vec![0f32; FRAME];
    let mut clean = vec![0f32; FRAME];
    let mut filled = 0usize;

    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let denoise = DENOISE.load(Ordering::Relaxed);
            let wet = wet_mix(REDUCTION.load(Ordering::Relaxed));
            let monitor_on = MONITOR_ON.load(Ordering::Relaxed);
            let mut peak = 0f32;

            for frame in data.chunks_exact(channels) {
                let mut mono = 0f32;
                for s in frame {
                    mono += (*s).to_sample::<f32>();
                }
                acc[filled] = mono / channels as f32;
                filled += 1;

                if filled < FRAME {
                    continue;
                }
                filled = 0;

                if AEC_ON.load(Ordering::Relaxed) {
                    match reference.try_lock() {
                        Ok(mut fifo) => {
                            if fifo.len() >= FRAME {
                                for slot in ref_frame.iter_mut() {
                                    *slot = fifo.pop_front().unwrap_or(0.0);
                                }
                            } else {
                                // Underrun: cancel nothing this block rather than guess.
                                ref_frame.fill(0.0);
                                fifo.clear();
                            }
                            drop(fifo);
                            echo.process(&ref_frame, &mut acc);
                            // RNNoise's FFT overflows to Inf on values far outside the
                            // +/-1 a mic produces, and its inverse FFT has an unchecked
                            // path that is UB on the resulting non-finite spectrum. A
                            // filter that pushes the block past a few units has
                            // diverged, so clear it and pass silence instead.
                            if acc.iter().any(|s| !s.is_finite() || s.abs() > 4.0) {
                                acc.fill(0.0);
                                echo.reset();
                            }
                        }
                        // Lock contention: skip AEC this block, never block the
                        // callback. The filter keeps its weights for next time.
                        Err(_) => {}
                    }
                }

                if denoise {
                    for i in 0..FRAME {
                        scaled[i] = denoise_input(acc[i]);
                    }
                    denoiser.process_frame(&mut clean, &scaled);
                    for i in 0..FRAME {
                        scaled[i] = acc[i] * (1.0 - wet) + (clean[i] / 32_768.0) * wet;
                    }
                } else {
                    scaled.copy_from_slice(&acc);
                }

                let wanted_eq = EQ_PRESET.load(Ordering::Relaxed);
                if wanted_eq != eq_preset {
                    equalizer.set_preset(wanted_eq);
                    eq_preset = wanted_eq;
                }
                if eq_preset != eq::NEUTRAL {
                    equalizer.process(&mut scaled);
                }

                for s in scaled.iter() {
                    let a = s.abs();
                    if a > peak {
                        peak = a;
                    }
                }

                if monitor_on {
                    match queue.try_lock() {
                        Ok(mut q) => {
                            if q.len() + FRAME <= QUEUE {
                                q.extend(scaled.iter().copied());
                            }
                        }
                        // A miss drops one block; the monitor repeats its last
                        // sample instead of stalling the capture callback.
                        Err(_) => {}
                    }
                }
            }

            if peak > 0.0 {
                let mut slot = level.lock().unwrap();
                if peak > *slot {
                    *slot = peak;
                }
            }
        },
        // A yanked USB mic stops the stream silently otherwise: the meter would
        // keep saying "live" with a dead device behind it.
        move |e| {
            if let Ok(mut st) = shared.lock() {
                st.err = Some(format!("mic stopped: {e}"));
            }
            METER_RUNNING.store(false, Ordering::SeqCst);
            wake(&proxy);
        },
        None,
    )
}

/// Plays the processed mic. Best-effort: any failure just means no monitoring.
fn build_monitor(
    host: &cpal::Host,
    queue: Arc<Mutex<VecDeque<f32>>>,
    wanted: Option<&str>,
) -> Option<cpal::Stream> {
    let device = match wanted {
        Some(name) => host
            .output_devices()
            .ok()?
            .filter_map(|d| d.name().ok().map(|n| (n, d)))
            .find(|(n, _)| n == name)
            .map(|(_, d)| d)?,
        None => host.default_output_device()?,
    };
    // Playback has to match the capture rate or the monitor sounds pitched.
    let supported = device
        .supported_output_configs()
        .ok()?
        .filter(|c| {
            c.sample_format() == SampleFormat::F32
                && c.min_sample_rate().0 <= RATE
                && c.max_sample_rate().0 >= RATE
        })
        .next()?
        .with_sample_rate(cpal::SampleRate(RATE));
    let channels = supported.channels() as usize;
    let mut last = 0f32;
    let fill = queue.clone();
    let stream = device
        .build_output_stream(
            &supported.config(),
            move |data: &mut [f32], _| {
                let frames = data.len() / channels;
                match queue.try_lock() {
                    Ok(mut q) => {
                        // Two devices, two clocks: the queue slowly drains or
                        // grows. Steal or repeat a single sample per callback,
                        // and only outside the deadband, so the depth stays put
                        // and the correction is one sample out of hundreds.
                        let pop = if q.len() > MON_TARGET + MON_DEAD {
                            frames as i32 + 1
                        } else if q.len() < MON_TARGET - MON_DEAD {
                            frames as i32 - 1
                        } else {
                            frames as i32
                        };
                        let mut taken = 0i32;
                        for frame in data.chunks_mut(channels) {
                            if taken < pop {
                                match q.pop_front() {
                                    Some(value) => last = value,
                                    // Underrun: hold the last sample. A zero here
                                    // is a step to silence and an audible click.
                                    None => {}
                                }
                                taken += 1;
                            }
                            for s in frame {
                                *s = last;
                            }
                        }
                        // Catch-up path consumed one more than it wrote.
                        if pop > frames as i32 {
                            let _ = q.pop_front();
                        }
                    }
                    // The capture side is mid-write; repeating one buffer of the
                    // last sample beats a hard gap.
                    Err(_) => {
                        for s in data.iter_mut() {
                            *s = last;
                        }
                    }
                }
            },
            |e| eprintln!("kriuk monitor: {e}"),
            None,
        )
        .ok()?;
    // Start at the target depth: an empty queue means the first callbacks
    // underrun while the capture side is still filling.
    {
        let mut q = fill.lock().unwrap();
        q.clear();
        q.resize(MON_TARGET, 0.0);
    }
    stream.play().ok()?;
    Some(stream)
}

/// Speaker audio kept as the echo reference. Deliberately short: the adaptive
/// filter covers 40 ms of echo path, so a deep buffer would push the reference
/// past what it can model.
const REF_QUEUE: usize = 1_920; // 40 ms, 4 AEC partitions; try_lock misses need headroom
const _: () = assert!(FRAME == aec::BLOCK);

fn spawn_loopback(
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        if let Err(err) = run_loopback(&queue) {
            AEC_ON.store(false, Ordering::Relaxed);
            let mut st = shared.lock().unwrap();
            st.aec_on = false;
            st.err = Some(err);
            drop(st);
            wake(&proxy);
        }
    })
}

/// Captures what the speakers play, which is what the mic hears back. Always the
/// default render device: the monitor target carries our own processed voice, so
/// using it as the reference would cancel the user instead of the echo.
fn run_loopback(queue: &Arc<Mutex<VecDeque<f32>>>) -> Result<(), String> {
    // A fresh thread starts with no COM apartment, and every WASAPI call needs
    // one. Without this the enumerator fails with 0x800401F0 and echo
    // cancellation silently never runs.
    let _ = wasapi::initialize_mta();
    let result = loopback_body(queue);
    wasapi::deinitialize();
    result
}

fn loopback_body(queue: &Arc<Mutex<VecDeque<f32>>>) -> Result<(), String> {
    use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|e| e.to_string())?;

    let mut client = device.get_iaudioclient().map_err(|e| e.to_string())?;
    let channels = client
        .get_mixformat()
        .map_err(|e| e.to_string())?
        .get_nchannels() as usize;
    // Always 48 kHz float; autoconvert makes WASAPI resample the endpoint.
    let format = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, channels, None);
    client
        .initialize_client(
            &format,
            &Direction::Capture,
            &StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            },
        )
        .map_err(|e| e.to_string())?;
    let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
    client.start_stream().map_err(|e| e.to_string())?;

    let mut bytes: Vec<u8> = Vec::new();
    while AEC_ON.load(Ordering::Relaxed) && METER_RUNNING.load(Ordering::SeqCst) {
        let frames = match capture.get_next_packet_size().map_err(|e| e.to_string())? {
            Some(0) | None => {
                std::thread::sleep(Duration::from_millis(4));
                continue;
            }
            Some(n) => n as usize,
        };
        let need = frames * channels * 4;
        if bytes.len() < need {
            bytes.resize(need, 0);
        }
        let (read, _) = capture
            .read_from_device(&mut bytes[..need])
            .map_err(|e| e.to_string())?;

        if let Ok(mut fifo) = queue.try_lock() {
            for frame in 0..read as usize {
                let mut mono = 0f32;
                for ch in 0..channels {
                    let at = (frame * channels + ch) * 4;
                    mono += f32::from_le_bytes([
                        bytes[at],
                        bytes[at + 1],
                        bytes[at + 2],
                        bytes[at + 3],
                    ]);
                }
                // A device that ignores the requested float format yields garbage
                // here; keep it finite and bounded so a bad reference can never
                // drive the canceller off the rails.
                let sample = mono / channels as f32;
                fifo.push_back(if sample.is_finite() {
                    sample.clamp(-8.0, 8.0)
                } else {
                    0.0
                });
            }
            while fifo.len() > REF_QUEUE {
                fifo.pop_front();
            }
        }
        // Lock contention: skip this packet; the capture side will see an
        // underrun and cancel nothing, which is better than blocking.
    }
    let _ = client.stop_stream();
    Ok(())
}

/// The popover and its GL surface. Dropping it gives the memory back: there is no
/// hidden window and no second process while the app sits in the tray.
struct Panel {
    // Fields drop in order, so the GL surface and context go before the HWND they
    // were made for; the window is last on purpose.
    painter: egui_glow::Painter,
    gl: Arc<glow::Context>,
    state: egui_winit::State,
    ctx: egui::Context,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    _config: GlConfig,
    window: Window,
    next_repaint: Option<Instant>,
}

impl Panel {
    fn open(el: &ActiveEventLoop) -> Option<Self> {
        let attrs = Window::default_attributes()
            .with_title("Kriuk")
            .with_inner_size(LogicalSize::new(380.0, 780.0))
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_skip_taskbar(true)
            .with_visible(false);
        let template = glutin::config::ConfigTemplateBuilder::new()
            .with_transparency(true)
            .prefer_hardware_accelerated(Some(true));
        let (window, config) = glutin_winit::DisplayBuilder::new()
            .with_preference(glutin_winit::ApiPreference::FallbackEgl)
            .with_window_attributes(Some(attrs))
            .build(el, template, |mut configs| {
                configs.next().expect("no gl config for the panel")
            })
            .ok()?;
        let window = window?;

        let display = config.display();
        let raw = window.window_handle().ok()?.as_raw();
        let ctx_attrs = glutin::context::ContextAttributesBuilder::new().build(Some(raw));
        let not_current = unsafe { display.create_context(&config, &ctx_attrs).ok()? };
        let size = window.inner_size();
        let surf_attrs = glutin::surface::SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw,
            NonZeroU32::new(size.width.max(1))?,
            NonZeroU32::new(size.height.max(1))?,
        );
        let surface = unsafe { display.create_window_surface(&config, &surf_attrs).ok()? };
        let context = not_current.make_current(&surface).ok()?;
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                display.get_proc_address(&std::ffi::CString::new(s).unwrap())
            })
        };
        let gl = Arc::new(gl);
        let painter = egui_glow::Painter::new(gl.clone(), "", None, false).ok()?;

        if let Some((_left, _top, right, bottom)) = work_area() {
            let s = window.scale_factor();
            let w = (380.0 * s) as i32;
            let h = (780.0 * s) as i32;
            let gap = (GAP as f64 * s) as i32;
            window.set_outer_position(PhysicalPosition::new(right - w - gap, bottom - h - gap));
        }
        window.set_visible(true);
        window.focus_window();

        let ctx = egui::Context::default();
        // Fonts are rebuilt at the start of a frame, so they have to be set before
        // the first one; asking for them inside draw panics on an unknown family.
        ui::install_fonts(&ctx);
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        PANEL_OPEN.store(true, Ordering::Relaxed);
        Some(Self {
            painter,
            gl,
            state,
            ctx,
            surface,
            context,
            _config: config,
            window,
            next_repaint: None,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            self.surface.resize(&self.context, w, h);
        }
    }

    fn paint(&mut self, shared: &Shared) -> Vec<ui::Action> {
        let raw = self.state.take_egui_input(&self.window);
        let mut actions = Vec::new();
        let output = self.ctx.run(raw, |ctx| {
            let mut st = shared.lock().unwrap();
            actions = ui::draw(ctx, &mut st);
        });
        self.state
            .handle_platform_output(&self.window, output.platform_output.clone());
        let clipped = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let size = self.window.inner_size();
        unsafe {
            use glow::HasContext;
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.painter.paint_and_update_textures(
            [size.width.max(1), size.height.max(1)],
            output.pixels_per_point,
            &clipped,
            &output.textures_delta,
        );
        let _ = self.surface.swap_buffers(&self.context);

        let delay = output
            .viewport_output
            .values()
            .map(|v| v.repaint_delay)
            .min()
            .unwrap_or(Duration::from_secs(3600));
        self.next_repaint = if delay < Duration::from_secs(1) {
            Some(Instant::now() + delay.max(Duration::from_millis(8)))
        } else {
            None
        };
        actions
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        self.painter.destroy();
        PANEL_OPEN.store(false, Ordering::Relaxed);
    }
}

/// The taskbar eats part of the monitor, so the panel has to sit inside the work
/// area rather than the screen bounds.
fn work_area() -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut rect as *mut RECT as *mut core::ffi::c_void,
            0,
        )
    };
    (ok != 0).then_some((rect.left, rect.top, rect.right, rect.bottom))
}

struct App {
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
    panel: Option<Panel>,
    tray: Option<TrayIcon>,
    open_item: Option<MenuItem>,
    quit_item: Option<MenuItem>,
    tooltip: String,
    chain_saved: bool,
    shown_at: Instant,
    started: bool,
    follow_system_theme: bool,
    pending: Vec<ui::Action>,
}

impl App {
    fn show_panel(&mut self, el: &ActiveEventLoop) {
        if let Some(panel) = &self.panel {
            panel.window.focus_window();
            return;
        }
        // Windows only reports new devices on request, so refresh on every open.
        // Enumeration takes seconds on some machines, so it never runs on the UI
        // thread; the list from the previous pass is already on screen.
        {
            // A dropdown left open when the panel hid would reopen by itself.
            let mut st = self.shared.lock().unwrap();
            st.open = None;
        }
        refresh_devices(self.shared.clone(), self.proxy.clone());
        self.shown_at = Instant::now();
        match Panel::open(el) {
            Some(panel) => self.panel = Some(panel),
            None => {
                self.shared.lock().unwrap().err = Some("could not open the panel".into());
            }
        }
    }

    fn hide_panel(&mut self) {
        self.panel = None;
        self.pending.clear();
    }

    fn toggle_panel(&mut self, el: &ActiveEventLoop) {
        if self.panel.is_some() {
            self.hide_panel();
        } else {
            self.show_panel(el);
        }
    }

    fn build_tray(&mut self) {
        let indonesian = self.shared.lock().unwrap().lang_id;
        let open = MenuItem::with_id(
            "open",
            if indonesian { "Buka Kriuk" } else { "Open Kriuk" },
            true,
            None,
        );
        let quit = MenuItem::with_id(
            "quit",
            if indonesian { "Keluar" } else { "Quit" },
            true,
            None,
        );
        let menu = Menu::new();
        let _ = menu.append(&open);
        let _ = menu.append(&quit);

        let png = image::load_from_memory(include_bytes!("../../assets/tray.png"))
            .expect("tray.png")
            .into_rgba8();
        let (w, h) = png.dimensions();
        let icon = tray_icon::Icon::from_rgba(png.into_raw(), w, h).expect("tray icon");
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Kriuk")
            .with_icon(icon)
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(err) => eprintln!("kriuk tray: {err}"),
        }
        self.open_item = Some(open);
        self.quit_item = Some(quit);
    }

    fn relabel_tray(&mut self) {
        let indonesian = self.shared.lock().unwrap().lang_id;
        if let Some(item) = &self.open_item {
            let _ = item.set_text(if indonesian { "Buka Kriuk" } else { "Open Kriuk" });
        }
        if let Some(item) = &self.quit_item {
            let _ = item.set_text(if indonesian { "Keluar" } else { "Quit" });
        }
    }

    fn apply(&mut self, action: ui::Action, _el: &ActiveEventLoop) {
        let mut hide = false;
        let mut relabel = false;
        let mut write = true;
        let mut saved = None;
        let mut start: Option<Option<String>> = None;
        {
            let mut st = self.shared.lock().unwrap();
            match action {
                ui::Action::Tab(i) => {
                    st.tab = i;
                    st.open = None;
                    write = false;
                }
                ui::Action::Hide => {
                    hide = true;
                    write = false;
                }
                ui::Action::Denoise(on) => {
                    st.denoise = on;
                    DENOISE.store(on, Ordering::Relaxed);
                    if !on {
                        // Stopping the chain ends the current test, so its peak
                        // still lands in the activity list.
                        log_test(&mut st);
                        METER_RUNNING.store(false, Ordering::SeqCst);
                    }
                }
                ui::Action::Reduction(i) => {
                    st.reduction = i;
                    REDUCTION.store(i, Ordering::Relaxed);
                }
                ui::Action::Eq(i) => {
                    st.eq = i;
                    EQ_PRESET.store(i, Ordering::Relaxed);
                }
                ui::Action::Aec(on) => {
                    st.aec_on = on;
                    st.notice = None;
                    AEC_ON.store(on, Ordering::Relaxed);
                }
                ui::Action::Spk(on) => {
                    st.spk_on = on;
                    st.notice = None;
                    MONITOR_ON.store(on, Ordering::Relaxed);
                    // Routing only exists while the mic chain runs, so turning it
                    // on starts the chain, the way the webview did.
                    if on && !st.listening {
                        start = Some(st.mic.clone());
                    }
                }
                ui::Action::Mic(device) => {
                    st.mic = device;
                }
                ui::Action::SpkDevice(device) => {
                    *MONITOR_DEVICE.lock().unwrap() = device.clone();
                    st.spk = device;
                }
                ui::Action::TestToggle => {
                    if st.listening {
                        log_test(&mut st);
                        METER_RUNNING.store(false, Ordering::SeqCst);
                    } else {
                        start = Some(st.mic.clone());
                    }
                }
                ui::Action::Lang(id) => {
                    st.lang_id = id;
                    LANG_ID.store(id, Ordering::Relaxed);
                    relabel = true;
                }
                ui::Action::Dark(dark) => {
                    st.dark = dark;
                    st.dark_set = true;
                    self.follow_system_theme = false;
                }
                ui::Action::Focus(on) => st.focus = on,
                ui::Action::CloseToTray(on) => {
                    st.close_to_tray = on;
                    CLOSE_TO_TRAY.store(on, Ordering::Relaxed);
                }
                ui::Action::Autostart(on) => match set_autostart(on) {
                    Ok(()) => st.autostart = on,
                    Err(err) => st.err = Some(err),
                },
            }
            // Tab and Hide change nothing that is stored, so they skip the write.
            if write {
                saved = Some(st.saved());
            }
        }

        if let Some(saved) = saved {
            settings::save(&saved);
        }

        if relabel {
            self.relabel_tray();
        }
        if let Some(device) = start {
            start_meter(self.shared.clone(), self.proxy.clone(), device);
        }
        if hide {
            self.hide_panel();
        } else if let Some(panel) = &self.panel {
            panel.window.request_redraw();
        }
    }

    fn sync_tray(&mut self) {
        let (listening, indonesian) = {
            let st = self.shared.lock().unwrap();
            (st.listening, st.lang_id)
        };
        let text = match (listening, indonesian) {
            (true, true) => "Kriuk — mendengarkan",
            (true, false) => "Kriuk — listening",
            _ => "Kriuk",
        };
        if self.tooltip != text {
            self.tooltip = text.to_string();
            if let Some(tray) = &self.tray {
                let _ = tray.set_tooltip(Some(text));
            }
        }
        // The chain flag is what brings it back on the next launch, so it is
        // written when the chain starts or stops, not only when a switch moves.
        if self.chain_saved != listening {
            self.chain_saved = listening;
            let saved = self.shared.lock().unwrap().saved();
            settings::save(&saved);
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        if self.follow_system_theme {
            let dark = el
                .system_theme()
                .map(|theme| theme == winit::window::Theme::Dark)
                .unwrap_or(false);
            self.shared.lock().unwrap().dark = dark;
        }
        self.build_tray();
        self.shared.lock().unwrap().autostart = refresh_autostart();

        let hidden = std::env::args().any(|a| a == "--hidden");
        if !hidden {
            self.show_panel(el);
        }
        self.sync_tray();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let is_panel = self.panel.as_ref().map(|p| p.window.id()) == Some(id);
        if !is_panel {
            return;
        }
        let close_to_tray = CLOSE_TO_TRAY.load(Ordering::Relaxed);
        let mut hide = false;
        let mut exit = false;

        if let Some(panel) = self.panel.as_mut() {
            let _ = panel.state.on_window_event(&panel.window, &event);
            match &event {
                WindowEvent::CloseRequested => {
                    if close_to_tray {
                        hide = true;
                    } else {
                        exit = true;
                    }
                }
                WindowEvent::Focused(false) => {
                    if self.shown_at.elapsed() > BLUR_GRACE {
                        hide = true;
                    }
                }
                WindowEvent::Resized(size) => panel.resize(size.width, size.height),
                WindowEvent::RedrawRequested => {
                    let shared = self.shared.clone();
                    let actions = panel.paint(&shared);
                    self.pending.extend(actions);
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let esc = event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape);
                    if esc && event.state == winit::event::ElementState::Pressed {
                        hide = true;
                    }
                }
                _ => {}
            }
        }

        if hide {
            self.hide_panel();
        }
        if exit {
            el.exit();
        }
        if !self.pending.is_empty() {
            let actions = std::mem::take(&mut self.pending);
            for action in actions {
                self.apply(action, el);
            }
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Wake => {
                self.sync_tray();
                if let Some(panel) = &self.panel {
                    panel.window.request_redraw();
                }
            }
            UserEvent::TrayClick => self.toggle_panel(el),
            UserEvent::Menu(id) => match id.as_str() {
                "open" => self.show_panel(el),
                "quit" => el.exit(),
                _ => {}
            },
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        match self.pending_repaint() {
            Some(at) if self.panel.is_some() => {
                if Instant::now() >= at {
                    if let Some(panel) = &self.panel {
                        panel.window.request_redraw();
                    }
                    el.set_control_flow(ControlFlow::Wait);
                } else {
                    el.set_control_flow(ControlFlow::WaitUntil(at));
                }
            }
            _ => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

impl App {
    fn pending_repaint(&mut self) -> Option<Instant> {
        let at = self.panel.as_ref().and_then(|p| p.next_repaint)?;
        if Instant::now() >= at {
            if let Some(panel) = self.panel.as_mut() {
                panel.next_repaint = None;
            }
            return Some(at);
        }
        Some(at)
    }
}

fn main() {
    let saved = settings::load();
    let follow_system_theme = saved.dark.is_none();
    let dark = saved.dark.unwrap_or(false);
    // A chain that was live when the app quit comes back listening.
    let chain = saved.chain;
    let chain_mic = saved.mic.clone();

    let state = State::from_saved(saved, dark);
    DENOISE.store(state.denoise, Ordering::Relaxed);
    REDUCTION.store(state.reduction, Ordering::Relaxed);
    EQ_PRESET.store(state.eq, Ordering::Relaxed);
    LANG_ID.store(state.lang_id, Ordering::Relaxed);
    CLOSE_TO_TRAY.store(state.close_to_tray, Ordering::Relaxed);
    // Without this the panel shows Echo on after a restart while the canceller
    // is dead, so the echo the user already switched off comes back.
    AEC_ON.store(state.aec_on, Ordering::Relaxed);
    *MONITOR_DEVICE.lock().unwrap() = state.spk.clone();

    let shared: Shared = Arc::new(Mutex::new(state));

    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(loop_) => loop_,
        Err(err) => {
            eprintln!("kriuk: {err}");
            return;
        }
    };
    let proxy = event_loop.create_proxy();

    {
        let tray_proxy = proxy.clone();
        std::thread::spawn(move || {
            while let Ok(event) = TrayIconEvent::receiver().recv() {
                if let TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = tray_proxy.send_event(UserEvent::TrayClick);
                }
            }
        });
        let menu_proxy = proxy.clone();
        std::thread::spawn(move || {
            while let Ok(event) = MenuEvent::receiver().recv() {
                let _ = menu_proxy.send_event(UserEvent::Menu(event.id.0.to_string()));
            }
        });
    }

    let mut app = App {
        shared: shared.clone(),
        proxy,
        panel: None,
        tray: None,
        open_item: None,
        quit_item: None,
        tooltip: String::new(),
        chain_saved: false,
        shown_at: Instant::now(),
        started: false,
        follow_system_theme,
        pending: Vec::new(),
    };

    if chain {
        start_meter(shared.clone(), app.proxy.clone(), chain_mic);
    }
    refresh_devices(shared.clone(), app.proxy.clone());

    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("kriuk: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        denoise_input, plays_in_your_ear, same_device, same_endpoint, wet_mix, DenoiseState, FRAME,
    };

    /// The denoiser is fed this value, and its inverse FFT is UB on a non-finite
    /// spectrum, so no input may reach it as NaN, Inf, or an overflow-sized number.
    #[test]
    fn denoise_input_is_always_finite_and_bounded() {
        assert_eq!(denoise_input(f32::INFINITY), 0.0);
        assert_eq!(denoise_input(f32::NEG_INFINITY), 0.0);
        assert_eq!(denoise_input(f32::NAN), 0.0);
        assert_eq!(denoise_input(1e30), 131_072.0);
        assert_eq!(denoise_input(-1e30), -131_072.0);
        assert_eq!(denoise_input(0.5), 16_384.0);
    }

    fn noise(i: usize) -> f32 {
        let mut x = (i as u32).wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        x = (x ^ (x >> 16)).wrapping_mul(2_246_822_519);
        x ^= x >> 13;
        x as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// The three reduction levels must actually differ, not just relabel a constant.
    #[test]
    fn reduction_levels_change_how_much_noise_survives() {
        let mut denoiser = DenoiseState::new();
        let mut input = vec![0f32; FRAME];
        let mut clean = vec![0f32; FRAME];
        let light_wet = wet_mix(0);
        let maximum_wet = wet_mix(2);
        let mut energy = [0f64; 2];
        let mut blocks = 0usize;

        for block in 0..400 {
            for (i, s) in input.iter_mut().enumerate() {
                *s = 0.06 * noise(block * FRAME + i);
            }
            let scaled: Vec<f32> = input.iter().map(|s| s * 32_768.0).collect();
            denoiser.process_frame(&mut clean, &scaled);
            if block < 40 {
                continue; // let the model settle before measuring
            }
            for i in 0..FRAME {
                let wet = clean[i] / 32_768.0;
                let light = input[i] * (1.0 - light_wet) + wet * light_wet;
                let maximum = input[i] * (1.0 - maximum_wet) + wet * maximum_wet;
                energy[0] += (light * light) as f64;
                energy[1] += (maximum * maximum) as f64;
            }
            blocks += 1;
        }

        let samples = (blocks * FRAME) as f64;
        let light = (energy[0] / samples).sqrt();
        let maximum = (energy[1] / samples).sqrt();
        let gap_db = 20.0 * (light / maximum).log10();
        // RNNoise itself only pulls white noise down by ~9 dB, so that is the ceiling
        // for any wet-mix preset; real hiss and fans give it more to work with.
        assert!(
            gap_db > 5.0,
            "light keeps only {gap_db:.1} dB more noise than maximum; the presets are too close"
        );
    }

    /// The activity tab is a session log, so it must not grow without bound.
    #[test]
    fn the_activity_list_is_capped() {
        let mut st = crate::State::from_saved(crate::settings::Saved::default(), false);
        st.listening = true;
        st.peak = 0.5;
        for _ in 0..40 {
            crate::log_test(&mut st);
        }
        assert_eq!(st.tests.len(), 20);
        assert!(st.tests[0].1.contains(':'));
    }

    #[test]
    fn monitor_on_the_default_device_is_the_echo_reference() {
        assert!(same_device(None, Some("Speakers (Realtek)")));
        assert!(same_device(Some("Speakers (Realtek)"), Some("speakers (realtek)")));
        assert!(!same_device(Some("CABLE Input (VB-Audio)"), Some("Speakers (Realtek)")));
        assert!(!same_device(Some("CABLE Input"), None));
    }

    /// Names measured on the development laptop: the ME6S is a speakerphone, so
    /// playing on "Speakers (ME6S)" puts the output inches from "Microphone (ME6S)".
    #[test]
    fn a_speaker_and_a_mic_on_one_box_are_the_same_endpoint() {
        assert!(same_endpoint("Microphone (ME6S)", "Speakers (ME6S)"));
        assert!(same_endpoint("Microphone (Realtek(R) Audio)", "Speakers (Realtek(R) Audio)"));
        assert!(!same_endpoint("Microphone (ME6S)", "Speakers (USB Audio Device)"));
        assert!(!same_endpoint("Microphone (ME6S)", "CABLE Input (VB-Audio Virtual Cable)"));
        assert!(!same_endpoint("", "Speakers (ME6S)"));
        // A headset shares a box with its own mic but keeps the speaker on your ear.
        assert!(same_endpoint("Microphone (USB Audio Device)", "Headset Earphone (USB Audio Device)"));
        assert!(plays_in_your_ear("Headset Earphone (USB Audio Device)"));
        assert!(!plays_in_your_ear("Speakers (ME6S)"));
    }
}
