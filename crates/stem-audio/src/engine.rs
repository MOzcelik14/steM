//! Audio engine managing cpal hardware stream and lock-free UI communication

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

use crate::callback::{
    AudioCommand, AudioProcessor, AudioTelemetry, RealtimeAudioCallback, SharedAudioTimeline,
};

#[derive(Error, Debug)]
pub enum AudioEngineError {
    #[error("No audio output device available")]
    NoDevice,

    #[error("Failed to query device configurations: {0}")]
    DeviceConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Failed to build audio stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    #[error("Failed to play audio stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
}

/// Handle to the running audio engine.
pub struct AudioEngine {
    _stream: Option<Stream>,
    command_tx: Producer<AudioCommand>,
    telemetry_rx: Consumer<AudioTelemetry>,
    timeline_tx: crossbeam_channel::Sender<Arc<SharedAudioTimeline>>,
    retired_timeline_rx: crossbeam_channel::Receiver<Arc<SharedAudioTimeline>>,
    monitor_tx: Option<Producer<f32>>,
    device_lost: Arc<AtomicBool>,
    is_input_monitoring: bool,
    device_name: String,
    sample_rate: u32,
    _channels: u16,
    latest_telemetry: AudioTelemetry,
    is_playing: bool,
}

/// Configuration options for opening an `AudioEngine` output stream.
#[derive(Debug, Clone, Default)]
pub struct AudioEngineConfig {
    pub preferred_device: Option<String>,
    pub preferred_sample_rate: Option<u32>,
    pub buffer_size_frames: Option<u32>,
}

impl AudioEngine {
    /// Lists all available output device names on the host.
    pub fn available_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut names = Vec::new();
        if let Ok(devs) = host.output_devices() {
            for d in devs {
                if let Ok(name) = d.name() {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Initialize audio engine with the default system output device.
    pub fn new() -> Result<Self, AudioEngineError> {
        Self::with_config(AudioEngineConfig::default())
    }

    /// Initialize audio engine with negotiated device, sample rate, channel layout, and buffer size.
    pub fn with_config(cfg: AudioEngineConfig) -> Result<Self, AudioEngineError> {
        let host = cpal::default_host();
        let device = if let Some(ref target_name) = cfg.preferred_device {
            let mut found = None;
            if let Ok(devs) = host.output_devices() {
                for d in devs {
                    if let Ok(name) = d.name() {
                        if name == *target_name {
                            found = Some(d);
                            break;
                        }
                    }
                }
            }
            found.or_else(|| host.default_output_device())
        } else {
            host.default_output_device()
        }
        .ok_or(AudioEngineError::NoDevice)?;

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio Device".to_string());

        let default_config = device.default_output_config()?;
        let default_sr = default_config.sample_rate().0;
        let default_ch = default_config.channels().max(1);

        let mut negotiated_channels = default_ch.clamp(1, 2);
        let mut negotiated_sr = default_sr;
        let mut negotiated_buf = cpal::BufferSize::Default;

        if let Ok(ranges) = device.supported_output_configs() {
            let ranges: Vec<_> = ranges.collect();
            // Prefer stereo (2ch) if any supported range offers it; otherwise keep default_ch
            if ranges.iter().any(|r| r.channels() == 2) {
                negotiated_channels = 2;
            } else if let Some(r) = ranges.first() {
                negotiated_channels = r.channels().max(1);
            }

            if let Some(req_sr) = cfg.preferred_sample_rate {
                if let Some(best) = ranges
                    .iter()
                    .filter(|r| r.channels() == negotiated_channels)
                    .min_by_key(|r| {
                        let clamped = req_sr.clamp(r.min_sample_rate().0, r.max_sample_rate().0);
                        clamped.abs_diff(req_sr)
                    })
                {
                    negotiated_sr =
                        req_sr.clamp(best.min_sample_rate().0, best.max_sample_rate().0);
                }
            }

            if let Some(req_buf) = cfg.buffer_size_frames {
                if let Some(r) = ranges
                    .iter()
                    .find(|r| r.channels() == negotiated_channels)
                    .or_else(|| ranges.first())
                {
                    match r.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => {
                            let clamped = req_buf.clamp(*min, *max);
                            negotiated_buf = cpal::BufferSize::Fixed(clamped);
                        }
                        cpal::SupportedBufferSize::Unknown => {
                            negotiated_buf = cpal::BufferSize::Fixed(req_buf);
                        }
                    }
                }
            }
        }

        let config = StreamConfig {
            channels: negotiated_channels,
            sample_rate: cpal::SampleRate(negotiated_sr),
            buffer_size: negotiated_buf,
        };

        // Ring buffers for lock-free communication
        let (cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(512);
        let (telem_tx, telem_rx) = RingBuffer::<AudioTelemetry>::new(512);
        let (mon_tx, mon_rx) = RingBuffer::<f32>::new(8192);

        // Channels to exchange shared timeline without deallocating on audio thread
        let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);
        let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);

        let processor = AudioProcessor::with_config(
            cmd_rx,
            telem_tx,
            Some(mon_rx),
            negotiated_sr,
            negotiated_channels,
        );
        let mut audio_callback = RealtimeAudioCallback::new(processor, timeline_rx, retired_tx);

        let device_lost = Arc::new(AtomicBool::new(false));
        let device_lost_cb = device_lost.clone();

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                audio_callback.process(data);
            },
            move |_err| {
                device_lost_cb.store(true, Ordering::SeqCst);
            },
            None,
        )?;

        stream.play()?;

        Ok(Self {
            _stream: Some(stream),
            command_tx: cmd_tx,
            telemetry_rx: telem_rx,
            timeline_tx,
            retired_timeline_rx: retired_rx,
            monitor_tx: Some(mon_tx),
            device_lost,
            is_input_monitoring: false,
            device_name,
            sample_rate: negotiated_sr,
            _channels: negotiated_channels,
            latest_telemetry: AudioTelemetry::default(),
            is_playing: false,
        })
    }

    /// Retrieve the current output device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Retrieve the output sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Retrieve the negotiated output channel count.
    pub fn channels(&self) -> u16 {
        self._channels
    }

    /// Query available audio output devices.
    pub fn available_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        if let Ok(devs) = host.output_devices() {
            for d in devs {
                if let Ok(name) = d.name() {
                    devices.push(name);
                }
            }
        }
        devices
    }

    /// Check if the audio output stream encountered a hardware disconnection or error.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Relaxed)
    }

    /// Extracts the software input monitoring producer to connect to the input recorder.
    pub fn take_monitor_producer(&mut self) -> Option<Producer<f32>> {
        self.monitor_tx.take()
    }

    /// Enables or disables software input monitoring.
    pub fn set_input_monitoring(&mut self, enabled: bool) {
        self.is_input_monitoring = enabled;
        let _ = self
            .command_tx
            .push(AudioCommand::SetInputMonitoring(enabled));
    }

    pub fn is_input_monitoring(&self) -> bool {
        self.is_input_monitoring
    }

    /// Configures real-time metronome click synthesis on the audio thread.
    pub fn set_metronome(&mut self, enabled: bool, bpm: f64, beats_per_bar: u8, sample_rate: u32) {
        let _ = self.command_tx.push(AudioCommand::SetMetronome {
            enabled,
            bpm,
            beats_per_bar,
            sample_rate,
        });
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.is_playing = true;
        let _ = self.command_tx.push(AudioCommand::Play);
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.is_playing = false;
        let _ = self.command_tx.push(AudioCommand::Pause);
    }

    /// Stop playback and reset playhead to beginning.
    pub fn stop(&mut self) {
        self.is_playing = false;
        let _ = self.command_tx.push(AudioCommand::Stop);
    }

    /// Toggle play / pause.
    pub fn toggle_play(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Seek playhead to a specific frame.
    pub fn seek(&mut self, frame: u64) {
        let _ = self.command_tx.push(AudioCommand::Seek(frame));
    }

    /// Set master gain in decibels.
    pub fn set_master_gain_db(&mut self, db: f32) {
        let linear = 10.0f32.powf(db / 20.0);
        let _ = self.command_tx.push(AudioCommand::SetMasterGain(linear));
    }

    /// Set track volume gain in decibels.
    pub fn set_track_gain_db(&mut self, track_idx: usize, db: f32) {
        let linear = 10.0f32.powf(db / 20.0);
        let _ = self.command_tx.push(AudioCommand::SetTrackGain {
            track_idx,
            gain: linear,
        });
    }

    /// Set track stereo panning (-1.0 to +1.0).
    pub fn set_track_pan(&mut self, track_idx: usize, pan: f32) {
        let _ = self
            .command_tx
            .push(AudioCommand::SetTrackPan { track_idx, pan });
    }

    /// Set track mute.
    pub fn set_track_mute(&mut self, track_idx: usize, muted: bool) {
        let _ = self
            .command_tx
            .push(AudioCommand::SetTrackMute { track_idx, muted });
    }

    /// Set track solo.
    pub fn set_track_solo(&mut self, track_idx: usize, soloed: bool) {
        let _ = self
            .command_tx
            .push(AudioCommand::SetTrackSolo { track_idx, soloed });
    }

    /// Configure loop region.
    pub fn set_loop(&mut self, enabled: bool, start_frame: u64, end_frame: u64) {
        let _ = self.command_tx.push(AudioCommand::SetLoop {
            enabled,
            start_frame,
            end_frame,
        });
    }

    /// Set effect bypass state on track (Some(idx)) or master (None).
    pub fn set_effect_bypass(
        &mut self,
        track_idx: Option<usize>,
        effect_idx: usize,
        bypassed: bool,
    ) {
        let _ = self.command_tx.push(AudioCommand::SetEffectBypass {
            track_idx,
            effect_idx,
            bypassed,
        });
    }

    /// Set effect parameter on track (Some(idx)) or master (None).
    pub fn set_effect_param(
        &mut self,
        track_idx: Option<usize>,
        effect_idx: usize,
        param_id: u8,
        value: f32,
    ) {
        let _ = self.command_tx.push(AudioCommand::SetEffectParam {
            track_idx,
            effect_idx,
            param_id,
            value,
        });
    }

    /// Reorder effect in insert chain.
    pub fn reorder_effect(&mut self, track_idx: Option<usize>, from: usize, to: usize) {
        let _ = self.command_tx.push(AudioCommand::ReorderEffect {
            track_idx,
            from,
            to,
        });
    }

    /// Reset effect state.
    pub fn reset_effect(&mut self, track_idx: Option<usize>, effect_idx: usize) {
        let _ = self.command_tx.push(AudioCommand::ResetEffect {
            track_idx,
            effect_idx,
        });
    }

    /// Update the shared audio timeline data played by the audio callback.
    pub fn update_timeline(&self, timeline: Arc<SharedAudioTimeline>) {
        while self.retired_timeline_rx.try_recv().is_ok() {}
        let _ = self.timeline_tx.try_send(timeline);
    }

    /// Drain latest telemetry from audio thread ringbuffer.
    pub fn poll_telemetry(&mut self) -> AudioTelemetry {
        while let Ok(telem) = self.telemetry_rx.pop() {
            self.latest_telemetry = telem;
        }
        // Drain any retired timeline buffers to free memory on the UI thread
        while self.retired_timeline_rx.try_recv().is_ok() {}
        self.latest_telemetry
    }
}
