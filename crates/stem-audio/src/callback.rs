//! Real-time audio callback processor
//!
//! STRICT INVARIANTS:
//! - NO memory allocations (no Vec::new, Box, String, etc.)
//! - NO mutex locks or synchronous waiting
//! - NO file I/O or system calls
//! - NO logging / print statements

use rtrb::{Consumer, Producer};
use std::sync::Arc;

/// Real-time audio commands sent from UI to the audio callback.
#[derive(Debug, Clone, Copy)]
pub enum AudioCommand {
    Play,
    Pause,
    Stop,
    Seek(u64),
    SetMasterGain(f32),
    SetTrackGain {
        track_idx: usize,
        gain: f32,
    },
    SetTrackPan {
        track_idx: usize,
        pan: f32,
    },
    SetTrackMute {
        track_idx: usize,
        muted: bool,
    },
    SetTrackSolo {
        track_idx: usize,
        soloed: bool,
    },
    SetLoop {
        enabled: bool,
        start_frame: u64,
        end_frame: u64,
    },
}

/// Telemetry metrics returned from the audio thread back to the UI.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioTelemetry {
    pub playhead_frame: u64,
    pub peak_left: f32,
    pub peak_right: f32,
}

pub const MAX_TRACKS: usize = 32;

/// Static per-track state kept in the audio callback.
#[derive(Clone, Copy)]
pub struct TrackMixerState {
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
}

impl Default for TrackMixerState {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
        }
    }
}

/// Pre-allocated playback buffer shared with the audio thread.
/// Swapped safely using Arc when timeline changes.
pub struct SharedAudioTimeline {
    /// Planar stereo mixdown buffer [L, R] prepared by background thread, or per-track buffers.
    pub channels: Vec<Vec<f32>>,
    pub total_frames: u64,
}

pub struct AudioProcessor {
    command_consumer: Consumer<AudioCommand>,
    telemetry_producer: Producer<AudioTelemetry>,
    is_playing: bool,
    playhead_frame: u64,
    master_gain: f32,
    tracks: [TrackMixerState; MAX_TRACKS],
    loop_enabled: bool,
    loop_start_frame: u64,
    loop_end_frame: u64,

    // Shared timeline audio buffer
    audio_data: Option<Arc<SharedAudioTimeline>>,
}

impl AudioProcessor {
    pub fn new(
        command_consumer: Consumer<AudioCommand>,
        telemetry_producer: Producer<AudioTelemetry>,
    ) -> Self {
        Self {
            command_consumer,
            telemetry_producer,
            is_playing: false,
            playhead_frame: 0,
            master_gain: 1.0,
            tracks: [TrackMixerState::default(); MAX_TRACKS],
            loop_enabled: false,
            loop_start_frame: 0,
            loop_end_frame: 0,
            audio_data: None,
        }
    }

    /// Update the timeline audio buffer reference. Returns the previous buffer so it can be deallocated off the audio thread.
    pub fn set_audio_data(
        &mut self,
        data: Option<Arc<SharedAudioTimeline>>,
    ) -> Option<Arc<SharedAudioTimeline>> {
        std::mem::replace(&mut self.audio_data, data)
    }

    /// Process commands from UI via lock-free ringbuffer.
    #[inline]
    fn process_commands(&mut self) {
        while let Ok(cmd) = self.command_consumer.pop() {
            match cmd {
                AudioCommand::Play => self.is_playing = true,
                AudioCommand::Pause => self.is_playing = false,
                AudioCommand::Stop => {
                    self.is_playing = false;
                    self.playhead_frame = 0;
                }
                AudioCommand::Seek(frame) => {
                    self.playhead_frame = frame;
                }
                AudioCommand::SetMasterGain(gain) => {
                    self.master_gain = gain;
                }
                AudioCommand::SetTrackGain { track_idx, gain } => {
                    if track_idx < MAX_TRACKS {
                        self.tracks[track_idx].gain = gain;
                    }
                }
                AudioCommand::SetTrackPan { track_idx, pan } => {
                    if track_idx < MAX_TRACKS {
                        self.tracks[track_idx].pan = pan;
                    }
                }
                AudioCommand::SetTrackMute { track_idx, muted } => {
                    if track_idx < MAX_TRACKS {
                        self.tracks[track_idx].muted = muted;
                    }
                }
                AudioCommand::SetTrackSolo { track_idx, soloed } => {
                    if track_idx < MAX_TRACKS {
                        self.tracks[track_idx].soloed = soloed;
                    }
                }
                AudioCommand::SetLoop {
                    enabled,
                    start_frame,
                    end_frame,
                } => {
                    self.loop_enabled = enabled;
                    self.loop_start_frame = start_frame;
                    self.loop_end_frame = end_frame;
                }
            }
        }
    }

    /// Real-time audio rendering callback.
    /// Interleaved output buffer [L0, R0, L1, R1, ...].
    pub fn process_interleaved(&mut self, output: &mut [f32]) {
        self.process_commands();

        let num_samples = output.len();
        let num_frames = num_samples / 2;

        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;

        if !self.is_playing {
            output.fill(0.0);
        } else if let Some(ref timeline) = self.audio_data {
            let left_ch = &timeline.channels[0];
            let right_ch = if timeline.channels.len() > 1 {
                &timeline.channels[1]
            } else {
                &timeline.channels[0]
            };

            let total_frames = timeline.total_frames;

            for f in 0..num_frames {
                // Check looping
                if self.loop_enabled
                    && self.loop_end_frame > self.loop_start_frame
                    && self.playhead_frame >= self.loop_end_frame
                {
                    self.playhead_frame = self.loop_start_frame;
                }

                let current_frame = self.playhead_frame as usize;
                let (l_sample, r_sample) = if current_frame < total_frames as usize {
                    let l = left_ch.get(current_frame).copied().unwrap_or(0.0) * self.master_gain;
                    let r = right_ch.get(current_frame).copied().unwrap_or(0.0) * self.master_gain;
                    (l, r)
                } else {
                    (0.0, 0.0)
                };

                let out_idx = f * 2;
                output[out_idx] = l_sample;
                output[out_idx + 1] = r_sample;

                let abs_l = l_sample.abs();
                let abs_r = r_sample.abs();
                if abs_l > peak_l {
                    peak_l = abs_l;
                }
                if abs_r > peak_r {
                    peak_r = abs_r;
                }

                self.playhead_frame = self.playhead_frame.saturating_add(1);

                // Auto-stop at end if not looping
                if !self.loop_enabled && self.playhead_frame >= total_frames {
                    self.is_playing = false;
                    // Zero out remaining frames
                    for rem in (f + 1)..num_frames {
                        output[rem * 2] = 0.0;
                        output[rem * 2 + 1] = 0.0;
                    }
                    break;
                }
            }
        } else {
            output.fill(0.0);
        }

        // Push telemetry to UI ring buffer (ignore error if ringbuffer is full)
        let _ = self.telemetry_producer.push(AudioTelemetry {
            playhead_frame: self.playhead_frame,
            peak_left: peak_l,
            peak_right: peak_r,
        });
    }
}
