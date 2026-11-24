#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aec;
mod eq;

use std::collections::VecDeque;
use std::os::windows::process::CommandExt;
use std::process::Command;
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
/// Closing the window hides it instead of exiting, so the chain survives.
static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);
/// UI language for the strings Rust owns: tray menu and tooltip.
static LANG_ID: AtomicBool = AtomicBool::new(false);

struct TrayLabels {
    open: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

type ShownAt = Arc<Mutex<Instant>>;

fn wet_mix(level: u8) -> f32 {
    match level {
        0 => 0.25,
        2 => 1.0,
        _ => 0.6,
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
        // Hiding is not stopping: the chain keeps running in the tray.
        let _ = win.hide();
        return;
    }
    show_popover(win, shown);
}

#[tauri::command]
fn hide_window(win: WebviewWindow) {
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
    set_tray_status(&app);
    std::thread::spawn(move || {
        if let Err(err) = run_meter(&app, device.as_deref()) {
            let _ = app.emit("kriuk://meter-error", err);
        }
        MONITOR_ON.store(false, Ordering::Relaxed);
        METER_RUNNING.store(false, Ordering::SeqCst);
        set_tray_status(&app);
    });
}

#[tauri::command]
fn stop_meter() {
    METER_RUNNING.store(false, Ordering::SeqCst);
}

#[tauri::command]
fn meter_active() -> bool {
    METER_RUNNING.load(Ordering::SeqCst)
}

#[tauri::command]
fn set_close_to_tray(on: bool) {
    CLOSE_TO_TRAY.store(on, Ordering::Relaxed);
}

/// The tray tooltip is the only sign that the chain is live while the popover is hidden.
fn set_tray_status(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("kriuk") {
        let listening = METER_RUNNING.load(Ordering::SeqCst);
        let indonesian = LANG_ID.load(Ordering::Relaxed);
        let text = match (listening, indonesian) {
            (true, true) => "Kriuk — mendengarkan",
            (true, false) => "Kriuk — listening",
            _ => "Kriuk",
        };
        let _ = tray.set_tooltip(Some(text));
    }
}

/// The webview owns the language choice, so it pushes it here for the tray strings.
#[tauri::command]
fn set_language(app: AppHandle, lang: String) {
    let indonesian = lang == "id";
    LANG_ID.store(indonesian, Ordering::Relaxed);
    if let Some(labels) = app.try_state::<TrayLabels>() {
        let _ = labels.open.set_text(if indonesian { "Buka Kriuk" } else { "Open Kriuk" });
        let _ = labels.quit.set_text(if indonesian { "Keluar" } else { "Quit" });
    }
    set_tray_status(&app);
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Kriuk";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[tauri::command]
fn get_autostart() -> bool {
    Command::new("reg")
        .args(["query", RUN_KEY, "/v", RUN_VALUE])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[tauri::command]
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
fn refresh_autostart() {
    if get_autostart() {
        let _ = set_autostart(true);
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

        // Same device for monitor and reference: the canceller would hear its own
        // output, so refuse the pair instead of eating the voice.
        if live && monitor_is_reference(&host, monitor_target.as_deref()) {
            if AEC_ON.swap(false, Ordering::Relaxed) {
                let _ = app.emit(
                    "kriuk://aec-error",
                    "echo cancellation needs the processed audio on a different device",
                );
            }
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

/// Captures what the speakers play, which is what the mic hears back. Always the
/// default render device: the monitor target carries our own processed voice, so
/// using it as the reference would cancel the user instead of the echo.
fn run_loopback(queue: &Arc<Mutex<VecDeque<f32>>>) -> Result<(), String> {
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            hide_window,
            quit_app,
            list_devices,
            list_outputs,
            start_meter,
            stop_meter,
            meter_active,
            set_close_to_tray,
            get_autostart,
            set_autostart,
            set_reduction,
            set_denoise,
            set_monitor,
            set_aec,
            set_eq,
            set_output,
            set_language
        ])
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            let shown: ShownAt = Arc::new(Mutex::new(Instant::now()));
            // Autostart passes --hidden so signing in does not throw a popover on screen.
            if std::env::args().any(|a| a == "--hidden") {
                let _ = win.hide();
            } else {
                show_popover(&win, &shown);
            }

            let open = MenuItem::with_id(app, "open", "Open Kriuk", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            app.manage(TrayLabels {
                open: open.clone(),
                quit: quit.clone(),
            });
            refresh_autostart();

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
            win.on_window_event(move |event| match event {
                tauri::WindowEvent::Focused(false) => {
                    if blur_shown.lock().unwrap().elapsed() > BLUR_GRACE {
                        let _ = blur_target.hide();
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if CLOSE_TO_TRAY.load(Ordering::Relaxed) {
                        api.prevent_close();
                        let _ = blur_target.hide();
                    }
                }
                _ => {}
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run kriuk");
}

#[cfg(test)]
mod tests {
    use super::{same_device, wet_mix, DenoiseState, FRAME};

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

    #[test]
    fn monitor_on_the_default_device_is_the_echo_reference() {
        assert!(same_device(None, Some("Speakers (Realtek)")));
        assert!(same_device(Some("Speakers (Realtek)"), Some("speakers (realtek)")));
        assert!(!same_device(Some("CABLE Input (VB-Audio)"), Some("Speakers (Realtek)")));
        assert!(!same_device(Some("CABLE Input"), None));
    }
}
