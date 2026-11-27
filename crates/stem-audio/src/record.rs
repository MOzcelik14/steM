//! Multi-track audio recording engine
//!
//! STRICT INVARIANTS:
//! - Real-time input callback performs NO allocations, NO locks, NO I/O.
//! - Lock-free ringbuffer streams incoming PCM to background recording worker.
//! - Live Peak & RMS input telemetry and software input monitoring.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use thiserror::Error;
use uuid::Uuid;

use stem_core::project::AudioSource;

/// Maximum bounded fallback samples in RAM if disk writing fails mid-take (~10s stereo @ 48 kHz).
const MAX_FALLBACK_SAMPLES: usize = 48_000 * 2 * 10;

#[derive(Error, Debug)]
pub enum RecordError {
    #[error("No audio input device available")]
    NoInputDevice,

    #[error("Selected input device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to query input device configurations: {0}")]
    DeviceConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Failed to build input stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    #[error("Failed to start input stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),

    #[error("Recording disk I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Returns the default persistent directory for in-progress recording takes.
/// Uses `<project_dir>/.stem_recordings` if provided, otherwise OS user data dir.
pub fn default_recording_dir(project_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = project_dir {
        return dir.join(".stem_recordings");
    }
    let base_dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base_dir.join("steM").join(".stem_recordings")
}

/// Scans `dir` for orphaned `stem_rec_*.wav` takes left behind by an unclean exit.
pub fn recover_orphaned_recordings(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("stem_rec_") && name.ends_with(".wav") && p.is_file() {
            if let Ok(meta) = entry.metadata() {
                if meta.len() > 44 {
                    found.push(p);
                }
            }
        }
    }
    found
}

/// Incremental disk WAV writer with periodic header flush so crashes retain valid audio.
pub struct RecordingDiskWriter {
    path: PathBuf,
    writer: hound::WavWriter<BufWriter<File>>,
    samples_since_flush: usize,
    flush_interval_samples: usize,
    has_samples: bool,
}

impl RecordingDiskWriter {
    pub fn new(dir: &Path, spec: hound::WavSpec) -> Result<Self, RecordError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("stem_rec_{}.wav", Uuid::new_v4()));
        let writer = hound::WavWriter::create(&path, spec)
            .map_err(|e| RecordError::Io(std::io::Error::other(e.to_string())))?;
        let flush_interval_samples = (spec.sample_rate as usize)
            .saturating_mul((spec.channels as usize).max(1))
            .max(1);
        Ok(Self {
            path,
            writer,
            samples_since_flush: 0,
            flush_interval_samples,
            has_samples: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_sample(&mut self, sample: f32) -> Result<(), RecordError> {
        self.writer
            .write_sample(sample)
            .map_err(|e| RecordError::Io(std::io::Error::other(e.to_string())))?;
        self.has_samples = true;
        self.samples_since_flush += 1;
        if self.samples_since_flush >= self.flush_interval_samples {
            self.flush()?;
        }
        Ok(())
    }

    /// Flushes buffered PCM and updates WAV chunk sizes on disk.
    pub fn flush(&mut self) -> Result<(), RecordError> {
        self.writer
            .flush()
            .map_err(|e| RecordError::Io(std::io::Error::other(e.to_string())))?;
        self.samples_since_flush = 0;
        Ok(())
    }

    pub fn finalize(self) -> Result<(PathBuf, bool), RecordError> {
        let path = self.path;
        let has_samples = self.has_samples;
        self.writer
            .finalize()
            .map_err(|e| RecordError::Io(std::io::Error::other(e.to_string())))?;
        Ok((path, has_samples))
    }
}

/// Real-time input signal telemetry for level meters.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputTelemetry {
    pub peak_l: f32,
    pub peak_r: f32,
    pub rms_l: f32,
    pub rms_r: f32,
}

impl InputTelemetry {
    #[inline]
    pub fn max_peak(&self) -> f32 {
        self.peak_l.max(self.peak_r)
    }

    #[inline]
    pub fn max_rms(&self) -> f32 {
        self.rms_l.max(self.rms_r)
    }
}

/// Manages hardware audio recording from the system input device.
pub struct AudioRecorder {
    _stream: Stream,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    is_recording: Arc<AtomicBool>,
    monitoring_enabled: Arc<AtomicBool>,
    device_lost: Arc<AtomicBool>,
    write_error: Arc<AtomicBool>,
    overrun_samples: Arc<AtomicU64>,
    telemetry_rx: Consumer<InputTelemetry>,
    latest_telemetry: InputTelemetry,
    worker_running: Arc<AtomicBool>,
    flush_tx: crossbeam_channel::Sender<()>,
    recorded_samples_rx: crossbeam_channel::Receiver<(Vec<Vec<f32>>, Option<std::path::PathBuf>)>,
    worker_handle: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    /// Query available system audio input devices.
    pub fn available_input_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        if let Ok(devs) = host.input_devices() {
            for d in devs {
                if let Ok(name) = d.name() {
                    devices.push(name);
                }
            }
        }
        devices
    }

    /// Initialize the audio recorder with default settings.
    pub fn new(monitor_producer: Option<Producer<f32>>) -> Result<Self, RecordError> {
        Self::with_config(None, None, monitor_producer)
    }

    /// Initialize the audio recorder with a specified device, sample rate, and optional monitoring producer.
    pub fn with_config(
        preferred_device: Option<&str>,
        preferred_sample_rate: Option<u32>,
        monitor_producer: Option<Producer<f32>>,
    ) -> Result<Self, RecordError> {
        Self::with_full_config(
            preferred_device,
            preferred_sample_rate,
            None,
            monitor_producer,
        )
    }

    /// Initialize the audio recorder with full device, sample rate, buffer size, and monitoring configuration.
    pub fn with_full_config(
        preferred_device: Option<&str>,
        preferred_sample_rate: Option<u32>,
        buffer_size_frames: Option<u32>,
        monitor_producer: Option<Producer<f32>>,
    ) -> Result<Self, RecordError> {
        let host = cpal::default_host();

        let device = if let Some(target_name) = preferred_device {
            let mut found = None;
            if let Ok(devs) = host.input_devices() {
                for d in devs {
                    if let Ok(name) = d.name() {
                        if name == target_name {
                            found = Some(d);
                            break;
                        }
                    }
                }
            }
            found.ok_or_else(|| RecordError::DeviceNotFound(target_name.to_string()))?
        } else {
            host.default_input_device()
                .ok_or(RecordError::NoInputDevice)?
        };

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Input Device".to_string());

        let default_config = device.default_input_config()?;
        let mut channels = default_config.channels().clamp(1, 2); // Prefer mono (1) or stereo (2)
        let mut sample_rate = default_config.sample_rate().0;
        let mut buffer_size = cpal::BufferSize::Default;

        if let Ok(ranges) = device.supported_input_configs() {
            let ranges: Vec<_> = ranges.collect();
            if !ranges.iter().any(|r| r.channels() == channels) {
                channels = default_config.channels();
            }
            if let Some(req_sr) = preferred_sample_rate {
                if let Some(best) = ranges
                    .iter()
                    .filter(|r| r.channels() == channels)
                    .min_by_key(|r| {
                        let clamped = req_sr.clamp(r.min_sample_rate().0, r.max_sample_rate().0);
                        clamped.abs_diff(req_sr)
                    })
                {
                    sample_rate = req_sr.clamp(best.min_sample_rate().0, best.max_sample_rate().0);
                }
            }
            if let Some(req_buf) = buffer_size_frames {
                if let Some(r) = ranges
                    .iter()
                    .find(|r| r.channels() == channels)
                    .or_else(|| ranges.first())
                {
                    match r.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => {
                            buffer_size = cpal::BufferSize::Fixed(req_buf.clamp(*min, *max));
                        }
                        cpal::SupportedBufferSize::Unknown => {
                            buffer_size = cpal::BufferSize::Fixed(req_buf);
                        }
                    }
                }
            }
        }

        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size,
        };

        // Lock-free ringbuffers
        let (record_producer, mut record_consumer) = RingBuffer::<f32>::new(65536);
        let (telem_producer, telem_consumer) = RingBuffer::<InputTelemetry>::new(256);

        let is_recording = Arc::new(AtomicBool::new(false));
        let monitoring_enabled = Arc::new(AtomicBool::new(false));

        let device_lost = Arc::new(AtomicBool::new(false));
        let device_lost_cb = device_lost.clone();
        let overrun_samples = Arc::new(AtomicU64::new(0));

        let mut record_callback = RealtimeRecordCallback::new(
            channels,
            record_producer,
            telem_producer,
            monitor_producer,
            is_recording.clone(),
            monitoring_enabled.clone(),
        )
        .with_overrun_counter(overrun_samples.clone());

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                record_callback.process(data);
            },
            move |_err| {
                // Device unplugged or audio service lost: mark flag without panicking or allocating
                device_lost_cb.store(true, Ordering::SeqCst);
            },
            None,
        )?;

        stream.play()?;

        let recording_dir = default_recording_dir(None);
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let initial_writer = RecordingDiskWriter::new(&recording_dir, spec)?;

        let worker_running = Arc::new(AtomicBool::new(true));
        let worker_running_thread = worker_running.clone();
        let write_error = Arc::new(AtomicBool::new(false));
        let write_error_thread = write_error.clone();

        let (result_tx, result_rx) = crossbeam_channel::bounded(4);
        let (flush_tx, flush_rx) = crossbeam_channel::bounded::<()>(4);

        let worker_handle = std::thread::spawn(move || {
            let mut writer = Some(initial_writer);
            let mut fallback_recorded: Vec<f32> = Vec::new();
            let num_ch = (channels as usize).max(1);

            while worker_running_thread.load(Ordering::Relaxed) {
                let mut drained_any = false;
                while let Ok(sample) = record_consumer.pop() {
                    drained_any = true;
                    if let Some(ref mut w) = writer {
                        if w.write_sample(sample).is_err() {
                            write_error_thread.store(true, Ordering::SeqCst);
                            writer = None;
                            if fallback_recorded.len() < MAX_FALLBACK_SAMPLES {
                                fallback_recorded.push(sample);
                            }
                        }
                    } else if fallback_recorded.len() < MAX_FALLBACK_SAMPLES {
                        fallback_recorded.push(sample);
                    }
                }

                if flush_rx.try_recv().is_ok() {
                    let mut planar = vec![Vec::new(); num_ch];
                    let mut finalized_path: Option<std::path::PathBuf> = None;
                    if let Some(w) = writer.take() {
                        if let Ok((wav_path, has_samples)) = w.finalize() {
                            if has_samples {
                                if let Ok(mut reader) = hound::WavReader::open(&wav_path) {
                                    let est_frames = (reader.len() as usize) / num_ch;
                                    for ch in &mut planar {
                                        ch.reserve(est_frames);
                                    }
                                    let mut ch_idx = 0usize;
                                    for s in reader.samples::<f32>().flatten() {
                                        planar[ch_idx].push(s);
                                        ch_idx = (ch_idx + 1) % num_ch;
                                    }
                                }
                                finalized_path = Some(wav_path);
                            } else {
                                let _ = std::fs::remove_file(&wav_path);
                            }
                        } else {
                            write_error_thread.store(true, Ordering::SeqCst);
                        }
                        if planar.first().is_none_or(|c| c.is_empty())
                            && !fallback_recorded.is_empty()
                        {
                            for chunk in fallback_recorded.chunks_exact(num_ch) {
                                for (c_idx, &s) in chunk.iter().enumerate() {
                                    planar[c_idx].push(s);
                                }
                            }
                        }
                        fallback_recorded.clear();
                        match RecordingDiskWriter::new(&recording_dir, spec) {
                            Ok(next_w) => writer = Some(next_w),
                            Err(_) => write_error_thread.store(true, Ordering::SeqCst),
                        }
                    } else if !fallback_recorded.is_empty() {
                        for chunk in fallback_recorded.chunks_exact(num_ch) {
                            for (c_idx, &s) in chunk.iter().enumerate() {
                                planar[c_idx].push(s);
                            }
                        }
                        fallback_recorded.clear();
                    }
                    let _ = result_tx.send((planar, finalized_path));
                }

                if !drained_any {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }

            if let Some(w) = writer.take() {
                if let Ok((wav_path, has_samples)) = w.finalize() {
                    if !has_samples {
                        let _ = std::fs::remove_file(&wav_path);
                    }
                }
            }
        });

        Ok(Self {
            _stream: stream,
            device_name,
            sample_rate,
            channels,
            is_recording,
            monitoring_enabled,
            device_lost,
            write_error,
            overrun_samples,
            telemetry_rx: telem_consumer,
            latest_telemetry: InputTelemetry::default(),
            worker_running,
            flush_tx,
            recorded_samples_rx: result_rx,
            worker_handle: Some(worker_handle),
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Relaxed)
    }

    pub fn has_write_error(&self) -> bool {
        self.write_error.load(Ordering::Relaxed)
    }

    pub fn overrun_samples(&self) -> u64 {
        self.overrun_samples.load(Ordering::Relaxed)
    }

    pub fn set_monitoring(&self, enabled: bool) {
        self.monitoring_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_monitoring(&self) -> bool {
        self.monitoring_enabled.load(Ordering::Relaxed)
    }

    /// Read latest input telemetry (peaks and RMS).
    pub fn telemetry(&mut self) -> InputTelemetry {
        while let Ok(telem) = self.telemetry_rx.pop() {
            self.latest_telemetry = telem;
        }
        self.latest_telemetry
    }

    /// Start capturing audio.
    pub fn start_recording(&self) {
        self.is_recording.store(true, Ordering::SeqCst);
    }

    /// Stop capturing audio and package recorded PCM into a new AudioSource.
    pub fn stop_recording(&self) -> Option<(AudioSource, Vec<Vec<f32>>)> {
        self.is_recording.store(false, Ordering::SeqCst);
        let _ = self.flush_tx.send(());

        // Wait for flushed planar buffers from worker thread
        let (planar, wav_path) = self
            .recorded_samples_rx
            .recv_timeout(std::time::Duration::from_millis(10_000))
            .ok()?;

        let num_frames = planar.first().map(|c| c.len()).unwrap_or(0);
        if num_frames == 0 {
            return None;
        }
        let num_channels = planar.len().max(1);

        let source_id = Uuid::new_v4();
        let default_filename = format!("Recorded_Take_{}.wav", &source_id.to_string()[..8]);
        let (path, filename) = if let Some(p) = wav_path {
            let fname = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or(default_filename);
            (p, fname)
        } else {
            (
                std::path::PathBuf::from(&default_filename),
                default_filename,
            )
        };

        let source = AudioSource {
            id: source_id,
            path,
            filename,
            sample_rate: self.sample_rate,
            channels: num_channels,
            duration_frames: num_frames as u64,
            file_hash: None,
        };

        Some((source, planar))
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.worker_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Encapsulates the real-time audio input callback state.
///
/// Invariants:
/// - Never allocates or locks on the audio recording callback thread.
pub struct RealtimeRecordCallback {
    pub channels: u16,
    pub record_producer: Producer<f32>,
    pub telem_producer: Producer<InputTelemetry>,
    pub monitor_producer: Option<Producer<f32>>,
    pub is_recording: Arc<AtomicBool>,
    pub monitoring_enabled: Arc<AtomicBool>,
    pub overrun_samples: Arc<AtomicU64>,
}

impl RealtimeRecordCallback {
    pub fn new(
        channels: u16,
        record_producer: Producer<f32>,
        telem_producer: Producer<InputTelemetry>,
        monitor_producer: Option<Producer<f32>>,
        is_recording: Arc<AtomicBool>,
        monitoring_enabled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            channels,
            record_producer,
            telem_producer,
            monitor_producer,
            is_recording,
            monitoring_enabled,
            overrun_samples: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_overrun_counter(mut self, overrun_samples: Arc<AtomicU64>) -> Self {
        self.overrun_samples = overrun_samples;
        self
    }

    pub fn process(&mut self, data: &[f32]) {
        // 1. Calculate Peak and RMS for live telemetry
        let mut p_l = 0.0_f32;
        let mut p_r = 0.0_f32;
        let mut sum_sq_l = 0.0_f32;
        let mut sum_sq_r = 0.0_f32;
        let mut frames = 0_usize;

        if self.channels == 1 {
            for &s in data {
                let abs = s.abs();
                if abs > p_l {
                    p_l = abs;
                }
                sum_sq_l += s * s;
                frames += 1;
            }
            p_r = p_l;
            sum_sq_r = sum_sq_l;
        } else {
            for chunk in data.as_chunks::<2>().0 {
                let l = chunk[0];
                let r = chunk[1];
                let abs_l = l.abs();
                let abs_r = r.abs();
                if abs_l > p_l {
                    p_l = abs_l;
                }
                if abs_r > p_r {
                    p_r = abs_r;
                }
                sum_sq_l += l * l;
                sum_sq_r += r * r;
                frames += 1;
            }
        }
        let count = frames.max(1) as f32;
        let r_l = (sum_sq_l / count).sqrt();
        let r_r = (sum_sq_r / count).sqrt();
        let _ = self.telem_producer.push(InputTelemetry {
            peak_l: p_l,
            peak_r: p_r,
            rms_l: r_l,
            rms_r: r_r,
        });

        // 2. Stream to recording worker if active (frame-aligned so overrun never shifts channels)
        if self.is_recording.load(Ordering::Relaxed) {
            let ch_count = (self.channels as usize).max(1);
            for frame in data.chunks_exact(ch_count) {
                if self.record_producer.slots() < ch_count {
                    self.overrun_samples
                        .fetch_add(ch_count as u64, Ordering::Relaxed);
                    break;
                }
                for &sample in frame {
                    let _ = self.record_producer.push(sample);
                }
            }
        }

        // 3. Software input monitoring (push atomic L/R pairs so overrun never swaps channels)
        if self.monitoring_enabled.load(Ordering::Relaxed) {
            if let Some(ref mut mon) = self.monitor_producer {
                if self.channels == 1 {
                    for &sample in data {
                        if mon.slots() < 2 {
                            break;
                        }
                        let _ = mon.push(sample);
                        let _ = mon.push(sample);
                    }
                } else {
                    let ch_count = (self.channels as usize).max(2);
                    for frame in data.chunks_exact(ch_count) {
                        if mon.slots() < 2 {
                            break;
                        }
                        let _ = mon.push(frame[0]);
                        let _ = mon.push(frame[1]);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_writer_unwritable_dir_returns_error() {
        let dir = std::env::temp_dir().join(format!("stem_rec_unwritable_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let blocking_file = dir.join("not_a_dir");
        std::fs::write(&blocking_file, b"file").unwrap();

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let err = RecordingDiskWriter::new(&blocking_file, spec)
            .err()
            .expect("unwritable recording directory must return Err");
        assert!(matches!(err, RecordError::Io(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recording_writer_periodic_flush_readable_after_crash_simulation() {
        let dir = std::env::temp_dir().join(format!("stem_rec_flush_{}", Uuid::new_v4()));
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = RecordingDiskWriter::new(&dir, spec).unwrap();
        let wav_path = writer.path().to_path_buf();

        // Write slightly over 1 second (8000 frames * 2 ch = 16000 samples) to trigger auto-flush
        for i in 0..16_200 {
            writer.write_sample((i % 100) as f32 * 0.005).unwrap();
        }
        // Drop without calling finalize() to simulate sudden crash
        drop(writer);

        let mut reader = hound::WavReader::open(&wav_path)
            .expect("flushed WAV must be readable by hound even without finalize");
        let count = reader.samples::<f32>().filter_map(Result::ok).count();
        assert!(
            count >= 16_000,
            "expected >= 16000 flushed samples, got {count}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recover_orphaned_recordings_finds_valid_wav() {
        let dir = std::env::temp_dir().join(format!("stem_rec_orphan_{}", Uuid::new_v4()));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = RecordingDiskWriter::new(&dir, spec).unwrap();
        for _ in 0..128 {
            writer.write_sample(0.1).unwrap();
        }
        let (path, has_samples) = writer.finalize().unwrap();
        assert!(has_samples);

        let found = recover_orphaned_recordings(&dir);
        assert_eq!(found, vec![path]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
