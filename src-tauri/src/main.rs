#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aec;
mod eq;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aec::EchoCanceller;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use nnnoiseless::DenoiseState;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow,
};

const GAP: i32 = 10;
/// Blur right after showing is Windows handing focus back; ignore it.
const BLUR_GRACE: Duration = Duration::from_millis(600);
/// Denoiser only works at this rate (RNNoise is trained on 48 kHz).
const RATE: u32 = 48_000;
const FRAME: usize = 480; // 10 ms, DenoiseState::FRAME_SIZE
const QUEUE: usize = 4_800; // 100 ms of monitor audio

static METER_RUNNING: AtomicBool = AtomicBool::new(false);
/// 0 = ringan, 1 = seimbang, 2 = maksimal. Wet mix read by the audio callback.
static REDUCTION: AtomicU8 = AtomicU8::new(1);
/// Denoiser on/off, mirrors the Mikrofon toggle.
static DENOISE: AtomicBool = AtomicBool::new(true);
/// Monitor playback is toggled live; the output stream is only built while it is on.
static MONITOR_ON: AtomicBool = AtomicBool::new(false);
/// Output device for the processed audio. None = system default.
static MONITOR_DEVICE: Mutex<Option<String>> = Mutex::new(None);
/// Echo cancellation, mirrors the Echo toggle. Off = no loopback stream at all.
static AEC_ON: AtomicBool = AtomicBool::new(false);
/// Equalizer preset; 0 is a true bypass.
static EQ_PRESET: AtomicU8 = AtomicU8::new(0);

type ShownAt = Arc<Mutex<Instant>>;

fn wet_mix(level: u8) -> f32 {
    match level {
        0 => 0.4,
        2 => 1.0,
        _ => 0.75,
    }
}

fn place(win: &WebviewWindow) {
    let size = match win.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let monitor = match win.current_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };
    let area = monitor.work_area();
    let x = area.position.x + area.size.width as i32 - size.width as i32 - GAP;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - GAP;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn show_popover(win: &WebviewWindow, shown: &ShownAt) {
    place(win);
    *shown.lock().unwrap() = Instant::now();
    let _ = win.show();
    let _ = win.set_focus();
    // Lets the UI re-enumerate audio devices: Windows only reports new ones on request.
    let _ = win.app_handle().emit("kriuk://shown", ());
}

fn toggle(win: &WebviewWindow, shown: &ShownAt) {
    if win.is_visible().unwrap_or(false) {
        halt_meter(win.app_handle());
        let _ = win.hide();
        return;
    }
    show_popover(win, shown);
}

#[tauri::command]
fn hide_window(win: WebviewWindow) {
    halt_meter(win.app_handle());
    let _ = win.hide();
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
fn set_reduction(level: u8) {
    REDUCTION.store(level.min(2), Ordering::Relaxed);
}

#[tauri::command]
fn set_denoise(on: bool) {
    DENOISE.store(on, Ordering::Relaxed);
}

#[tauri::command]
fn set_monitor(on: bool) {
    MONITOR_ON.store(on, Ordering::Relaxed);
}

#[tauri::command]
fn set_aec(on: bool) {
    AEC_ON.store(on, Ordering::Relaxed);
}

#[tauri::command]
fn set_eq(preset: u8) {
    EQ_PRESET.store(preset.min(eq::AUTO), Ordering::Relaxed);
}

#[tauri::command]
fn set_output(device: Option<String>) {
    *MONITOR_DEVICE.lock().unwrap() = device.filter(|d| !d.is_empty());
}

#[tauri::command]
fn list_outputs() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devices) => devices.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
fn start_meter(app: AppHandle, device: Option<String>) {
    if METER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        if let Err(err) = run_meter(&app, device.as_deref()) {
            let _ = app.emit("kriuk://meter-error", err);
        }
        MONITOR_ON.store(false, Ordering::Relaxed);
        METER_RUNNING.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
fn stop_meter() {
    METER_RUNNING.store(false, Ordering::SeqCst);
}

/// Closes the mic stream when the popover goes away, so nothing keeps listening unseen.
fn halt_meter(app: &AppHandle) {
    if METER_RUNNING.swap(false, Ordering::SeqCst) {
        let _ = app.emit("kriuk://meter-stopped", ());
    }
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

fn run_meter(app: &AppHandle, wanted: Option<&str>) -> Result<(), String> {
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
            &device, &config, channels, level.clone(), queue.clone(), reference.clone(),
        ),
        SampleFormat::I16 => capture::<i16>(
            &device, &config, channels, level.clone(), queue.clone(), reference.clone(),
        ),
        SampleFormat::U16 => capture::<u16>(
            &device, &config, channels, level.clone(), queue.clone(), reference.clone(),
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
                loopback = Some(spawn_loopback(app.clone(), reference.clone()));
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
                MONITOR_ON.store(false, Ordering::Relaxed);
            }
        } else if !want && monitor.is_some() {
            monitor = None;
            queue.lock().unwrap().clear();
        }
        let live = monitor.is_some();
        if live != reported {
            reported = live;
            let _ = app.emit("kriuk://monitor", live);
        }

        std::thread::sleep(Duration::from_millis(50));
        let value = {
            let mut slot = level.lock().unwrap();
            let v = *slot;
            *slot = 0.0;
            v
        };
        let _ = app.emit("kriuk://level", value);
    }
    Ok(())
}

/// Capture → downmix → denoise → wet mix → level + monitor queue.
/// Everything is allocated up front; the audio callback only writes into it.
fn capture<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    level: Arc<Mutex<f32>>,
    queue: Arc<Mutex<VecDeque<f32>>>,
    reference: Arc<Mutex<VecDeque<f32>>>,
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
                    let mut fifo = reference.lock().unwrap();
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
                }

                if denoise {
                    for i in 0..FRAME {
                        scaled[i] = acc[i] * 32_768.0;
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
                    let mut q = queue.lock().unwrap();
                    if q.len() + FRAME <= QUEUE {
                        q.extend(scaled.iter().copied());
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
        |e| eprintln!("kriuk capture: {e}"),
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
    let stream = device
        .build_output_stream(
            &supported.config(),
            move |data: &mut [f32], _| {
                let mut q = queue.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    let value = q.pop_front().unwrap_or(0.0);
                    for s in frame {
                        *s = value;
                    }
                }
            },
            |e| eprintln!("kriuk monitor: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}

/// Speaker audio kept as the echo reference. Deliberately short: the adaptive
/// filter covers 40 ms of echo path, so a deep buffer would push the reference
/// past what it can model.
const REF_QUEUE: usize = 960;
const _: () = assert!(FRAME == aec::BLOCK);

fn spawn_loopback(app: AppHandle, queue: Arc<Mutex<VecDeque<f32>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        if let Err(err) = run_loopback(&queue) {
            let _ = app.emit("kriuk://aec-error", err);
            AEC_ON.store(false, Ordering::Relaxed);
        }
    })
}

/// Captures what the speakers play, which is what the mic hears back.
fn run_loopback(queue: &Arc<Mutex<VecDeque<f32>>>) -> Result<(), String> {
    use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let fallback = || enumerator.get_default_device(&Direction::Render);
    let device = match MONITOR_DEVICE.lock().unwrap().clone() {
        Some(name) => render_by_name(&enumerator, &name).or_else(|| fallback().ok()),
        None => fallback().ok(),
    }
    .ok_or("no output device for echo reference")?;

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

        let mut fifo = queue.lock().unwrap();
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
            fifo.push_back(mono / channels as f32);
        }
        while fifo.len() > REF_QUEUE {
            fifo.pop_front();
        }
    }
    let _ = client.stop_stream();
    Ok(())
}

fn render_by_name(enumerator: &wasapi::DeviceEnumerator, name: &str) -> Option<wasapi::Device> {
    enumerator
        .get_device_collection(&wasapi::Direction::Render)
        .ok()?
        .get_device_with_name(name)
        .ok()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            hide_window,
            quit_app,
            list_devices,
            list_outputs,
            start_meter,
            stop_meter,
            set_reduction,
            set_denoise,
            set_monitor,
            set_aec,
            set_eq,
            set_output
        ])
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            let shown: ShownAt = Arc::new(Mutex::new(Instant::now()));
            show_popover(&win, &shown);

            let open = MenuItem::with_id(app, "open", "Open Kriuk", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let tray_shown = shown.clone();
            TrayIconBuilder::with_id("kriuk")
                .icon(Image::from_bytes(include_bytes!("../../assets/tray.png"))?)
                .tooltip("Kriuk")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            show_popover(&w, &tray_shown);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event({
                    let shown = shown.clone();
                    move |tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                toggle(&w, &shown);
                            }
                        }
                    }
                })
                .build(app)?;

            let blur_target = win.clone();
            let blur_shown = shown.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    if blur_shown.lock().unwrap().elapsed() > BLUR_GRACE {
                        halt_meter(blur_target.app_handle());
                        let _ = blur_target.hide();
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run kriuk");
}
