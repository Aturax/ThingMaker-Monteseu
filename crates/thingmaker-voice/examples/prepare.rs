//! Opt-in test of the real verified download, into a caller-supplied scratch folder.
fn main() {
    let folder = std::env::args().nth(1).expect("prepare SCRATCH_DATA_DIR");
    let service = thingmaker_voice::Service::new(std::path::Path::new(&folder));
    service
        .install(|p| {
            if p.phase != "downloading" {
                println!("{}", p.phase);
            }
        })
        .expect("verified model installation");
    assert!(service.ready());
    println!("Verified model installed and ready");
}
