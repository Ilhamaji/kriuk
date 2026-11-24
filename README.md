# Kriuk

[![build](https://github.com/Ilhamaji/kriuk/actions/workflows/build.yml/badge.svg)](https://github.com/Ilhamaji/kriuk/actions/workflows/build.yml)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-lightgrey.svg)](#requirements)

Tray audio utility for Windows that cleans a microphone signal: RNNoise
denoising, echo cancellation, and a voice equalizer, with a live level meter
and routing to any output device.

Pure Rust on Tauri v2. One ~6 MB executable, no Electron, no Node at runtime,
no background service.

<img src="docs/screenshot.png" width="320" alt="Kriuk main window">

## Features

| Feature | How it works |
|---|---|
| Mic noise cancellation | RNNoise (`nnnoiseless`) at 48 kHz, 10 ms frames, on the WASAPI audio thread |
| Reduction level | Light / Balanced / Maximum = 40 / 75 / 100 % wet mix |
| Echo cancellation | Partitioned block frequency-domain adaptive filter (`rustfft`), 4 partitions × 480 samples ≈ 40 ms, reference from a WASAPI speaker loopback, Geigel double-talk guard |
| Equalizer | RBJ biquads: Neutral (true bypass), Clear, Warm, and Auto, which measures the 300 Hz / 3 kHz band balance twice a second and tilts the curve, clamped to ±4–5 dB |
| Level meter | Peak level of the processed signal |
| Send result to | Plays the processed audio on a chosen output device, including a virtual audio cable |
| Language | English by default, Indonesian available in Settings |
| Appearance | Light or dark, follows the system theme on first run |

Processing order: echo cancellation → denoiser → equalizer → meter / routing.
A disabled stage costs one atomic load per frame; no extra thread, no extra
stream.

Measured on the development laptop: 0 % CPU idle, ~4.7 % with the full chain
plus echo cancellation, ~32 MB RAM.

## Requirements

- Windows 10 1809 or newer, or Windows 11
- WebView2 Runtime — preinstalled on Windows 11 and on most Windows 10 machines

## Install

Download `Kriuk.exe` from [Releases](../../releases) and run it. There is no
installer: the app lives in the tray, and deleting the file removes it.

The build is not code-signed, so Windows SmartScreen shows
"Windows protected your PC" on first run. Choose **More info → Run anyway**.
A code-signing certificate is the only way to remove that prompt.

## Usage

1. Pick a microphone and leave **Microphone** on.
2. Press **Test mic** to start the chain; the meter shows the processed level.
3. **Echo cancellation** removes what your speaker plays from the mic signal.
   It needs the chain running and the reference speaker selected under
   **Send result to**.
4. To feed other apps, set **Send result to** to a virtual cable such as
   VB-CABLE or VoiceMeeter, then set that cable as the microphone in the other
   app. Windows has no way to inject audio into another app's mic without a
   virtual device.

Hiding the popover stops the chain and closes the microphone.

## Build

Requires the Rust MSVC toolchain and Visual Studio Build Tools with the
Desktop C++ workload.

```powershell
cargo build --release --manifest-path src-tauri\Cargo.toml
```

The executable lands in `src-tauri\target\release\kriuk.exe`. The UI in `src/`
is embedded into the binary at build time, so no separate asset step is needed.

Tests cover the DSP, not the UI:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

They assert echo reduction of 10 dB or more across the whole 40 ms filter span,
that double talk does not swallow the local voice, and that each equalizer
preset moves the band it claims to.

## How it works

```
speaker loopback ──┐
                   ├─→ echo canceller ─→ RNNoise ─→ equalizer ─→ meter
microphone ────────┘                                            └→ output device
```

| Path | Purpose |
|---|---|
| `src-tauri/src/main.rs` | tray icon, popover placement, hide on blur, cpal capture and monitor streams, loopback reference, IPC commands |
| `src-tauri/src/aec.rs` | PBFDAF echo canceller and its unit tests |
| `src-tauri/src/eq.rs` | biquad equalizer, presets, auto curve |
| `src/index.html`, `src/styles.css`, `src/app.js` | popover UI, no framework |
| `src-tauri/tauri.conf.json` | frameless transparent always-on-top window |
| `tools/make-icon.js` | generates the icon set without dependencies |
| `DESIGN.md` | visual specification |
| `REMAKE-RUST.md`, `REPORT.md` | reverse-engineering notes that led to this build |

## Limitations

- The audio chain runs while the mic test is active. Continuous background
  processing while the panel is hidden is not implemented yet.
- Echo cancellation is validated on synthetic signals, not yet measured in a
  real room; the loopback reference also cannot model a speaker that is not the
  selected output device.
- The tray menu is English only.
- Unsigned binary, so expect the SmartScreen prompt.

## License

[MIT](LICENSE)
