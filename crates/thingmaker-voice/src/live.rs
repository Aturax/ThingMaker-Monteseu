//! Bounded offline previews. The capture thread never performs model inference.
use crate::{Control, recognizer, transcribe};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

pub const PREVIEW_SECONDS: usize = 2;
pub const WINDOW_SECONDS: usize = 16;

pub(crate) struct Job {
    pub audio: Vec<f32>,
    pub start: usize,
    pub final_pass: bool,
}
pub(crate) struct Response {
    pub text: Result<String, String>,
    pub final_pass: bool,
}

pub(crate) struct Worker {
    sender: Option<mpsc::SyncSender<Job>>,
    pub results: mpsc::Receiver<Response>,
    busy: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(folder: &Path, rate: u32, control: Arc<Control>) -> Result<Self, String> {
        // Load before recording, then move this single recognizer to its owner thread.
        let model = recognizer(folder)?;
        let mut rolling = RollingText::default();
        Self::spawn(control, move |job| {
            let text = transcribe(&model, rate, &job.audio)?;
            if job.final_pass {
                return Ok(text);
            }
            Ok(rolling.update(job.start, job.start + job.audio.len(), text))
        })
    }
    fn spawn(
        control: Arc<Control>,
        mut decode: impl FnMut(Job) -> Result<String, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (sender, requests) = mpsc::sync_channel::<Job>(1);
        let (results_tx, results) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        let active = busy.clone();
        let thread = thread::Builder::new()
            .name("voice-inference".into())
            .spawn(move || {
                while let Ok(job) = requests.recv() {
                    if control.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let final_pass = job.final_pass;
                    if !final_pass && control.stop.load(Ordering::Acquire) {
                        active.store(false, Ordering::Release);
                        continue;
                    }
                    let text = decode(job);
                    active.store(false, Ordering::Release);
                    if !control.cancel.load(Ordering::Acquire)
                        && (final_pass || !control.stop.load(Ordering::Acquire))
                        && results_tx.send(Response { text, final_pass }).is_err()
                    {
                        break;
                    }
                    if final_pass || control.cancel.load(Ordering::Acquire) {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender: Some(sender),
            results,
            busy,
            thread: Some(thread),
        })
    }
    /// Only one preview may be in flight. A slow model cannot build up a backlog.
    pub fn preview(&self, audio: &[f32], rate: u32) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let start = audio.len().saturating_sub(rate as usize * WINDOW_SECONDS);
        let job = Job {
            audio: audio[start..].to_vec(),
            start,
            final_pass: false,
        };
        if self.sender.as_ref().unwrap().try_send(job).is_err() {
            self.busy.store(false, Ordering::Release);
            return false;
        }
        true
    }
    pub fn finish(&self, audio: Vec<f32>) -> Result<String, String> {
        self.sender
            .as_ref()
            .unwrap()
            .send(Job {
                audio,
                start: 0,
                final_pass: true,
            })
            .map_err(|_| "Voice inference stopped unexpectedly.")?;
        // Ignore any preview that finished while the microphone was stopping.
        while let Ok(result) = self.results.recv() {
            if result.final_pass {
                return result.text;
            }
        }
        Err("Voice inference stopped unexpectedly.".into())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Default)]
struct RollingText {
    committed: String,
    tail: String,
    end: usize,
}
impl RollingText {
    fn update(&mut self, start: usize, end: usize, text: String) -> String {
        if start == 0 {
            self.committed.clear();
            self.tail = text;
            self.end = end;
        } else if !text.is_empty() {
            // Match only against the previous audio window. Searching the full
            // history can align a repeated sentence to an earlier occurrence.
            let (prefix, tail) = if start >= self.end {
                (self.tail.clone(), text)
            } else {
                split_overlap(&self.tail, &text)
            };
            self.committed = join_text(&self.committed, &prefix);
            self.tail = tail;
            self.end = end;
        }
        join_text(&self.committed, &self.tail)
    }
}
fn join_text(first: &str, second: &str) -> String {
    [first, second]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Split the previous tail into a committed prefix and a rewritten live tail.
/// The final full-audio pass does not use this provisional merge.
fn split_overlap(previous: &str, next: &str) -> (String, String) {
    let old: Vec<_> = previous.split_whitespace().collect();
    let new: Vec<_> = next.split_whitespace().collect();
    let offset = old.len().saturating_sub(160);
    let normalize = |word: &str| {
        word.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let a: Vec<_> = old[offset..].iter().map(|w| normalize(w)).collect();
    let b: Vec<_> = new.iter().map(|w| normalize(w)).collect();
    let mut best = (0, 0, 0); // length, old start, new start
    for i in 0..a.len() {
        for j in 0..b.len() {
            let length = a[i..]
                .iter()
                .zip(&b[j..])
                .take_while(|(x, y)| !x.is_empty() && x == y)
                .count();
            if length > best.0 || (length == best.0 && i > best.1) {
                best = (length, i, j);
            }
        }
    }
    if best.0 >= 2 || (best.0 == 1 && offset + best.1 == old.len() - 1 && best.2 == 0) {
        (old[..offset + best.1].join(" "), new[best.2..].join(" "))
    } else {
        // An uncertain provisional boundary never deletes earlier text. The
        // final pass independently re-decodes all audio, correcting this seam.
        (previous.into(), next.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    fn control() -> Arc<Control> {
        Arc::new(Control {
            id: "test".into(),
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        })
    }
    #[test]
    fn overlap_rewrites_the_recent_tail_without_repeating_shared_words() {
        let merge_overlap = |a: &str, b: &str| {
            let (prefix, tail) = split_overlap(a, b);
            join_text(&prefix, &tail)
        };
        assert_eq!(
            merge_overlap(
                "Primero revisa el archivo. No cambies quince",
                "el archivo. No cambies cincuenta todavía."
            ),
            "Primero revisa el archivo. No cambies cincuenta todavía."
        );
        assert_eq!(
            merge_overlap(
                "Keep it. DO NOT change the file",
                "do not change the file, please."
            ),
            "Keep it. do not change the file, please."
        );
        assert_eq!(
            merge_overlap("no no cambies", "no cambies el límite"),
            "no no cambies el límite"
        );
        assert_eq!(
            merge_overlap("first phrase", "unrelated words"),
            "first phrase unrelated words"
        );
    }
    #[test]
    fn repeated_sentences_do_not_erase_committed_history() {
        let mut rolling = RollingText::default();
        rolling.update(
            0,
            16,
            "Start. Red green blue. Keep this sentence. Tail.".into(),
        );
        rolling.update(2, 18, "green blue. Keep this sentence. Tail. More.".into());
        rolling.update(
            4,
            20,
            "Keep this sentence. Tail. More. Start. Red green blue.".into(),
        );
        let text = rolling.update(
            6,
            22,
            "Tail. More. Start. Red green blue. Keep this sentence.".into(),
        );
        assert_eq!(
            text,
            "Start. Red green blue. Keep this sentence. Tail. More. Start. Red green blue. Keep this sentence."
        );
    }
    #[test]
    fn a_sentence_repeated_after_a_silent_gap_is_not_deduplicated() {
        let mut rolling = RollingText::default();
        rolling.update(0, 16, "Do not change it.".into());
        assert_eq!(rolling.update(20, 36, String::new()), "Do not change it.");
        assert_eq!(
            rolling.update(40, 56, "Do not change it.".into()),
            "Do not change it. Do not change it."
        );
    }
    #[test]
    fn a_slow_preview_is_bounded_and_final_pass_uses_all_audio() {
        let control = control();
        let started = Arc::new(Barrier::new(2));
        let barrier = Arc::new(Barrier::new(2));
        let decode_barrier = barrier.clone();
        let decode_started = started.clone();
        let worker = Worker::spawn(control.clone(), move |job| {
            if !job.final_pass {
                decode_started.wait();
                decode_barrier.wait();
            }
            Ok(format!("{}:{}", job.start, job.audio.len()))
        })
        .unwrap();
        let audio = vec![0.; 40];
        assert!(worker.preview(&audio, 1));
        assert!(!worker.preview(&audio, 1));
        started.wait();
        control.stop.store(true, Ordering::Release);
        barrier.wait();
        assert_eq!(worker.finish(audio).unwrap(), "0:40");
    }
    #[test]
    fn cancellation_suppresses_a_preview_that_was_already_decoding() {
        let control = control();
        let started = Arc::new(Barrier::new(2));
        let finished = Arc::new(Barrier::new(2));
        let a = started.clone();
        let b = finished.clone();
        let worker = Worker::spawn(control.clone(), move |_| {
            a.wait();
            b.wait();
            Ok("late".into())
        })
        .unwrap();
        assert!(worker.preview(&[0.; 4], 1));
        started.wait();
        control.cancel.store(true, Ordering::Release);
        finished.wait();
        assert!(worker.results.recv().is_err());
    }

    #[test]
    #[ignore = "opt-in local model benchmark; set THINGMAKER_VOICE_MODEL and THINGMAKER_VOICE_WAV"]
    fn synthetic_two_minute_recording_has_bounded_previews_and_an_independent_final() {
        let folder = std::path::PathBuf::from(
            std::env::var("THINGMAKER_VOICE_MODEL").expect("model directory"),
        );
        let path = std::env::var("THINGMAKER_VOICE_WAV").expect("short WAV containing speech");
        let wav = sherpa_onnx::Wave::read(&path).unwrap();
        let rate = wav.sample_rate() as u32;
        let samples: Vec<_> = wav
            .samples()
            .iter()
            .copied()
            .cycle()
            .take(rate as usize * 120)
            .collect();
        let model = recognizer(&folder).unwrap();
        let reference = transcribe(&model, rate, &samples).unwrap();
        drop(model);
        let control = control();
        let worker = Worker::new(&folder, rate, control.clone()).unwrap();
        let mut last_preview = String::new();
        for seconds in (2..=120).step_by(2) {
            let started = std::time::Instant::now();
            assert!(worker.preview(&samples[..seconds * rate as usize], rate));
            let result = worker
                .results
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            let text = result.text.unwrap();
            assert!(!text.is_empty());
            println!(
                "audio_seconds={seconds} preview_ms={} characters={}",
                started.elapsed().as_millis(),
                text.len()
            );
            last_preview = text;
        }
        control.stop.store(true, Ordering::Release);
        let started = std::time::Instant::now();
        let final_text = worker.finish(samples).unwrap();
        assert_eq!(final_text, reference);
        assert!(
            last_preview.len() >= reference.len() * 3 / 4,
            "long preview lost its earlier text"
        );
        println!(
            "final_ms={} final_characters={}",
            started.elapsed().as_millis(),
            final_text.len()
        );
    }
}
