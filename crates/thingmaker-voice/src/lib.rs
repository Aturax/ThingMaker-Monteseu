//! The microphone and model stay native. No audio is sent to a provider or written to disk.
//! One bounded recording at a time; stopping releases capture before inference.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const MAX_SECONDS: u32 = 120;
const ARCHIVE_BYTES: u64 = 487_211_443;
const ARCHIVE_SHA: &str = "1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9";
const URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2";
const FILES: [&str; 4] = [
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Preparing,
    Recording { seconds: f32, level: f32 },
    Transcribing,
    Done { text: String },
    Cancelled,
    Error { message: String },
}
pub type Emit = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
    pub phase: &'static str,
}
#[derive(Serialize)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
}

struct Control {
    id: String,
    stop: AtomicBool,
    cancel: AtomicBool,
}
pub struct Service {
    folder: PathBuf,
    active: Arc<Mutex<Option<Arc<Control>>>>,
    downloading: AtomicBool,
    cancel_download: AtomicBool,
}

impl Service {
    pub fn new(data: &Path) -> Self {
        Self {
            folder: data.join("voice/parakeet-v3-int8"),
            active: Arc::default(),
            downloading: AtomicBool::new(false),
            cancel_download: AtomicBool::new(false),
        }
    }
    pub fn ready(&self) -> bool {
        model_ready(&self.folder)
    }
    pub fn cancel_download(&self) {
        self.cancel_download.store(true, Ordering::Release);
    }
    pub fn install(&self, progress: impl Fn(DownloadProgress)) -> Result<(), String> {
        if self.downloading.swap(true, Ordering::AcqRel) {
            return Err("A voice model download is already running.".into());
        }
        self.cancel_download.store(false, Ordering::Release);
        let result = self.install_inner(progress);
        self.downloading.store(false, Ordering::Release);
        result
    }
    fn install_inner(&self, progress: impl Fn(DownloadProgress)) -> Result<(), String> {
        if self.ready() {
            return Ok(());
        }
        let parent = self.folder.parent().ok_or("Invalid model folder")?;
        fs::create_dir_all(parent).map_err(err)?;
        let stage = tempfile::tempdir_in(parent).map_err(err)?;
        let mut archive_file = tempfile::tempfile_in(parent).map_err(err)?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(err)?;
        let mut response = client
            .get(URL)
            .send()
            .map_err(err)?
            .error_for_status()
            .map_err(err)?;
        let mut hash = Sha256::new();
        let mut received = 0;
        let mut buffer = [0u8; 64 * 1024];
        let mut last = Instant::now();
        loop {
            if self.cancel_download.load(Ordering::Acquire) {
                return Err("Voice model download cancelled.".into());
            }
            let n = response.read(&mut buffer).map_err(err)?;
            if n == 0 {
                break;
            }
            received += n as u64;
            if received > ARCHIVE_BYTES {
                return Err("Voice model download exceeded its expected size.".into());
            }
            hash.update(&buffer[..n]);
            archive_file.write_all(&buffer[..n]).map_err(err)?;
            if last.elapsed() >= Duration::from_millis(150) {
                progress(DownloadProgress {
                    received,
                    total: ARCHIVE_BYTES,
                    phase: "downloading",
                });
                last = Instant::now();
            }
        }
        if received != ARCHIVE_BYTES || format!("{:x}", hash.finalize()) != ARCHIVE_SHA {
            return Err("Voice model integrity check failed. Download it again.".into());
        }
        progress(DownloadProgress {
            received,
            total: ARCHIVE_BYTES,
            phase: "verifying",
        });
        use std::io::{Seek, SeekFrom};
        archive_file.seek(SeekFrom::Start(0)).map_err(err)?;
        let mut archive = tar::Archive::new(bzip2::read::BzDecoder::new(archive_file));
        let mut found = Vec::new();
        let mut expanded = 0u64;
        for entry in archive.entries().map_err(err)? {
            if self.cancel_download.load(Ordering::Acquire) {
                return Err("Voice model download cancelled.".into());
            }
            let mut entry = entry.map_err(err)?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry.path().map_err(err)?.into_owned();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| FILES.contains(n))
            else {
                continue;
            };
            if found.iter().any(|n| n == name) {
                return Err("Duplicate voice model file.".into());
            }
            expanded += entry.size();
            if expanded > 800 * 1024 * 1024 {
                return Err("Voice model unpacked size exceeded.".into());
            }
            // Only fixed leaf names are written, never archive-supplied paths or links.
            let mut file = fs::File::create(stage.path().join(name)).map_err(err)?;
            std::io::copy(&mut entry, &mut file).map_err(err)?;
            found.push(name.to_string());
        }
        if found.len() != FILES.len() {
            return Err("Voice model download is incomplete.".into());
        }
        fs::write(stage.path().join("verified.sha256"), ARCHIVE_SHA).map_err(err)?;
        // A failed earlier installation never becomes visible as ready.
        if self.folder.exists() {
            return Err(
                "An incomplete voice model folder exists. Remove it before downloading again."
                    .into(),
            );
        }
        fs::rename(stage.path(), &self.folder).map_err(err)?;
        progress(DownloadProgress {
            received,
            total: ARCHIVE_BYTES,
            phase: "ready",
        });
        Ok(())
    }
    pub fn start(&self, id: String, device: Option<String>, emit: Emit) -> Result<(), String> {
        if !self.ready() {
            return Err("Download the voice model in Settings first.".into());
        }
        if id.is_empty() || id.len() > 100 {
            return Err("Invalid dictation id.".into());
        }
        let control = Arc::new(Control {
            id,
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        });
        {
            let mut slot = self.active.lock().map_err(err)?;
            if slot.is_some() {
                return Err("Another dictation is already running.".into());
            }
            *slot = Some(control.clone());
        }
        let folder = self.folder.clone();
        let active = self.active.clone();
        let spawned = std::thread::Builder::new()
            .name("voice-dictation".into())
            .spawn(move || {
                let result = capture(&folder, device, &control, &emit);
                if let Ok(mut slot) = active.lock() {
                    *slot = None;
                }
                if control.cancel.load(Ordering::Acquire) {
                    emit(Event::Cancelled);
                } else {
                    match result {
                        Ok(text) => emit(Event::Done { text }),
                        Err(message) => emit(Event::Error { message }),
                    }
                }
            });
        if let Err(e) = spawned {
            if let Ok(mut slot) = self.active.lock() {
                *slot = None;
            }
            return Err(err(e));
        }
        Ok(())
    }
    pub fn stop(&self, id: &str, cancel: bool) {
        if let Ok(slot) = self.active.lock() {
            if let Some(c) = slot.as_ref().filter(|c| c.id == id) {
                if cancel {
                    c.cancel.store(true, Ordering::Release);
                }
                c.stop.store(true, Ordering::Release);
            }
        }
    }
    pub fn cancel_capture(&self) {
        if let Ok(slot) = self.active.lock() {
            if let Some(c) = slot.as_ref() {
                c.cancel.store(true, Ordering::Release);
                c.stop.store(true, Ordering::Release);
            }
        }
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn model_ready(path: &Path) -> bool {
    fs::read_to_string(path.join("verified.sha256")).is_ok_and(|s| s == ARCHIVE_SHA)
        && FILES
            .iter()
            .all(|f| fs::metadata(path.join(f)).is_ok_and(|m| m.is_file() && m.len() > 0))
}
pub fn recognizer(folder: &Path) -> Result<OfflineRecognizer, String> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer.encoder =
        Some(folder.join(FILES[0]).to_string_lossy().into_owned());
    config.model_config.transducer.decoder =
        Some(folder.join(FILES[1]).to_string_lossy().into_owned());
    config.model_config.transducer.joiner =
        Some(folder.join(FILES[2]).to_string_lossy().into_owned());
    config.model_config.tokens = Some(folder.join(FILES[3]).to_string_lossy().into_owned());
    config.model_config.model_type = Some("nemo_transducer".into());
    config.model_config.provider = Some("cpu".into());
    config.model_config.num_threads = 4;
    OfflineRecognizer::create(&config).ok_or_else(|| "Could not load the voice model.".into())
}
pub fn transcribe(
    recognizer: &OfflineRecognizer,
    rate: u32,
    samples: &[f32],
) -> Result<String, String> {
    if samples.is_empty() || samples.iter().all(|x| x.abs() < 0.00001) {
        return Ok(String::new());
    }
    let stream = recognizer.create_stream();
    // sherpa's feature pipeline resamples from the device rate to the model rate.
    stream.accept_waveform(rate as i32, samples);
    recognizer.decode(&stream);
    stream
        .get_result()
        .map(|r| r.text.trim().to_string())
        .ok_or_else(|| "Voice transcription failed.".into())
}
pub fn devices() -> Result<Vec<InputDevice>, String> {
    cpal::default_host()
        .input_devices()
        .map_err(err)?
        .map(|d| {
            Ok(InputDevice {
                id: d.id().map_err(err)?.to_string(),
                name: d.to_string(),
            })
        })
        .collect()
}

fn capture(
    folder: &Path,
    device: Option<String>,
    control: &Control,
    emit: &Emit,
) -> Result<String, String> {
    emit(Event::Preparing);
    let model = recognizer(folder)?;
    if control.cancel.load(Ordering::Acquire) || control.stop.load(Ordering::Acquire) {
        return Ok(String::new());
    }
    let host = cpal::default_host();
    let input = if let Some(id) = device.filter(|s| !s.is_empty()) {
        host.input_devices()
            .map_err(err)?
            .find(|d| d.id().is_ok_and(|value| value.to_string() == id))
    } else {
        host.default_input_device()
    }
    .ok_or("Microphone not available. Check your input device and microphone permissions.")?;
    let supported = input.default_input_config().map_err(err)?;
    let config = supported.config();
    let rate = config.sample_rate;
    let channels = config.channels as usize;
    if !(8000..=192000).contains(&rate) || channels == 0 || channels > 8 {
        return Err("Unsupported microphone format.".into());
    }
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);
    let failed = Arc::new(AtomicBool::new(false));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            input_stream::<f32>(&input, config, channels, tx, failed.clone())
        }
        cpal::SampleFormat::I16 => {
            input_stream::<i16>(&input, config, channels, tx, failed.clone())
        }
        cpal::SampleFormat::U16 => {
            input_stream::<u16>(&input, config, channels, tx, failed.clone())
        }
        cpal::SampleFormat::I32 => {
            input_stream::<i32>(&input, config, channels, tx, failed.clone())
        }
        _ => return Err("Unsupported microphone sample format.".into()),
    }?;
    stream.play().map_err(err)?;
    let started = Instant::now();
    let max = rate as usize * MAX_SECONDS as usize;
    let mut audio = Vec::with_capacity(max);
    let mut last = Instant::now();
    emit(Event::Recording {
        seconds: 0.,
        level: 0.,
    });
    while !control.stop.load(Ordering::Acquire)
        && started.elapsed().as_secs() < MAX_SECONDS as u64
        && audio.len() < max
    {
        if failed.load(Ordering::Acquire) {
            return Err(
                "Microphone disconnected or audio capture overflowed. Please try again.".into(),
            );
        }
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
            let level = chunk.iter().fold(0f32, |peak, x| peak.max(x.abs()));
            append_bounded(&mut audio, &chunk, max);
            if last.elapsed() >= Duration::from_millis(100) {
                emit(Event::Recording {
                    seconds: started.elapsed().as_secs_f32(),
                    level,
                });
                last = Instant::now();
            }
        }
    }
    drop(stream);
    for chunk in rx.try_iter() {
        append_bounded(&mut audio, &chunk, max);
    }
    if failed.load(Ordering::Acquire) {
        return Err("Audio capture failed. Please try again.".into());
    }
    if control.cancel.load(Ordering::Acquire) {
        return Ok(String::new());
    }
    emit(Event::Transcribing);
    transcribe(&model, rate, &audio)
}
fn append_bounded(audio: &mut Vec<f32>, chunk: &[f32], max: usize) {
    let count = chunk.len().min(max.saturating_sub(audio.len()));
    audio.extend_from_slice(&chunk[..count]);
}
fn input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    tx: mpsc::SyncSender<Vec<f32>>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let errors = failed.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data, _| {
                let mono: Vec<f32> = data
                    .chunks_exact(channels)
                    .map(|frame| {
                        frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                    })
                    .collect();
                if tx.try_send(mono).is_err() {
                    failed.store(true, Ordering::Release);
                }
            },
            move |_| {
                errors.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_is_bounded_even_when_stop_has_pending_frames() {
        let mut samples = vec![1., 2.];
        append_bounded(&mut samples, &[3., 4., 5.], 4);
        append_bounded(&mut samples, &[6.], 4);
        assert_eq!(samples, vec![1., 2., 3., 4.]);
    }
    #[test]
    fn unverified_download_is_never_ready() {
        let dir = tempfile::tempdir().unwrap();
        for name in FILES {
            fs::write(dir.path().join(name), b"partial").unwrap();
        }
        assert!(!model_ready(dir.path()));
    }
    #[test]
    fn stale_stop_cannot_cancel_another_recording() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path());
        let c = Arc::new(Control {
            id: "new".into(),
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        });
        *service.active.lock().unwrap() = Some(c.clone());
        service.stop("old", true);
        assert!(!c.stop.load(Ordering::Acquire));
        service.stop("new", true);
        assert!(c.cancel.load(Ordering::Acquire));
    }
}
