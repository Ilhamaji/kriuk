# Blueprint Remake di Rust

Berdasarkan `REPORT.md`. Prinsip: tiru **arsitektur** (capture → DSP → render ke device virtual), jangan tiru **model/format terenkripsi**.

## 1. Yang realistis dipakai ulang

| Komponen Krisp | Pengganti Rust | Catatan |
|---|---|---|
| NC engine (DeepFilterNet + ONNX) | crate `deep_filter_net` / `df` (Rust, MIT/Apache-2.0) atau `ort` + model `.onnx` sendiri | Krisp pakai arsitektur DFN; model mereka terenkripsi & proprietary. Latih/ambil model DFN publik. |
| WASAPI capture/render (MFPlat, MMDevAPI, AVRT) | crate `wasapi` (mode exclusive/shared, loopback) + `windows` (windows-rs) | `AVRT` = `AvSetMmThreadCharacteristics("Pro Audio")` untuk prioritas realtime. |
| Resampler (libsamplerate) | `rubato` | 48k ⇄ 16k/32k sebelum inferensi. |
| AEC (`aec-sdk.dll`) | `webrtc-audio-processing` (binding) atau `speexdsp` | Bisa juga `webrtc-audio-processing-sys`. |
| Opus encode/decode | `audiopus` / `opus` | Untuk rekaman/stream. |
| MP3 rekaman | `lame` / `mp3lame-encoder` | |
| Virtual device (SysVAD driver) | **jangan bikin driver di v1** | Pakai `VB-Audio Cable`/`VoiceMeeter` sebagai sink. Driver KMDF = berbulan-bulan + signing. |
| Tray UI (Electron) | `tauri` v2 atau `egui` + `tray-icon` | Tauri paling dekat dengan UX tray popover. |
| gRPC helper service (.NET) | tidak perlu | Di remake, tidak ada komponen yang butuh admin. |

## 2. Alur audio yang ditiru

```
mic fisik ──wasapi capture 48k/10ms──► ring buffer
   └─► [opsional AEC: far-end dari loopback speaker]
   └─► resample 48k → 32k (outbound) | 16k (inbound)
   └─► DFN infer (hop 10 ms) → mask → enhance
   └─► resample → 48k ──wasapi render──► "CABLE Input"
                                        aplikasi meeting pilih "CABLE Output"
```

Inbound (speaker) = capture loopback dari device output, proses model inbound, render ke output fisik.

## 3. Struktur crate

```
kriuk/
├── Cargo.toml            # workspace
├── crates/
│   ├── audio-io/         # wasapi capture/render, device enum, volume (pengganti kr-audio-dm)
│   ├── dsp/              # resample, AGC, VAD, AEC glue
│   ├── nc/               # DFN / ort inference, model registry (models.json-like)
│   ├── pipeline/         # graph: node chain, frame cache + drop strategy (pengganti FrameProcessingChain)
│   └── ui/               # tauri app + tray
└── src/main.rs
```

Urutan kerja (tiap langkah bisa didemokan):

1. **Passthrough** — capture mic → render ke CABLE Input. Buktikan meeting app dengar suara.
2. **Offline denoise** — `df` proses file WAV; ukur SNR/RTF. Ini inti nilai produk.
3. **Realtime** — pipeline frame 10 ms, thread `Pro Audio`, ring buffer, drop policy saat overload.
4. **Inbound** — loopback capture → model inbound → render.
5. **Tray UI** — toggle, pilih device, indikator level.
6. **Rekaman** — WAV/Opus/MP3 + timestamp.

## 4. Keputusan desain penting

- **Frame size**: 10 ms (480 sampel @48k). Semua buffer, latency target, dan model hop mengikuti ini. Krisp pakai frame cache + `ProcessingDelayStrategy` karena model kadang >10 ms — tiru: buffer N frame, laporkan delay ke user.
- **Sample rate kerja**: pisahkan `workRate` per model (8k/16k/32k) seperti `models.json`. Satu field `work_rate` di registry model, resample di depan inferensi.
- **Auto model switching**: pilih model berdasarkan `min_rate`/`max_rate` + `performance` (biaya). Sederhanakan: satu model default per arah + satu "small" untuk CPU lemah.
- **Tidak perlu** gRPC, driver KMDF, named pipe, Sentry, akun/telemetri untuk versi pertama. Itu semua kebutuhan produk komersial Krisp, bukan kebutuhan fungsi.
- **Kalau nanti butuh device sendiri**: SysVAD sample Microsoft (yang dipakai Krisp) adalah titik awal; butuh EV cert + attestation signing. Tunda sampai v2.

## 5. Legal

- Arsitektur, konsep, dan API publik: aman dipelajari/ditiru.
- Model `.kef` terenkripsi, kode `kr-media-sp.node`, dan key enkripsi: **jangan** didekripsi/dipakai ulang untuk produk.
- DeepFilterNet asli MIT/Apache-2.0 → pakai versi upstream, bukan model Krisp.
