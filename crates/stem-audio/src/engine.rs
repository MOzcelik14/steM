//! Audio engine managing cpal hardware stream and lock-free UI communication

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use thiserror::Error;

use crate::callback::{AudioCommand, AudioProcessor, AudioTelemetry, SharedAudioTimeline};

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
    device_name: String,
    sample_rate: u32,
    _channels: u16,
    latest_telemetry: AudioTelemetry,
    is_playing: bool,
}

impl AudioEngine {
    /// Initialize audio engine with the default system output device.
    pub fn new() -> Result<Self, AudioEngineError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioEngineError::NoDevice)?;
        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio Device".to_string());

        let default_config = device.default_output_config()?;
        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        let config = StreamConfig {
            channels: 2, // Standard stereo output
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        // Ring buffers for lock-free communication
        let (cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(512);
        let (telem_tx, telem_rx) = RingBuffer::<AudioTelemetry>::new(512);

        // Channels to exchange shared timeline without deallocating on audio thread
        let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(8);
        let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(8);

        let mut processor = AudioProcessor::new(cmd_rx, telem_tx);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Check if a new timeline buffer was sent from the UI/worker thread
                if let Ok(new_timeline) = timeline_rx.try_recv() {
                    // Send previous timeline back to UI thread to avoid deallocating here
                    if let Some(old) = processor.set_audio_data(Some(new_timeline)) {
                        let _ = retired_tx.try_send(old);
                    }
                }

                processor.process_interleaved(data);
            },
            |err| {
                eprintln!("steM. audio stream error: {err}");
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
            device_name,
            sample_rate,
            _channels: channels,
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
        let _ = self.timeline_tx.send(timeline);
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
