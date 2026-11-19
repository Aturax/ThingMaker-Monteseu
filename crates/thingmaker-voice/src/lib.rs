//! The microphone and model stay native. No audio is sent to a provider or written to disk.
//! One bounded recording at a time; provisional inference runs away from capture.
mod live;
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
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    Partial { text: String },
    Transcribing,
    Done { text: String },
    Cancelled,
    Warning { message: String },
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
    pub fn remove(&self) -> Result<(), String> {
        let active = self.active.lock().map_err(err)?;
        if active.is_some() {
            return Err("Finish dictating before removing the voice model.".into());
        }
        if self.downloading.swap(true, Ordering::AcqRel) {
            return Err("A voice model download is already running.".into());
        }
        let result = match fs::remove_dir_all(&self.folder) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other.map_err(err),
        };
        self.downloading.store(false, Ordering::Release);
        result
    }
    fn install_inner(&self, progress: impl Fn(DownloadProgress)) -> Result<(), String> {
        if self.ready() {
            return Ok(());
        }
        let parent = self.folder.parent().ok_or("Invalid model folder")?;
        fs::create_dir_all(parent).map_err(err)?;
        if self.folder.exists() {
            fs::remove_dir_all(&self.folder).map_err(err)?;
        }
        if fs2::available_space(parent).map_err(err)? < 1_200_000_000 {
            return Err("Free at least 1.2 GB before downloading the voice model.".into());
        }
        let stage = tempfile::tempdir_in(parent).map_err(err)?;
        let mut archive_file = tempfile::tempfile_in(parent).map_err(err)?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            // The blocking client applies this timeout to each response.read().
            .timeout(Duration::from_secs(60))
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
        fs::rename(stage.path(), &self.folder).map_err(err)?;
        progress(DownloadProgress {
            received,
            total: ARCHIVE_BYTES,
            phase: "ready",
        });
        Ok(())
    }
    pub fn start(&self, id: String, device: Option<String>, emit: Emit) -> Result<(), String> {
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
            if !self.ready() {
                return Err("Download the voice model in Settings first.".into());
            }
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
        if let Ok(slot) = self.active.lock()
            && let Some(c) = slot.as_ref().filter(|c| c.id == id)
        {
            if cancel {
                c.cancel.store(true, Ordering::Release);
            }
            c.stop.store(true, Ordering::Release);
        }
    }
    pub fn cancel_capture(&self) {
        if let Ok(slot) = self.active.lock()
            && let Some(c) = slot.as_ref()
            && !c.stop.load(Ordering::Acquire)
        {
            c.cancel.store(true, Ordering::Release);
            c.stop.store(true, Ordering::Release);
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
    config.model_config.num_threads =
        (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).clamp(1, 4) as i32;
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

#[derive(Default)]
struct CaptureHealth {
    // Preserve the first cause; never format or log on the real-time thread.
    failure: AtomicUsize,
    callbacks: AtomicUsize,
    frames: AtomicUsize,
    max_callback_us: AtomicUsize,
    host_overruns: AtomicUsize,
    device_changed: AtomicBool,
}
impl CaptureHealth {
    fn fail(&self, code: usize) {
        let _ = self
            .failure
            .compare_exchange(0, code, Ordering::AcqRel, Ordering::Acquire);
    }
    fn error(&self) -> Option<String> {
        let cause = match self.failure.load(Ordering::Acquire) {
            0 => return None,
            1 => "The local microphone buffer overflowed. Audio was lost; please record again.",
            3 => "The microphone was disconnected. Reconnect it and record again.",
            4 => "Microphone permission was revoked. Enable microphone access and try again.",
            _ => "The microphone stream failed. Please record again.",
        };
        Some(cause.into())
    }
}

const AUDIO_INTERRUPTED: &str = "The microphone reported an audio interruption. Review the dictation carefully or record it again.";

fn report_stream_error(health: &CaptureHealth, kind: cpal::ErrorKind) {
    match kind {
        // CPAL leaves the stream running for an xrun. Warn; never silently
        // claim the transcript is complete, and do not discard the whole take.
        cpal::ErrorKind::Xrun => {
            health.host_overruns.fetch_add(1, Ordering::Relaxed);
        }
        cpal::ErrorKind::DeviceChanged => health.device_changed.store(true, Ordering::Release),
        cpal::ErrorKind::RealtimeDenied => {}
        cpal::ErrorKind::DeviceNotAvailable => health.fail(3),
        cpal::ErrorKind::PermissionDenied => health.fail(4),
        _ => health.fail(5),
    }
}

fn warn_if_interrupted(health: &CaptureHealth, warned: &mut bool, emit: &Emit) {
    if health.device_changed.swap(false, Ordering::AcqRel) {
        emit(Event::Warning {
            message: "The input device changed during recording. Review the dictation carefully."
                .into(),
        });
    }
    if !*warned && health.host_overruns.load(Ordering::Relaxed) > 0 {
        *warned = true;
        emit(Event::Warning {
            message: AUDIO_INTERRUPTED.into(),
        });
    }
}

/// This is the real-time data callback: no allocation, locks, I/O or thread wakeups.
fn push_mono<T>(data: &[T], channels: usize, tx: &mut rtrb::Producer<f32>, health: &CaptureHealth)
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let started = Instant::now();
    if health.failure.load(Ordering::Acquire) != 0 {
        return;
    }
    let frames = data.len() / channels;
    let Ok(mut chunk) = tx.write_chunk(frames) else {
        health.fail(1);
        return;
    };
    let (first, second) = chunk.as_mut_slices();
    for (dst, frame) in first
        .iter_mut()
        .chain(second)
        .zip(data.chunks_exact(channels))
    {
        *dst = frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
    }
    chunk.commit_all();
    health.frames.fetch_add(frames, Ordering::Relaxed);
    health.callbacks.fetch_add(1, Ordering::Relaxed);
    health
        .max_callback_us
        .fetch_max(started.elapsed().as_micros() as usize, Ordering::Relaxed);
}

fn drain_audio(rx: &mut rtrb::Consumer<f32>, audio: &mut Vec<f32>, max: usize) -> f32 {
    let count = rx.slots();
    let chunk = rx
        .read_chunk(count)
        .expect("only this thread consumes audio");
    let (first, second) = chunk.as_slices();
    let level = first
        .iter()
        .chain(second)
        .fold(0f32, |peak, x| peak.max(x.abs()));
    append_bounded(audio, first, max);
    append_bounded(audio, second, max);
    chunk.commit_all();
    level
}

fn capture(
    folder: &Path,
    device: Option<String>,
    control: &Arc<Control>,
    emit: &Emit,
) -> Result<String, String> {
    emit(Event::Preparing);
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
    // One second of mono audio, independent of callback block size. Touch its
    // pages and the bounded final buffer before starting the real-time stream.
    let (mut tx, mut rx) = rtrb::RingBuffer::<f32>::new(rate as usize);
    tx.write_chunk(rate as usize).unwrap().commit_all();
    rx.read_chunk(rate as usize).unwrap().commit_all();
    let max = rate as usize * MAX_SECONDS as usize;
    let mut audio = vec![0.; max];
    audio.clear();
    let worker = live::Worker::new(folder, rate, control.clone())?;
    if control.cancel.load(Ordering::Acquire) || control.stop.load(Ordering::Acquire) {
        return Ok(String::new());
    }
    let health = Arc::new(CaptureHealth::default());
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            input_stream::<f32>(&input, config, channels, tx, health.clone())
        }
        cpal::SampleFormat::I16 => {
            input_stream::<i16>(&input, config, channels, tx, health.clone())
        }
        cpal::SampleFormat::U16 => {
            input_stream::<u16>(&input, config, channels, tx, health.clone())
        }
        cpal::SampleFormat::I32 => {
            input_stream::<i32>(&input, config, channels, tx, health.clone())
        }
        _ => return Err("Unsupported microphone sample format.".into()),
    }?;
    stream.play().map_err(err)?;
    let started = Instant::now();
    let mut last = Instant::now();
    let mut warned = false;
    let mut preview_failed = false;
    let interval = rate as usize * live::PREVIEW_SECONDS;
    let mut next_preview = interval;
    emit(Event::Recording {
        seconds: 0.,
        level: 0.,
    });
    while !control.stop.load(Ordering::Acquire)
        && started.elapsed().as_secs() < MAX_SECONDS as u64
        && audio.len() < max
    {
        if health.failure.load(Ordering::Acquire) != 0 {
            break;
        }
        warn_if_interrupted(&health, &mut warned, emit);
        let level = drain_audio(&mut rx, &mut audio, max);
        while let Ok(result) = worker.results.try_recv() {
            if control.stop.load(Ordering::Acquire) || control.cancel.load(Ordering::Acquire) {
                break;
            }
            match result.text {
                Ok(text) => emit(Event::Partial { text }),
                Err(_) if !preview_failed => {
                    preview_failed = true;
                    emit(Event::Warning { message: "Live preview is unavailable. Stop recording for the final transcription.".into() });
                }
                Err(_) => {}
            }
        }
        if audio.len() >= next_preview && worker.preview(&audio, rate) {
            next_preview = audio.len() + interval;
        }
        if last.elapsed() >= Duration::from_millis(100) {
            emit(Event::Recording {
                seconds: started.elapsed().as_secs_f32(),
                level,
            });
            last = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    control.stop.store(true, Ordering::Release);
    drop(stream);
    drain_audio(&mut rx, &mut audio, max);
    warn_if_interrupted(&health, &mut warned, emit);
    tracing::info!(
        sample_rate = rate,
        channels,
        callbacks = health.callbacks.load(Ordering::Relaxed),
        frames = health.frames.load(Ordering::Relaxed),
        max_callback_us = health.max_callback_us.load(Ordering::Relaxed),
        failure = health.failure.load(Ordering::Acquire),
        host_overruns = health.host_overruns.load(Ordering::Relaxed),
        "voice capture finished"
    );
    if let Some(error) = health.error() {
        return Err(error);
    }
    if control.cancel.load(Ordering::Acquire) {
        return Ok(String::new());
    }
    #[cfg(windows)]
    if !audio.is_empty() && audio.iter().all(|&sample| sample == 0.0) {
        return Err("The microphone returned silent audio. Check the input device and microphone access in Windows Privacy settings.".into());
    }
    emit(Event::Transcribing);
    worker.finish(audio)
}
fn append_bounded(audio: &mut Vec<f32>, chunk: &[f32], max: usize) {
    let count = chunk.len().min(max.saturating_sub(audio.len()));
    audio.extend_from_slice(&chunk[..count]);
}
fn input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    mut tx: rtrb::Producer<f32>,
    health: Arc<CaptureHealth>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let errors = health.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data, _| push_mono(data, channels, &mut tx, &health),
            move |error| report_stream_error(&errors, error.kind()),
            None,
        )
        .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    // Count only allocations on the test's own thread, after setup. Other
    // concurrently running tests cannot make the real-time assertion flaky.
    thread_local! {
        static TRACK: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }
    struct CountingAllocator;
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;
    fn count_allocation() {
        let _ = TRACK.try_with(|track| {
            if track.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
    }
    // SAFETY: Every operation forwards the same pointer/layout to System;
    // the thread-local counters never allocate or change allocator semantics.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count_allocation();
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            count_allocation();
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            count_allocation();
            unsafe { System.realloc(ptr, layout, size) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            count_allocation();
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    #[test]
    fn realtime_callback_never_allocates_even_when_wrapping_or_full() {
        let (mut tx, mut rx) = rtrb::RingBuffer::new(4);
        let health = CaptureHealth::default();
        let mut audio = Vec::with_capacity(16);
        ALLOCATIONS.with(|c| c.set(0));
        TRACK.with(|c| c.set(true));
        push_mono(&[1f32, 0., 0., 1., -1., 1.], 2, &mut tx, &health);
        drain_audio(&mut rx, &mut audio, 16);
        push_mono(&[1i16, 2, 3, 4], 1, &mut tx, &health); // wraps
        push_mono(&[1f32], 1, &mut tx, &health); // full; fail without allocation
        TRACK.with(|c| c.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(health.failure.load(Ordering::Acquire), 1);
        assert_eq!(audio, [0.5, 0.5, 0.]);
    }

    #[test]
    fn host_xrun_warns_once_and_keeps_capturing_following_frames() {
        let health = CaptureHealth::default();
        let (mut tx, mut rx) = rtrb::RingBuffer::new(8);
        push_mono(&[0.1f32, 0.2], 1, &mut tx, &health);
        report_stream_error(&health, cpal::ErrorKind::Xrun);
        report_stream_error(&health, cpal::ErrorKind::Xrun);
        push_mono(&[0.3f32, 0.4], 1, &mut tx, &health);
        let mut audio = Vec::new();
        drain_audio(&mut rx, &mut audio, 8);
        assert_eq!(audio, [0.1, 0.2, 0.3, 0.4]);
        assert!(health.error().is_none());
        let (send, recv) = std::sync::mpsc::channel();
        let emit: Emit = Arc::new(move |e| send.send(e).unwrap());
        let mut warned = false;
        warn_if_interrupted(&health, &mut warned, &emit);
        warn_if_interrupted(&health, &mut warned, &emit);
        assert!(matches!(recv.try_recv(), Ok(Event::Warning { .. })));
        assert!(recv.try_recv().is_err());
    }

    #[test]
    fn real_queue_overflow_and_disconnection_are_fatal_and_distinct() {
        let overflow = CaptureHealth::default();
        let (mut tx, mut rx) = rtrb::RingBuffer::new(2);
        push_mono(&[1f32, 2.], 1, &mut tx, &overflow);
        push_mono(&[3f32], 1, &mut tx, &overflow);
        assert!(
            overflow
                .error()
                .unwrap()
                .contains("local microphone buffer overflowed")
        );
        let mut audio = Vec::new();
        drain_audio(&mut rx, &mut audio, 8);
        assert_eq!(audio, [1., 2.]); // never overwrite queued audio
        let disconnected = CaptureHealth::default();
        report_stream_error(&disconnected, cpal::ErrorKind::DeviceNotAvailable);
        assert!(disconnected.error().unwrap().contains("disconnected"));
        report_stream_error(&disconnected, cpal::ErrorKind::PermissionDenied);
        assert!(disconnected.error().unwrap().contains("disconnected")); // first cause wins
    }
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
    fn remove_only_clears_the_model_and_can_be_retried() {
        let dir = tempfile::tempdir().unwrap();
        let service = Service::new(dir.path());
        fs::create_dir_all(&service.folder).unwrap();
        fs::write(service.folder.join("tokens.txt"), "partial").unwrap();
        let sibling = service.folder.parent().unwrap().join("settings");
        fs::write(&sibling, "keep").unwrap();
        service.downloading.store(true, Ordering::Release);
        assert!(service.remove().is_err());
        assert!(service.folder.exists());
        service.downloading.store(false, Ordering::Release);
        service.remove().unwrap();
        service.remove().unwrap();
        assert!(!service.folder.exists());
        assert!(sibling.is_file());
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
        service.stop("new", false);
        service.cancel_capture();
        assert!(!c.cancel.load(Ordering::Acquire));
        service.stop("new", true);
        assert!(c.cancel.load(Ordering::Acquire));
    }
}
