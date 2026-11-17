//! Opt-in native capture lifecycle probe. Never saves audio or prints transcripts.
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::Duration,
};
use thingmaker_voice::{Event, Service};
fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    let args: Vec<_> = std::env::args().collect();
    let data = args
        .get(1)
        .expect("capture DATA_DIR [CYCLES] [SECONDS] [mixed|stop] [EXPECTED_TEXT]");
    let cycles: usize = args.get(2).map_or(10, |s| s.parse().unwrap());
    let seconds: f32 = args.get(3).map_or(1., |s| s.parse().unwrap());
    let service = Service::new(Path::new(data));
    let mixed = args.get(4).is_some_and(|s| s == "mixed");
    let mut failures = 0;
    let mut warnings = 0;
    for cycle in 0..cycles {
        let id = format!("probe-{cycle}");
        let (tx, rx) = mpsc::channel();
        service
            .start(
                id.clone(),
                None,
                Arc::new(move |event| {
                    let _ = tx.send(event);
                }),
            )
            .unwrap();
        loop {
            match rx
                .recv_timeout(Duration::from_secs(30))
                .expect("capture timed out")
            {
                Event::Recording {
                    seconds: elapsed, ..
                } if elapsed >= seconds => service.stop(&id, mixed && cycle % 3 == 2),
                Event::Done { text } => {
                    if let Some(expected) = args.get(5) {
                        let matched = normalize(&text).contains(&normalize(expected));
                        println!("cycle={} expected_match={matched}", cycle + 1);
                        if !matched {
                            failures += 1;
                        }
                    }
                    println!("cycle={} ok", cycle + 1);
                    break;
                }
                Event::Error { message } => {
                    failures += 1;
                    println!("cycle={} error={message}", cycle + 1);
                    break;
                }
                Event::Cancelled if mixed && cycle % 3 == 2 => {
                    println!("cycle={} cancelled", cycle + 1);
                    break;
                }
                Event::Warning { .. } => {
                    warnings += 1;
                }
                Event::Cancelled => panic!("unexpected cancellation"),
                _ => {}
            }
        }
    }
    println!("cycles={cycles} failures={failures} warnings={warnings}");
    if failures > 0 {
        std::process::exit(1);
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
