//! Real-time audio callback processor
//!
//! STRICT INVARIANTS:
//! - NO memory allocations (no Vec::new, Box, String, etc.)
//! - NO mutex locks or synchronous waiting
//! - NO file I/O or system calls
//! - NO logging / print statements

use rtrb::{Consumer, Producer};
use std::sync::Arc;
use stem_dsp::chain::EffectChain;

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
    SetEffectBypass {
        track_idx: Option<usize>,
        effect_idx: usize,
        bypassed: bool,
    },
    SetEffectParam {
        track_idx: Option<usize>,
        effect_idx: usize,
        param_id: u8,
        value: f32,
    },
    ReorderEffect {
        track_idx: Option<usize>,
        from: usize,
        to: usize,
    },
    ResetEffect {
        track_idx: Option<usize>,
        effect_idx: usize,
    },
    SetInputMonitoring(bool),
}

/// Telemetry metrics returned from the audio thread back to the UI.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioTelemetry {
    pub playhead_frame: u64,
    pub peak_left: f32,
    pub peak_right: f32,
    pub rms_left: f32,
    pub rms_right: f32,
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
    /// Planar stereo mixdown buffer [L, R] prepared by background thread, or waveform preview.
    pub channels: Vec<Vec<f32>>,
    /// Optional per-track planar audio buffers [track_0: [L, R], track_1: [L, R], ...].
    pub track_channels: Vec<Vec<Vec<f32>>>,
    pub total_frames: u64,
}

pub struct AudioProcessor {
    command_consumer: Consumer<AudioCommand>,
    telemetry_producer: Producer<AudioTelemetry>,
    is_playing: bool,
    playhead_frame: u64,
    master_gain: f32,
    tracks: [TrackMixerState; MAX_TRACKS],
    track_effects: [EffectChain; MAX_TRACKS],
    master_effects: EffectChain,
    loop_enabled: bool,
    loop_start_frame: u64,
    loop_end_frame: u64,

    // Shared timeline audio buffer
    audio_data: Option<Arc<SharedAudioTimeline>>,

    // Software input monitoring
    monitor_consumer: Option<Consumer<f32>>,
    monitoring_enabled: bool,
}

impl AudioProcessor {
    pub fn new(
        command_consumer: Consumer<AudioCommand>,
        telemetry_producer: Producer<AudioTelemetry>,
        monitor_consumer: Option<Consumer<f32>>,
    ) -> Self {
        let default_track_effects: [EffectChain; MAX_TRACKS] =
            std::array::from_fn(|_| EffectChain::default_chain(48000.0));
        Self {
            command_consumer,
            telemetry_producer,
            is_playing: false,
            playhead_frame: 0,
            master_gain: 1.0,
            tracks: [TrackMixerState::default(); MAX_TRACKS],
            track_effects: default_track_effects,
            master_effects: EffectChain::default_chain(48000.0),
            loop_enabled: false,
            loop_start_frame: 0,
            loop_end_frame: 0,
            audio_data: None,
            monitor_consumer,
            monitoring_enabled: false,
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
                AudioCommand::SetEffectBypass {
                    track_idx,
                    effect_idx,
                    bypassed,
                } => match track_idx {
                    Some(t) if t < MAX_TRACKS => {
                        self.track_effects[t].set_bypassed(effect_idx, bypassed);
                    }
                    None => {
                        self.master_effects.set_bypassed(effect_idx, bypassed);
                    }
                    _ => {}
                },
                AudioCommand::SetEffectParam {
                    track_idx,
                    effect_idx,
                    param_id,
                    value,
                } => match track_idx {
                    Some(t) if t < MAX_TRACKS => {
                        self.track_effects[t].set_param(effect_idx, param_id, value);
                    }
                    None => {
                        self.master_effects.set_param(effect_idx, param_id, value);
                    }
                    _ => {}
                },
                AudioCommand::ReorderEffect {
                    track_idx,
                    from,
                    to,
                } => match track_idx {
                    Some(t) if t < MAX_TRACKS => {
                        self.track_effects[t].reorder(from, to);
                    }
                    None => {
                        self.master_effects.reorder(from, to);
                    }
                    _ => {}
                },
                AudioCommand::ResetEffect {
                    track_idx,
                    effect_idx,
                } => match track_idx {
                    Some(t) if t < MAX_TRACKS => {
                        self.track_effects[t].reset(effect_idx);
                    }
                    None => {
                        self.master_effects.reset(effect_idx);
                    }
                    _ => {}
                },
                AudioCommand::SetInputMonitoring(enabled) => {
                    self.monitoring_enabled = enabled;
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
        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let mut frames_counted = 0usize;

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
            let has_solos = self.tracks.iter().any(|t| t.soloed);
            let has_multitrack = !timeline.track_channels.is_empty();

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
                    if has_multitrack {
                        let mut accum_l = 0.0f32;
                        let mut accum_r = 0.0f32;
                        let num_tracks = timeline.track_channels.len().min(MAX_TRACKS);

                        for t in 0..num_tracks {
                            let track_state = self.tracks[t];
                            if track_state.muted || (has_solos && !track_state.soloed) {
                                continue;
                            }
                            let t_chans = &timeline.track_channels[t];
                            if t_chans.is_empty() {
                                continue;
                            }
                            let t_left = t_chans[0].get(current_frame).copied().unwrap_or(0.0);
                            let t_right = if t_chans.len() > 1 {
                                t_chans[1].get(current_frame).copied().unwrap_or(0.0)
                            } else {
                                t_left
                            };

                            // Real-time per-track effect chain
                            let (proc_l, proc_r) =
                                self.track_effects[t].process_stereo(t_left, t_right);

                            // Constant-power panning law
                            let pan = track_state.pan.clamp(-1.0, 1.0);
                            let pan_rad = (pan + 1.0) * (std::f32::consts::PI / 4.0);
                            let gain_l = track_state.gain * pan_rad.cos();
                            let gain_r = track_state.gain * pan_rad.sin();

                            accum_l += proc_l * gain_l;
                            accum_r += proc_r * gain_r;
                        }

                        // Real-time master insert chain
                        let (ml, mr) = self.master_effects.process_stereo(accum_l, accum_r);
                        (ml * self.master_gain, mr * self.master_gain)
                    } else {
                        // Single stereo stream (e.g. waveform editor preview or stereo file)
                        let raw_l = left_ch.get(current_frame).copied().unwrap_or(0.0);
                        let raw_r = right_ch.get(current_frame).copied().unwrap_or(0.0);

                        // Real-time master insert chain preview
                        let (ml, mr) = self.master_effects.process_stereo(raw_l, raw_r);
                        (ml * self.master_gain, mr * self.master_gain)
                    }
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

                sum_sq_l += l_sample * l_sample;
                sum_sq_r += r_sample * r_sample;
                frames_counted += 1;

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

        // Software input monitoring (live passthrough of inputs to playback outputs)
        if self.monitoring_enabled {
            if let Some(ref mut mon) = self.monitor_consumer {
                for f in 0..num_frames {
                    if let Ok(l_mon) = mon.pop() {
                        let r_mon = mon.pop().unwrap_or(l_mon);
                        output[f * 2] += l_mon;
                        output[f * 2 + 1] += r_mon;

                        if !self.is_playing {
                            let abs_l = output[f * 2].abs();
                            let abs_r = output[f * 2 + 1].abs();
                            if abs_l > peak_l {
                                peak_l = abs_l;
                            }
                            if abs_r > peak_r {
                                peak_r = abs_r;
                            }
                            sum_sq_l += output[f * 2] * output[f * 2];
                            sum_sq_r += output[f * 2 + 1] * output[f * 2 + 1];
                            frames_counted += 1;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        let count = frames_counted.max(1) as f32;
        let rms_l = (sum_sq_l / count).sqrt();
        let rms_r = (sum_sq_r / count).sqrt();

        // Push telemetry to UI ring buffer (ignore error if ringbuffer is full)
        let _ = self.telemetry_producer.push(AudioTelemetry {
            playhead_frame: self.playhead_frame,
            peak_left: peak_l,
            peak_right: peak_r,
            rms_left: rms_l,
            rms_right: rms_r,
        });
    }
}
