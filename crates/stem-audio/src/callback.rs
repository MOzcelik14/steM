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
    SetMetronome {
        enabled: bool,
        bpm: f64,
        beats_per_bar: u8,
        sample_rate: u32,
    },
    SetTrackOutputBus {
        track_idx: usize,
        bus_idx: Option<usize>,
    },
    SetTrackSend {
        track_idx: usize,
        send_idx: usize,
        bus_idx: Option<usize>,
        gain: f32,
        pre_fader: bool,
    },
    SetBusGain {
        bus_idx: usize,
        gain: f32,
    },
    SetBusPan {
        bus_idx: usize,
        pan: f32,
    },
    SetBusMute {
        bus_idx: usize,
        muted: bool,
    },
    SetBusSolo {
        bus_idx: usize,
        soloed: bool,
    },
    SetBusOutputBus {
        bus_idx: usize,
        parent_idx: Option<usize>,
    },
}

pub const MAX_TRACKS: usize = stem_core::MAX_PROJECT_TRACKS;
pub const MAX_BUSSES: usize = 16;
pub const MAX_SENDS: usize = 4;

/// Telemetry metrics returned from the audio thread back to the UI.
#[derive(Debug, Clone, Copy)]
pub struct AudioTelemetry {
    pub playhead_frame: u64,
    pub peak_left: f32,
    pub peak_right: f32,
    pub rms_left: f32,
    pub rms_right: f32,
    pub track_peaks: [f32; MAX_TRACKS],
}

impl Default for AudioTelemetry {
    fn default() -> Self {
        Self {
            playhead_frame: 0,
            peak_left: 0.0,
            peak_right: 0.0,
            rms_left: 0.0,
            rms_right: 0.0,
            track_peaks: [0.0; MAX_TRACKS],
        }
    }
}

/// Auxiliary send state for a track.
#[derive(Clone, Copy, Default)]
pub struct SendState {
    pub bus_idx: Option<usize>,
    pub gain: f32,
    pub pre_fader: bool,
}

/// Static per-bus state kept in the audio callback.
#[derive(Clone, Copy)]
pub struct BusMixerState {
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub output_bus_idx: Option<usize>,
}

impl Default for BusMixerState {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            output_bus_idx: None,
        }
    }
}

/// Static per-track state kept in the audio callback.
#[derive(Clone, Copy)]
pub struct TrackMixerState {
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub output_bus_idx: Option<usize>,
    pub sends: [SendState; MAX_SENDS],
}

impl Default for TrackMixerState {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            output_bus_idx: None,
            sends: [SendState::default(); MAX_SENDS],
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
    busses: [BusMixerState; MAX_BUSSES],
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

    // Real-time metronome click oscillator (zero-alloc)
    metronome_enabled: bool,
    metronome_bpm: f64,
    metronome_beats_per_bar: u8,
    metronome_sample_rate: u32,
    metronome_phase: f32,
    metronome_env: f32,
    metronome_freq_hz: f32,
    metronome_last_beat_idx: i64,
    output_channels: u16,
}

impl AudioProcessor {
    pub fn new(
        command_consumer: Consumer<AudioCommand>,
        telemetry_producer: Producer<AudioTelemetry>,
        monitor_consumer: Option<Consumer<f32>>,
    ) -> Self {
        Self::with_sample_rate(
            command_consumer,
            telemetry_producer,
            monitor_consumer,
            48000,
        )
    }

    pub fn with_sample_rate(
        command_consumer: Consumer<AudioCommand>,
        telemetry_producer: Producer<AudioTelemetry>,
        monitor_consumer: Option<Consumer<f32>>,
        sample_rate: u32,
    ) -> Self {
        Self::with_config(
            command_consumer,
            telemetry_producer,
            monitor_consumer,
            sample_rate,
            2,
        )
    }

    pub fn with_config(
        command_consumer: Consumer<AudioCommand>,
        telemetry_producer: Producer<AudioTelemetry>,
        monitor_consumer: Option<Consumer<f32>>,
        sample_rate: u32,
        output_channels: u16,
    ) -> Self {
        let sr = sample_rate.max(1);
        let default_track_effects: [EffectChain; MAX_TRACKS] =
            std::array::from_fn(|_| EffectChain::default_chain(sr as f32));
        Self {
            command_consumer,
            telemetry_producer,
            is_playing: false,
            playhead_frame: 0,
            master_gain: 1.0,
            tracks: [TrackMixerState::default(); MAX_TRACKS],
            busses: [BusMixerState::default(); MAX_BUSSES],
            track_effects: default_track_effects,
            master_effects: EffectChain::default_chain(sr as f32),
            loop_enabled: false,
            loop_start_frame: 0,
            loop_end_frame: 0,
            audio_data: None,
            monitor_consumer,
            monitoring_enabled: false,
            metronome_enabled: false,
            metronome_bpm: 120.0,
            metronome_beats_per_bar: 4,
            metronome_sample_rate: sr,
            metronome_phase: 0.0,
            metronome_env: 0.0,
            metronome_freq_hz: 1000.0,
            metronome_last_beat_idx: -1,
            output_channels: output_channels.max(1),
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
                AudioCommand::Pause => {
                    self.is_playing = false;
                    self.metronome_env = 0.0;
                    self.metronome_last_beat_idx = -1;
                }
                AudioCommand::Stop => {
                    self.is_playing = false;
                    self.playhead_frame = 0;
                    self.metronome_env = 0.0;
                    self.metronome_last_beat_idx = -1;
                }
                AudioCommand::Seek(frame) => {
                    self.playhead_frame = frame;
                    self.metronome_env = 0.0;
                    self.metronome_last_beat_idx = -1;
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
                AudioCommand::SetMetronome {
                    enabled,
                    bpm,
                    beats_per_bar,
                    sample_rate,
                } => {
                    self.metronome_enabled = enabled;
                    self.metronome_bpm = bpm.max(20.0);
                    self.metronome_beats_per_bar = beats_per_bar.max(1);
                    self.metronome_sample_rate = sample_rate.max(8000);
                    if !enabled {
                        self.metronome_env = 0.0;
                        self.metronome_last_beat_idx = -1;
                    }
                }
                AudioCommand::SetTrackOutputBus { track_idx, bus_idx } => {
                    if track_idx < MAX_TRACKS {
                        self.tracks[track_idx].output_bus_idx = bus_idx.filter(|&b| b < MAX_BUSSES);
                    }
                }
                AudioCommand::SetTrackSend {
                    track_idx,
                    send_idx,
                    bus_idx,
                    gain,
                    pre_fader,
                } => {
                    if track_idx < MAX_TRACKS && send_idx < MAX_SENDS {
                        self.tracks[track_idx].sends[send_idx] = SendState {
                            bus_idx: bus_idx.filter(|&b| b < MAX_BUSSES),
                            gain,
                            pre_fader,
                        };
                    }
                }
                AudioCommand::SetBusGain { bus_idx, gain } => {
                    if bus_idx < MAX_BUSSES {
                        self.busses[bus_idx].gain = gain;
                    }
                }
                AudioCommand::SetBusPan { bus_idx, pan } => {
                    if bus_idx < MAX_BUSSES {
                        self.busses[bus_idx].pan = pan;
                    }
                }
                AudioCommand::SetBusMute { bus_idx, muted } => {
                    if bus_idx < MAX_BUSSES {
                        self.busses[bus_idx].muted = muted;
                    }
                }
                AudioCommand::SetBusSolo { bus_idx, soloed } => {
                    if bus_idx < MAX_BUSSES {
                        self.busses[bus_idx].soloed = soloed;
                    }
                }
                AudioCommand::SetBusOutputBus {
                    bus_idx,
                    parent_idx,
                } => {
                    if bus_idx < MAX_BUSSES {
                        self.busses[bus_idx].output_bus_idx =
                            parent_idx.filter(|&p| p < MAX_BUSSES && p != bus_idx);
                    }
                }
            }
        }
    }

    /// Real-time audio rendering callback.
    /// Interleaved output buffer [L0, R0, L1, R1, ...].
    pub fn process_interleaved(&mut self, output: &mut [f32]) {
        self.process_commands();

        let out_ch = (self.output_channels as usize).max(1);
        let num_samples = output.len();
        let num_frames = num_samples / out_ch;

        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;
        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let mut frames_counted = 0usize;
        let mut track_peaks = [0.0f32; MAX_TRACKS];

        if !self.is_playing {
            output.fill(0.0);
        } else {
            let has_solos = self.tracks.iter().any(|t| t.soloed);

            for f in 0..num_frames {
                let (mut l_sample, mut r_sample, maybe_total_frames) = if let Some(ref timeline) =
                    self.audio_data
                {
                    let total_frames = timeline.total_frames;
                    if self.loop_enabled
                        && self.loop_end_frame > self.loop_start_frame
                        && self.playhead_frame >= self.loop_end_frame
                    {
                        self.playhead_frame = self.loop_start_frame;
                        self.metronome_last_beat_idx = -1;
                    }

                    let current_frame = self.playhead_frame as usize;
                    if current_frame < total_frames as usize {
                        if !timeline.track_channels.is_empty() {
                            let mut accum_l = 0.0f32;
                            let mut accum_r = 0.0f32;
                            let mut bus_accum_l = [0.0f32; MAX_BUSSES];
                            let mut bus_accum_r = [0.0f32; MAX_BUSSES];
                            let has_bus_solos = self.busses.iter().any(|b| b.soloed);
                            let num_tracks = timeline.track_channels.len().min(MAX_TRACKS);

                            #[allow(clippy::needless_range_loop)]
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

                                let (proc_l, proc_r) =
                                    self.track_effects[t].process_stereo(t_left, t_right);

                                // Pre-fader auxiliary sends
                                for send in &track_state.sends {
                                    if send.pre_fader {
                                        if let Some(b_idx) = send.bus_idx {
                                            bus_accum_l[b_idx] += proc_l * send.gain;
                                            bus_accum_r[b_idx] += proc_r * send.gain;
                                        }
                                    }
                                }

                                let pan = track_state.pan.clamp(-1.0, 1.0);
                                let pan_rad = (pan + 1.0) * (std::f32::consts::PI / 4.0);
                                let gain_l = track_state.gain * pan_rad.cos();
                                let gain_r = track_state.gain * pan_rad.sin();

                                let out_l = proc_l * gain_l;
                                let out_r = proc_r * gain_r;
                                let trk_peak = out_l.abs().max(out_r.abs());
                                if trk_peak > track_peaks[t] {
                                    track_peaks[t] = trk_peak;
                                }

                                // Post-fader auxiliary sends
                                for send in &track_state.sends {
                                    if !send.pre_fader {
                                        if let Some(b_idx) = send.bus_idx {
                                            bus_accum_l[b_idx] += out_l * send.gain;
                                            bus_accum_r[b_idx] += out_r * send.gain;
                                        }
                                    }
                                }

                                if let Some(b_idx) = track_state.output_bus_idx {
                                    bus_accum_l[b_idx] += out_l;
                                    bus_accum_r[b_idx] += out_r;
                                } else {
                                    accum_l += out_l;
                                    accum_r += out_r;
                                }
                            }

                            // Process submix busses (2 passes to support bus-to-bus chaining)
                            for pass in 0..2 {
                                for b in 0..MAX_BUSSES {
                                    let b_in_l = bus_accum_l[b];
                                    let b_in_r = bus_accum_r[b];
                                    if b_in_l == 0.0 && b_in_r == 0.0 {
                                        continue;
                                    }
                                    let bus = self.busses[b];
                                    if pass == 0 && bus.output_bus_idx.is_none() {
                                        continue;
                                    }
                                    if pass == 1 && bus.output_bus_idx.is_some() {
                                        continue;
                                    }
                                    bus_accum_l[b] = 0.0;
                                    bus_accum_r[b] = 0.0;

                                    if bus.muted || (has_bus_solos && !bus.soloed) {
                                        continue;
                                    }
                                    let pan = bus.pan.clamp(-1.0, 1.0);
                                    let pan_rad = (pan + 1.0) * (std::f32::consts::PI / 4.0);
                                    let out_l = b_in_l * bus.gain * pan_rad.cos();
                                    let out_r = b_in_r * bus.gain * pan_rad.sin();
                                    if let Some(parent_idx) = bus.output_bus_idx {
                                        bus_accum_l[parent_idx] += out_l;
                                        bus_accum_r[parent_idx] += out_r;
                                    } else {
                                        accum_l += out_l;
                                        accum_r += out_r;
                                    }
                                }
                            }

                            let (ml, mr) = self.master_effects.process_stereo(accum_l, accum_r);
                            (
                                ml * self.master_gain,
                                mr * self.master_gain,
                                Some(total_frames),
                            )
                        } else if let Some(left_ch) = timeline.channels.first() {
                            let right_ch = timeline.channels.get(1).unwrap_or(left_ch);
                            let raw_l = left_ch.get(current_frame).copied().unwrap_or(0.0);
                            let raw_r = right_ch.get(current_frame).copied().unwrap_or(0.0);
                            let (ml, mr) = self.master_effects.process_stereo(raw_l, raw_r);
                            (
                                ml * self.master_gain,
                                mr * self.master_gain,
                                Some(total_frames),
                            )
                        } else {
                            (0.0, 0.0, Some(total_frames))
                        }
                    } else {
                        (0.0, 0.0, Some(total_frames))
                    }
                } else {
                    (0.0, 0.0, None)
                };

                // Zero-alloc real-time metronome click synthesis
                if self.metronome_enabled {
                    let sr = self.metronome_sample_rate.max(1) as f64;
                    let frames_per_beat = (60.0 / self.metronome_bpm.max(20.0)) * sr;
                    let beat_idx = (self.playhead_frame as f64 / frames_per_beat).floor() as i64;
                    if beat_idx != self.metronome_last_beat_idx {
                        self.metronome_last_beat_idx = beat_idx;
                        let bpb = self.metronome_beats_per_bar.max(1) as i64;
                        self.metronome_freq_hz = if beat_idx % bpb == 0 { 1500.0 } else { 1000.0 };
                        self.metronome_env = 0.35;
                        self.metronome_phase = 0.0;
                    }
                    if self.metronome_env > 1e-4 {
                        let click = (self.metronome_phase * std::f32::consts::TAU).sin()
                            * self.metronome_env;
                        self.metronome_phase =
                            (self.metronome_phase + self.metronome_freq_hz / sr as f32).fract();
                        self.metronome_env *= 0.996;
                        l_sample += click;
                        r_sample += click;
                    }
                }

                let out_idx = f * out_ch;
                if out_ch == 1 {
                    output[out_idx] = 0.5 * (l_sample + r_sample);
                } else {
                    output[out_idx] = l_sample;
                    output[out_idx + 1] = r_sample;
                    for extra in 2..out_ch {
                        output[out_idx + extra] = 0.0;
                    }
                }

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

                // Auto-stop at end if not looping and metronome is off
                if let Some(total_frames) = maybe_total_frames {
                    if !self.loop_enabled
                        && !self.metronome_enabled
                        && self.playhead_frame >= total_frames
                    {
                        self.is_playing = false;
                        for rem in (f + 1)..num_frames {
                            let rem_idx = rem * out_ch;
                            for ch in 0..out_ch {
                                output[rem_idx + ch] = 0.0;
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Software input monitoring (live passthrough of inputs to playback outputs)
        if self.monitoring_enabled {
            if let Some(ref mut mon) = self.monitor_consumer {
                for f in 0..num_frames {
                    if mon.slots() < 2 {
                        break;
                    }
                    if let (Ok(l_mon), Ok(r_mon)) = (mon.pop(), mon.pop()) {
                        let out_idx = f * out_ch;
                        if out_ch == 1 {
                            output[out_idx] += 0.5 * (l_mon + r_mon);
                        } else {
                            output[out_idx] += l_mon;
                            output[out_idx + 1] += r_mon;
                        }

                        if !self.is_playing {
                            let abs_l = l_mon.abs();
                            let abs_r = r_mon.abs();
                            if abs_l > peak_l {
                                peak_l = abs_l;
                            }
                            if abs_r > peak_r {
                                peak_r = abs_r;
                            }
                            sum_sq_l += l_mon * l_mon;
                            sum_sq_r += r_mon * r_mon;
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
            track_peaks,
        });
    }
}

/// Encapsulates the real-time audio thread callback state and timeline buffer management.
///
/// Invariants:
/// - Never allocates or drops heap memory on the audio thread.
/// - Saturated retired channel halts timeline ingestion without leaking or freeing.
pub struct RealtimeAudioCallback {
    pub processor: AudioProcessor,
    pub timeline_rx: crossbeam_channel::Receiver<Arc<SharedAudioTimeline>>,
    pub retired_tx: crossbeam_channel::Sender<Arc<SharedAudioTimeline>>,
    pub stashed_retired: Option<Arc<SharedAudioTimeline>>,
}

impl RealtimeAudioCallback {
    pub fn new(
        processor: AudioProcessor,
        timeline_rx: crossbeam_channel::Receiver<Arc<SharedAudioTimeline>>,
        retired_tx: crossbeam_channel::Sender<Arc<SharedAudioTimeline>>,
    ) -> Self {
        Self {
            processor,
            timeline_rx,
            retired_tx,
            stashed_retired: None,
        }
    }

    /// Process a block of interleaved audio output samples.
    pub fn process(&mut self, data: &mut [f32]) {
        // If we previously couldn't return a retired timeline because the channel was full,
        // try draining it to retired_tx now.
        if let Some(stashed) = self.stashed_retired.take() {
            match self.retired_tx.try_send(stashed) {
                Ok(()) => {}
                Err(crossbeam_channel::TrySendError::Full(returned))
                | Err(crossbeam_channel::TrySendError::Disconnected(returned)) => {
                    self.stashed_retired = Some(returned);
                }
            }
        }

        // Check if a new timeline buffer was sent from the UI/worker thread.
        // Invariant: ONLY ingest a new timeline if stashed_retired is empty.
        // If retired_tx is saturated, pausing ingestion ensures we never drop or leak
        // an Arc on the audio thread, and timeline memory remains strictly bounded.
        if self.stashed_retired.is_none() {
            if let Ok(new_timeline) = self.timeline_rx.try_recv() {
                if let Some(old) = self.processor.set_audio_data(Some(new_timeline)) {
                    match self.retired_tx.try_send(old) {
                        Ok(()) => {}
                        Err(crossbeam_channel::TrySendError::Full(returned))
                        | Err(crossbeam_channel::TrySendError::Disconnected(returned)) => {
                            self.stashed_retired = Some(returned);
                        }
                    }
                }
            }
        }

        self.processor.process_interleaved(data);
    }
}
