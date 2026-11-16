//! Opt-in native smoke check with a local model directory and WAV. No microphone or network.
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "transcribe MODEL_DIR WAV");
    let model = thingmaker_voice::recognizer(std::path::Path::new(&args[1])).expect("model");
    let wave = sherpa_onnx::Wave::read(&args[2]).expect("wav");
    println!(
        "{}",
        thingmaker_voice::transcribe(&model, wave.sample_rate() as u32, wave.samples())
            .expect("transcribe")
    );
}
