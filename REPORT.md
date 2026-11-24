# Krisp 3.16.8 (Windows x64) — Reverse Engineering Report

Target diperoleh **tanpa menjalankan installer**. Semua artefak ada di `artifacts/`.

## 1. Rantai artefak

| Langkah | Hasil |
|---|---|
| `winget download XP9D25XXG3SV5X` (msstore) | `Krisp_3.16.8_User_X64_exe_en.exe`, 325.839.904 B, sha256 `46C89D2E…88AA0` |
| PE inspect (REA `inspect_pe_imports`) | PE32 (x86), delay-import `Cabinet.dll`, `msi.dll`, `WININET`, `WINTRUST` → WiX Burn |
| Section table | ada `.wixburn` (magic `0x00F14300`, v2) |
| Overlay scan | 3 CAB: UX cab @1003520, payload cab @1185912 (`cbCabinet=324.643.292`, 2 file: `a0`,`a1`) |
| `expand.exe` | `a0` = `KrispDriversInstaller-x64.msi` (5.537.792 B), `a1` = `Krisp.msi` (321.056.768 B) — ukuran cocok manifest |
| `msiexec /a` (administrative extract, bukan install) | `artifacts/app` (209 file, 628 MB), `artifacts/drivers` |
| `tools/asar-extract.js` | `artifacts/asar` (190 file, 106 MB) dari `app.asar` |

Manifest Burn (`artifacts/burn/0`) memberi ProductCode, UpgradeCode, dan nama payload persis.

## 2. Arsitektur aplikasi

```
Electron 40.8.5 (krisp.exe, 204 MB)
├── renderer: main_window / widget / companion / notification_*  (React, Vite)
└── main process (.vite/build/BootstrapApp-*.js, 6 MB, obfuscated)
    ├── kr-media-sp.node      ← inti DSP/ML (27 MB, N-API, statis link ONNX Runtime)
    ├── kr-audio-dm.node      ← manajemen device audio + volume
    ├── kr-ext-hub.node       ← hub integrasi browser extension
    ├── kr-native-utils.node  ← registry, WMI, secitem, monitoring proses
    ├── kr-node-logger.node
    ├── daily-core-sdk-wrapper.dll ← WebRTC/transport (Daily.co)
    ├── sioclient_tls.dll, krisp-curl-wss-wrapper.dll
    ├── aec-sdk.dll           ← echo cancellation (`krips_aec::AEC::create(Config)`)
    └── libmp3lame.dll
KrispHelperService.exe (11 MB)  ← Windows service, .NET single-file
                                   ASP.NET Core Kestrel + gRPC over Named Pipes
Krisp Audio Driver (SysVAD turunan, KMDF 1.15, PortCls/AVStream)
    KrispVUSB.sys  (virtual USB bus, HWID `KrispVUSB\KrispVad`)
    KrispAudio.sys (wave miniport: "Krisp Speaker" render + "Krisp Microphone" capture)
```

Alur audio (disimpulkan dari import `kr-media-sp.node`: `MFPlat.dll`, `MMDevAPI.dll`, `AVRT.dll`):
mic fisik → WASAPI capture (MF) → NC/AGC/resample → render ke endpoint **Krisp Speaker** → driver loopback → aplikasi meeting merekam dari **Krisp Microphone**. Arah inbound (speaker) dianalogikan dengan model inbound terpisah.

Komunikasi app ↔ service: gRPC (`@grpc/grpc-js`) lewat named pipe (Kestrel `Transport.NamedPipes`). Service ini yang butuh hak admin (driver/registry), dipakai untuk update & operasi elevated (`ELEVATED` flag di bundle).

## 3. Engine DSP/ML (`kr-media-sp.node`)

API N-API (`Krisp::MediaSP::*`): `GlobalInitialize`, `LoadModels`, `CreateGraph` / `StartGraph` / `StopGraph` / `DeleteGraph` / `ChangeGraphSettings`, `Suspend/ResumeProcessor`, `CreateRecording`, `CreateTranslation`, `CreateRtcTranscriber`, `Start/StopASRProcessing`, `Start/StopDiarization`, `RedactAudioToMp3`.

C API internal "KMS" (`KMSStream::Create*Processor`): `CreateNCProcessor(ncModelPath, rdModelPath, rdThreshold)`, `CreateRDProcessor`, `CreateARProcessor`.

Kelas pipeline: `FrameProcessingChain`, `MainCleaner`, `DfProcessor`, `NoiseCanceller`, `Resampler` (libsamplerate: `src_process`), `KrOpusEncoder/Decoder`, `Capturer`/`Renderer` (frame cache + drop strategy), `AsyncMediaRecorder`, `AsyncDiarizer`, `RingtoneConfigLoader`, `VadV3/VadV4`, `DiarConfigReader`.

Inference: **ONNX Runtime** statis (Conan build `onnxr36ce044dac6a7`), execution provider yang dikompilasi: `CPU`, `XNNPACK`, `OpenVINO`, `DML`, `QNN`, `SNPE`, `WEBNN`, `Azure`. Ada opsi `forceCpuInference` dan `useOpenVINOModel`.

Model noise-cancellation = **DeepFilterNet** (string `isDeepFilterNet: true`, `DfProcessor`, file `df.<id>.ort`). Ini arsitektur open-source, jadi bagian ini bisa direproduksi legal dengan crate Rust `deep_filter_net`/`df`.

## 4. Format model `.kef`

```
offset 0   : 40 hex (fileUid / sha1)
offset 40  : 4 byte pad/nol, lalu uint32 ×2 (offset & size blok JSON?)
berikutnya : JSON manifest
[ {fileUid, name, fileName, size, encrypted}, … ]  ← tabel file
berikutnya : payload mentah
```
Manifest contoh (`c8.f.m.528d1f-1.0.1_1.0.kef`):

```json
{"frameProcessorConfig":{"name":"fpConfig","isDeepFilterNet":true,"processorType":"nc", ...},
 "inferConfigs":[{"name":"inferConfig","deviceToRun":"CPU","engType":"ONNX",
                  "files":[{"name":"onnx","fileName":"df.c8.f.m.528d1f.ort","encrypted":true}]}]}
```

`engType`: `ONNX` (model saraf, terenkripsi) atau `NATIVE` (`m.thw`, weight proprietary, tidak terenkripsi). `encrypted: true` → payload dienkripsi; kunci ada di dalam `kr-media-sp.node`.

## 5. Katalog model (`resources/models/audio/models.json`)

| name | file | workRate | performance | fungsi |
|---|---|---|---|---|
| `nc-inbound-mwb` | `c8.w.m.d15e29` | 16000 | 50 | NC arah masuk (wideband) |
| `nc-outbound-mfb` | `c8.f.m.528d1f` | 32000 | 50 | NC keluar, fullband medium |
| `nc-outbound-sfb` | `c8.f.s.17aa82` | 32000 | 15 | NC keluar, small/fast |
| `nc-outbound-snb` | `c5.n.s.20949d` | 8000 | 10 | NC keluar, narrowband |
| `hvc-outbound-lfb` | `hs.c6.f.m.75df8f.berlin` | 32000 | 75 | pembatalan **suara manusia** |
| `hvc-outbound-sfb` | `hs.c6.f.s.de56df.bucharest` | 32000 | 15 | idem, varian kecil |
| `rd-inbound-wb` | `c6.b78b81` | 16000 | 0 | ringtone detector (`processorType: rtd`) |
| `diar` | `diar_2.0.0` | — | — | diarisasi pembicara |
| `asr`, `ar-*` | tidak dikirim | — | — | diunduh runtime (ASR + accent reduction) |

Ada auto-switching model berdasarkan beban CPU (`AudioService:handleModelAutoSwitching`, `performance` = biaya relatif).

## 6. Fitur lain yang terlihat di bundle

- Fitur ber-flag: `FEATURE_NC_OUT`, `FEATURE_DRIVERLESS_CAPTURE`, `FEATURE_EXT_HUB`, `FEATURE_TTS_DETECTION`, `FEATURE_AC` (accent conversion: `mel2PpgInferProcess` + `vocoderInferProcess` + Albert tokenizer untuk ASR).
- Rekaman: `CreateRecording` + `libmp3lame` (`RedactAudioToMp3Napi`), PII redaction (`pii-redaction-tone.wav`).
- Telemetri: Sentry, Amplitude, OpenTelemetry. Endpoint: `wss://websocket.krisp.ai`, `wss://dev.media-gateway.krisp.ai`, `https://dev.files.krisp.ai`, `stage-kr-files-cdn.s3.amazonaws.com`.
- Env var debug rekaman stream: `KRISP_MIC_STREAM_REC`, `KRISP_SPK_STREAM_REC`, `CAPTURE_MIC_AEC`, dll.
- Auto-update: downloader + eksekusi MSI elevated lewat service; cek sertifikat paket.

## 7. Bukti & batas

- Hash kunci: `kr-media-sp.node` `5B835207…94E3F`, `kr-audio-dm.node` `B126A265…DFB8F`, `aec-sdk.dll` `5230FF17…676BF`, `KrispHelperService.exe` `56F6621C…2998D`, `KrispAudio.sys` `F6181445…80D95A`, `KrispVUSB.sys` `72D5191C…C1BBA`.
- String/kelas berasal dari tabel string biner (statis, tidak dieksekusi).
- **Belum diverifikasi**: perilaku runtime, isi terenkripsi payload `.ort`, algoritma enkripsi `.kef`, isi penuh `BootstrapApp-*.js` (identifier diobfuscate), detail protobuf gRPC helper service.
- Analisis JS penuh via REA timeout (6 MB bundle terobfuscate); fakta diambil dari tabel string + `package.json`.
