//! Multi-track audio recording engine
//!
//! STRICT INVARIANTS:
//! - Real-time input callback performs NO allocations, NO locks, NO I/O.
//! - Lock-free ringbuffer streams incoming PCM to background recording worker.
//! - Live Peak & RMS input telemetry and software input monitoring.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use thiserror::Error;
use uuid::Uuid;

use stem_core::project::AudioSource;

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
    telemetry_rx: Consumer<InputTelemetry>,
    latest_telemetry: InputTelemetry,
    worker_running: Arc<AtomicBool>,
    flush_tx: crossbeam_channel::Sender<()>,
    recorded_samples_rx: crossbeam_channel::Receiver<Vec<f32>>,
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
        mut monitor_producer: Option<Producer<f32>>,
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
        let sample_rate = preferred_sample_rate.unwrap_or_else(|| default_config.sample_rate().0);
        let channels = default_config.channels().min(2); // Support mono (1) or stereo (2)

        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        // Lock-free ringbuffers
        let (mut record_producer, mut record_consumer) = RingBuffer::<f32>::new(65536);
        let (mut telem_producer, telem_consumer) = RingBuffer::<InputTelemetry>::new(256);

        let is_recording = Arc::new(AtomicBool::new(false));
        let is_recording_cb = is_recording.clone();

        let monitoring_enabled = Arc::new(AtomicBool::new(false));
        let monitoring_cb = monitoring_enabled.clone();

        let device_lost = Arc::new(AtomicBool::new(false));
        let device_lost_cb = device_lost.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 1. Calculate Peak and RMS for live telemetry
                let mut p_l = 0.0_f32;
                let mut p_r = 0.0_f32;
                let mut sum_sq_l = 0.0_f32;
                let mut sum_sq_r = 0.0_f32;
                let mut frames = 0_usize;

                if channels == 1 {
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
                let _ = telem_producer.push(InputTelemetry {
                    peak_l: p_l,
                    peak_r: p_r,
                    rms_l: r_l,
                    rms_r: r_r,
                });

                // 2. Stream to recording worker if active
                if is_recording_cb.load(Ordering::Relaxed) {
                    for &sample in data {
                        let _ = record_producer.push(sample);
                    }
                }

                // 3. Software input monitoring
                if monitoring_cb.load(Ordering::Relaxed) {
                    if let Some(ref mut mon) = monitor_producer {
                        if channels == 1 {
                            for &sample in data {
                                let _ = mon.push(sample);
                                let _ = mon.push(sample);
                            }
                        } else {
                            for &sample in data {
                                let _ = mon.push(sample);
                            }
                        }
                    }
                }
            },
            move |_err| {
                // Device unplugged or audio service lost: mark flag without panicking or allocating
                device_lost_cb.store(true, Ordering::SeqCst);
            },
            None,
        )?;

        stream.play()?;

        let worker_running = Arc::new(AtomicBool::new(true));
        let worker_running_thread = worker_running.clone();

        let (result_tx, result_rx) = crossbeam_channel::bounded(4);
        let (flush_tx, flush_rx) = crossbeam_channel::bounded::<()>(4);

        let worker_handle = std::thread::spawn(move || {
            let mut recorded = Vec::with_capacity(48000 * 10);

            while worker_running_thread.load(Ordering::Relaxed) {
                let mut drained_any = false;
                while let Ok(sample) = record_consumer.pop() {
                    recorded.push(sample);
                    drained_any = true;
                }

                if flush_rx.try_recv().is_ok() {
                    let _ = result_tx.send(std::mem::take(&mut recorded));
                }

                if !drained_any {
                    std::thread::sleep(std::time::Duration::from_millis(10));
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

        // Wait for flushed buffer from worker thread (up to 250ms)
        let interleaved = self
            .recorded_samples_rx
            .recv_timeout(std::time::Duration::from_millis(250))
            .ok()?;

        if interleaved.is_empty() {
            return None;
        }

        let num_channels = self.channels as usize;
        let num_frames = interleaved.len() / num_channels;

        let mut planar = vec![Vec::with_capacity(num_frames); num_channels];
        for chunk in interleaved.chunks(num_channels) {
            for (ch_idx, &sample) in chunk.iter().enumerate() {
                planar[ch_idx].push(sample);
            }
        }

        let source_id = Uuid::new_v4();
        let filename = format!("Recorded_Take_{}.wav", &source_id.to_string()[..8]);

        let source = AudioSource {
            id: source_id,
            path: std::path::PathBuf::from(&filename),
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
