// Diagnostic: prints mic peak levels. Run with:
//   cargo run --release --example peak --manifest-path src-tauri/Cargo.toml
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

fn report(tag: &str, peak: f32) {
    if peak > 0.0005 {
        println!("{tag} peak {peak:.4}");
    }
}

fn main() {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    println!("device: {:?}", device.name());
    let supported = device.default_input_config().expect("no default config");
    println!("format: {:?} rate: {} ch: {}", supported.sample_format(), supported.sample_rate().0, supported.channels());

    let stream = match supported.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &supported.config(),
            |data: &[f32], _| report("f32", data.iter().fold(0f32, |a, b| a.max(b.abs()))),
            |e| eprintln!("stream error: {e}"),
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &supported.config(),
            |data: &[i16], _| report("i16", data.iter().fold(0f32, |a, b| a.max(b.abs() as f32 / 32768.0))),
            |e| eprintln!("stream error: {e}"),
            None,
        ),
        other => panic!("unhandled format {other:?}"),
    }
    .expect("build stream");

    stream.play().expect("play");
    std::thread::sleep(std::time::Duration::from_secs(6));
    println!("done");
}
