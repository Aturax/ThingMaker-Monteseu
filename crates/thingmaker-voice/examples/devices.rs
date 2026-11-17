//! Enumerate native inputs without opening a stream or recording audio.
fn main() {
    match thingmaker_voice::devices() {
        Ok(devices) => {
            for device in devices {
                println!("{}: {}", device.id, device.name);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
