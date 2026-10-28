//! Multi-track audio recording engine
//!
//! STRICT INVARIANTS:
//! - Real-time input callback performs NO allocations, NO locks, NO I/O.
//! - Lock-free ringbuffer streams incoming PCM to background recording worker.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use rtrb::RingBuffer;
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

    #[error("Failed to query input device configurations: {0}")]
    DeviceConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Failed to build input stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    #[error("Failed to start input stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
}

/// Manages hardware audio recording from the system input device.
pub struct AudioRecorder {
    _stream: Stream,
    device_name: String,
    sample_rate: u32,
    channels: u16,
    is_recording: Arc<AtomicBool>,
    worker_running: Arc<AtomicBool>,
    flush_tx: crossbeam_channel::Sender<()>,
    recorded_samples_rx: crossbeam_channel::Receiver<Vec<f32>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl AudioRecorder {
    /// Initialize the audio recorder with the default system input device.
    pub fn new() -> Result<Self, RecordError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(RecordError::NoInputDevice)?;
        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Input Device".to_string());

        let default_config = device.default_input_config()?;
        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels().min(2); // Support mono or stereo input

        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        // Lock-free ringbuffer: 65536 samples capacity
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(65536);

        let is_recording = Arc::new(AtomicBool::new(false));
        let is_recording_cb = is_recording.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if is_recording_cb.load(Ordering::Relaxed) {
                    for &sample in data {
                        let _ = producer.push(sample);
                    }
                }
            },
            |err| {
                eprintln!("steM. recording input error: {err}");
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
                // Drain ringbuffer into memory vector
                let mut drained_any = false;
                while let Ok(sample) = consumer.pop() {
                    recorded.push(sample);
                    drained_any = true;
                }

                // Check for flush / finish signal
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

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    /// Start capturing audio.
    pub fn start_recording(&self) {
        self.is_recording.store(true, Ordering::SeqCst);
    }

    /// Stop capturing audio and package recorded PCM into a new AudioSource.
    pub fn stop_recording(&self) -> Option<(AudioSource, Vec<Vec<f32>>)> {
        self.is_recording.store(false, Ordering::SeqCst);
        let _ = self.flush_tx.send(());

        // Wait for flushed buffer from worker thread (up to 200ms)
        let interleaved = self
            .recorded_samples_rx
            .recv_timeout(std::time::Duration::from_millis(200))
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
