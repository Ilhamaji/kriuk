# Kriuk

Tray audio utility untuk Windows. Tauri v2 (Rust + WebView2), UI statis di `src/`.

## Build

Butuh Rust (MSVC toolchain) + VS Build Tools.

```powershell
cargo build --release --manifest-path src-tauri\Cargo.toml
```

Hasil: `src-tauri\target\release\kriuk.exe` (single file, ~6 MB). WebView2 Runtime dipakai dari sistem.

## Yang berjalan

| Fitur | Cara kerja |
|---|---|
| Peredaman mikrofon | RNNoise (`nnnoiseless`) 48 kHz, frame 10 ms, di thread audio WASAPI |
| Tingkat peredaman | Ringan/Seimbang/Maksimal = campuran sinyal bersih 40/75/100% |
| Echo cancellation | PBFDAF (rustfft), 4 partisi × 480 sampel (~40 ms), referensi loopback WASAPI speaker, penjaga double-talk Geigel, 18–33 dB di uji sintetis |
| Equalizer | Biquad RBJ: Netral (bypass), Jelas, Hangat, Auto (ukur pita 300 Hz / 3 kHz tiap 0,5 s lalu menyesuaikan, dibatasi ±4–5 dB) |
| Meter | Peak level dari sinyal yang sudah diproses |
| Kirim hasil ke | Hasil olahan diputar ke perangkat keluaran pilihan (bisa kabel audio virtual) |
| Bahasa | English (default) atau Indonesia, di Setelan, tersimpan di perangkat |
| Tampilan | Terang atau gelap, mengikuti sistem saat pertama dibuka, tersimpan di perangkat |

Urutan pemrosesan: echo cancellation → denoiser → equalizer → meter/monitor.
Rantai ini hidup selama "Tes mic" berjalan; menutup popover menghentikannya. Fitur DSP yang mati tidak menambah biaya: satu atomic load per frame, tanpa thread dan tanpa stream tambahan.

Terukur di laptop uji: 0 % CPU saat idle, ~4,7 % saat rantai penuh + echo cancellation, RAM ~32 MB.

Pilih perangkat keluaran seperti VB-Cable/VoiceMeeter sebagai "Kirim hasil ke", lalu set mikrofon aplikasi lain ke kabel itu: mereka akan menerima suara yang sudah bersih.

## Yang belum bisa

Echo cancellation hanya menghapus suara yang keluar lewat speaker yang dipilih; ia tidak bisa menyisipkan diri ke aplikasi lain tanpa kabel audio virtual. Speaker noise cancellation juga hanya mungkin lewat driver tersebut. Lihat `REMAKE-RUST.md` untuk urutan implementasi DSP.

## Isi

| Path | Fungsi |
|---|---|
| `src/index.html`, `src/styles.css`, `src/app.js` | UI popover 380×780 (di-embed ke exe saat build) |
| `src-tauri/src/main.rs` | tray icon, posisi popover, hide-on-blur, meter mikrofon (cpal/WASAPI), loopback echo |
| `src-tauri/src/aec.rs` | echo canceller PBFDAF + unit test ERLE |
| `src-tauri/src/eq.rs` | equalizer biquad + preset + auto kurva |
| `src-tauri/tauri.conf.json` | window frameless + transparan, `withGlobalTauri` |
| `tools/make-icon.js` | generator `assets/icon.png`, `assets/tray.png`, `assets/icon.ico` |
| `DESIGN.md` | spesifikasi desain |
| `REPORT.md`, `REMAKE-RUST.md` | hasil reverse engineering Krisp + blueprint |
