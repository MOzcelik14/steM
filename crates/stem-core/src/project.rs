use crate::error::ProjectError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Non-destructive audio source file descriptor.
/// Source audio files on disk are NEVER modified by steM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioSource {
    pub id: Uuid,
    pub path: PathBuf,
    pub filename: String,
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_frames: u64,
    /// Optional SHA-256 or mtime hash to detect source file tampering.
    pub file_hash: Option<String>,
}

impl AudioSource {
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.duration_frames as f64 / self.sample_rate as f64
        }
    }
}

/// Curve type for clip fade-in and fade-out.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum FadeCurve {
    #[default]
    Linear,
    Logarithmic,
    Exponential,
    SCurve,
}

impl FadeCurve {
    /// Calculate gain multiplier for normalized time t in [0.0, 1.0].
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            FadeCurve::Linear => t,
            FadeCurve::Logarithmic => (1.0 + 9.0 * t).log10(),
            FadeCurve::Exponential => (10.0f32.powf(t) - 1.0) / 9.0,
            FadeCurve::SCurve => 0.5 * (1.0 - (std::f32::consts::PI * t).cos()),
        }
    }

    #[inline]
    pub fn evaluate_fade_in(&self, t: f32) -> f32 {
        self.evaluate(t)
    }

    #[inline]
    pub fn evaluate_fade_out(&self, t: f32) -> f32 {
        self.evaluate(1.0 - t.clamp(0.0, 1.0))
    }
}

/// Crossfade curve law applied across overlapping clip regions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CrossfadeType {
    Linear,
    #[default]
    EqualPower,
    Exponential,
}

impl CrossfadeType {
    /// Evaluates gain factors `(gain_a, gain_b)` for normalized transition progress `t` in `[0.0, 1.0]`.
    /// Equal-power maintains acoustic energy: `gain_a^2 + gain_b^2 == 1.0` (0 dB dip).
    pub fn evaluate(&self, t: f32) -> (f32, f32) {
        let t_cl = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => (1.0 - t_cl, t_cl),
            Self::EqualPower => {
                let angle = t_cl * (std::f32::consts::PI / 2.0);
                (angle.cos(), angle.sin())
            }
            Self::Exponential => {
                let a = 1.0 - t_cl;
                (a * a, t_cl * t_cl)
            }
        }
    }
}

/// A slice of audio referencing an immutable `AudioSource`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub id: Uuid,
    pub name: String,
    pub source_id: Uuid,
    /// Offset within the source audio file (in source frames).
    pub source_start_frame: u64,
    /// Number of frames this clip references from the source.
    pub length_frames: u64,
    /// Timeline position where this clip begins (in project frames).
    pub track_offset_frames: u64,
    /// Clip-specific volume gain in decibels.
    pub gain_db: f32,
    /// Fade-in length in frames.
    pub fade_in_frames: u64,
    pub fade_in_curve: FadeCurve,
    /// Fade-out length in frames.
    pub fade_out_frames: u64,
    pub fade_out_curve: FadeCurve,
    #[serde(default)]
    pub crossfade_type: CrossfadeType,
}

impl Clip {
    pub fn new(
        name: impl Into<String>,
        source_id: Uuid,
        source_start_frame: u64,
        length_frames: u64,
        track_offset_frames: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            source_id,
            source_start_frame,
            length_frames,
            track_offset_frames,
            gain_db: 0.0,
            fade_in_frames: 0,
            fade_in_curve: FadeCurve::Linear,
            fade_out_frames: 0,
            fade_out_curve: FadeCurve::Linear,
            crossfade_type: CrossfadeType::default(),
        }
    }

    /// End frame on the timeline (non-inclusive).
    #[inline]
    pub fn timeline_end_frame(&self) -> u64 {
        self.track_offset_frames + self.length_frames
    }

    /// Computes fade gain for a frame relative to clip start.
    #[inline]
    pub fn gain_at_relative_frame(&self, rel_frame: u64) -> f32 {
        let mut gain = 1.0f32;

        if self.fade_in_frames > 0 && rel_frame < self.fade_in_frames {
            let t = rel_frame as f32 / self.fade_in_frames as f32;
            gain *= self.fade_in_curve.evaluate(t);
        }

        if self.fade_out_frames > 0 && rel_frame + self.fade_out_frames >= self.length_frames {
            let frames_from_end = self.length_frames.saturating_sub(rel_frame);
            let t = frames_from_end as f32 / self.fade_out_frames as f32;
            gain *= self.fade_out_curve.evaluate(t);
        }

        gain
    }
}

/// Filter / Equalizer band configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    Peaking,
    LowShelf,
    HighShelf,
    BandPass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EqBandConfig {
    pub enabled: bool,
    pub filter_type: FilterType,
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl Default for EqBandConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain_db: 0.0,
            q: 1.0,
        }
    }
}

/// Serialized effect parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EffectConfig {
    Equalizer {
        bands: Vec<EqBandConfig>,
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    },
    Limiter {
        threshold_db: f32,
        release_ms: f32,
    },
    Delay {
        time_ms: f32,
        feedback: f32,
        mix: f32,
    },
    Reverb {
        room_size: f32,
        damping: f32,
        mix: f32,
    },
    NoiseGate {
        threshold_db: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
        range_db: f32,
    },
    DeEsser {
        frequency_hz: f32,
        threshold_db: f32,
        ratio: f32,
        range_db: f32,
    },
}

/// Count-in countdown duration before recording begins.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CountInMode {
    #[default]
    Off,
    OneBar,
    TwoBars,
    TwoSeconds,
    FourSeconds,
}

impl CountInMode {
    pub fn duration_seconds(&self, bpm: f64, beats_per_bar: u8) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::OneBar => (beats_per_bar as f32 * 60.0) / bpm.max(1.0) as f32,
            Self::TwoBars => (2.0 * beats_per_bar as f32 * 60.0) / bpm.max(1.0) as f32,
            Self::TwoSeconds => 2.0,
            Self::FourSeconds => 4.0,
        }
    }
}

/// Hardware input routing configuration for a track.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TrackInputRouting {
    #[default]
    Mono1, // Input Channel 1 (Left)
    Mono2,  // Input Channel 2 (Right)
    Stereo, // Input Channels 1 + 2 (Stereo Pair)
    None,
}

impl TrackInputRouting {
    /// Extracts routed planar audio channels from raw multi-channel input planar buffer.
    pub fn extract_channels(&self, input_planar: &[Vec<f32>]) -> Vec<Vec<f32>> {
        match self {
            Self::Mono1 => {
                if !input_planar.is_empty() {
                    vec![input_planar[0].clone()]
                } else {
                    vec![Vec::new()]
                }
            }
            Self::Mono2 => {
                if input_planar.len() > 1 {
                    vec![input_planar[1].clone()]
                } else if !input_planar.is_empty() {
                    vec![input_planar[0].clone()]
                } else {
                    vec![Vec::new()]
                }
            }
            Self::Stereo => {
                if input_planar.len() >= 2 {
                    vec![input_planar[0].clone(), input_planar[1].clone()]
                } else if input_planar.len() == 1 {
                    vec![input_planar[0].clone(), input_planar[0].clone()]
                } else {
                    vec![Vec::new(), Vec::new()]
                }
            }
            Self::None => Vec::new(),
        }
    }
}

/// Supported project/export channel layouts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ChannelLayout {
    Mono,
    #[default]
    Stereo,
    Surround51,
}

impl ChannelLayout {
    pub fn channel_count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround51 => 6,
        }
    }
}

/// Output destination routing for a track or bus.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TrackOutputRouting {
    #[default]
    Master,
    Bus(Uuid),
}

/// An auxiliary send routing a tap of a track's audio to a submix bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackSend {
    pub id: Uuid,
    pub target_bus_id: Uuid,
    pub gain_db: f32,
    pub pan: f32,
    pub pre_fader: bool,
    pub muted: bool,
}

impl TrackSend {
    pub fn new(target_bus_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            target_bus_id,
            gain_db: 0.0,
            pan: 0.0,
            pre_fader: false,
            muted: false,
        }
    }
}

/// Submix auxiliary bus for grouping tracks, summing signals, and hosting insert effect chains.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubmixBus {
    pub id: Uuid,
    pub name: String,
    pub gain_db: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    #[serde(default)]
    pub output_routing: TrackOutputRouting,
    pub effects: Vec<EffectConfig>,
}

impl SubmixBus {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            output_routing: TrackOutputRouting::Master,
            effects: Vec::new(),
        }
    }
}

/// Track group for linking faders, mutes, solos, and record arms across multiple tracks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackGroup {
    pub id: Uuid,
    pub name: String,
    pub color_rgb: (u8, u8, u8),
    pub link_gain: bool,
    pub link_pan: bool,
    pub link_mute: bool,
    pub link_solo: bool,
    pub link_arm: bool,
}

impl TrackGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            color_rgb: (100, 180, 240),
            link_gain: true,
            link_pan: false,
            link_mute: true,
            link_solo: true,
            link_arm: true,
        }
    }
}

/// Interpolation curve between automation breakpoints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AutomationCurve {
    #[default]
    Linear,
    Bezier,
    Step,
}

impl AutomationCurve {
    pub fn interpolate(&self, t: f32, val_a: f32, val_b: f32) -> f32 {
        let t_cl = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => val_a + (val_b - val_a) * t_cl,
            Self::Step => {
                if t_cl < 1.0 {
                    val_a
                } else {
                    val_b
                }
            }
            Self::Bezier => {
                let smooth = t_cl * t_cl * (3.0 - 2.0 * t_cl);
                val_a + (val_b - val_a) * smooth
            }
        }
    }
}

/// External audio plugin standard supported by steM.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PluginFormat {
    Vst3,
    Lv2,
}

/// Persisted parameter state for an external plugin slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginParamValue {
    pub id: String,
    pub name: String,
    pub value: f32,
    pub default_value: f32,
}

/// Persisted external plugin insert slot on a track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginSlotConfig {
    pub id: Uuid,
    pub descriptor_id: String,
    pub name: String,
    pub vendor: String,
    pub format: PluginFormat,
    pub bundle_path: PathBuf,
    #[serde(default)]
    pub bypassed: bool,
    #[serde(default)]
    pub parameters: Vec<PluginParamValue>,
    #[serde(default)]
    pub state_blob: Option<Vec<u8>>,
}

/// Target parameter controlled by an automation lane.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutomationTarget {
    Volume, // decibels (-60.0 to +12.0)
    Pan,    // -1.0 to +1.0
    PluginParam { plugin_id: Uuid, param_index: u32 },
    SendLevel(usize),    // decibels (-60.0 to +12.0) for track send index
    EqFreq(usize),       // Hz (20.0 to 20000.0) for EQ band index
    EqGain(usize),       // dB (-24.0 to +24.0) for EQ band index
    EqQ(usize),          // Q factor (0.1 to 16.0) for EQ band index
    CompressorThreshold, // dB (-60.0 to 0.0)
    CompressorRatio,     // ratio (1.0 to 20.0)
    ReverbMix,           // 0.0 to 1.0
    ReverbRoomSize,      // 0.0 to 1.0
}

impl AutomationTarget {
    /// Returns `(min, max, zero_or_default)` range for visualization and clamping.
    pub fn value_range(&self) -> (f32, f32, f32) {
        match self {
            Self::Volume | Self::SendLevel(_) => (-60.0, 12.0, 0.0),
            Self::Pan => (-1.0, 1.0, 0.0),
            Self::PluginParam { .. } | Self::ReverbMix | Self::ReverbRoomSize => (0.0, 1.0, 0.5),
            Self::EqFreq(_) => (20.0, 20_000.0, 1_000.0),
            Self::EqGain(_) => (-24.0, 24.0, 0.0),
            Self::EqQ(_) => (0.1, 16.0, 1.0),
            Self::CompressorThreshold => (-60.0, 0.0, -18.0),
            Self::CompressorRatio => (1.0, 20.0, 4.0),
        }
    }
}

/// A single keyframe breakpoint in an automation lane.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct AutomationPoint {
    pub id: Uuid,
    pub frame: u64,
    pub value: f32,
    pub curve: AutomationCurve,
}

impl AutomationPoint {
    pub fn new(frame: u64, value: f32, curve: AutomationCurve) -> Self {
        Self {
            id: Uuid::new_v4(),
            frame,
            value,
            curve,
        }
    }
}

/// An automation lane storing a sequence of breakpoints for a parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutomationLane {
    pub target: AutomationTarget,
    pub enabled: bool,
    pub points: Vec<AutomationPoint>,
}

impl AutomationLane {
    pub fn new(target: AutomationTarget) -> Self {
        Self {
            target,
            enabled: false,
            points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, frame: u64, value: f32, curve: AutomationCurve) -> Uuid {
        let pt = AutomationPoint::new(frame, value, curve);
        let id = pt.id;
        self.points.push(pt);
        self.points.sort_by_key(|p| p.frame);
        id
    }

    pub fn remove_point(&mut self, id: Uuid) -> bool {
        if let Some(pos) = self.points.iter().position(|p| p.id == id) {
            self.points.remove(pos);
            true
        } else {
            false
        }
    }

    /// Copies automation points in `[start_frame, end_frame]` with frames normalized
    /// relative to `start_frame`.
    pub fn copy_range(&self, start_frame: u64, end_frame: u64) -> Vec<AutomationPoint> {
        let (lo, hi) = if start_frame <= end_frame {
            (start_frame, end_frame)
        } else {
            (end_frame, start_frame)
        };
        self.points
            .iter()
            .filter(|p| p.frame >= lo && p.frame <= hi)
            .map(|p| AutomationPoint::new(p.frame - lo, p.value, p.curve))
            .collect()
    }

    /// Pastes relative automation points starting at `target_frame`, generating fresh point IDs
    /// and replacing any existing points at the exact target frames.
    pub fn paste_at(&mut self, target_frame: u64, points: &[AutomationPoint]) {
        for pt in points {
            let abs_frame = target_frame.saturating_add(pt.frame);
            self.points.retain(|existing| existing.frame != abs_frame);
            self.points
                .push(AutomationPoint::new(abs_frame, pt.value, pt.curve));
        }
        self.points.sort_by_key(|p| p.frame);
    }

    /// Piecewise evaluation at timeline frame.
    pub fn evaluate(&self, frame: u64, default_val: f32) -> f32 {
        if !self.enabled || self.points.is_empty() {
            return default_val;
        }

        if let Some(first) = self.points.first() {
            if frame <= first.frame {
                return first.value;
            }
        }
        if let Some(last) = self.points.last() {
            if frame >= last.frame {
                return last.value;
            }
        }

        for w in self.points.windows(2) {
            if let [p1, p2] = w {
                if frame >= p1.frame && frame <= p2.frame {
                    let range = (p2.frame.saturating_sub(p1.frame)).max(1) as f32;
                    let t = (frame.saturating_sub(p1.frame)) as f32 / range;
                    return p1.curve.interpolate(t, p1.value, p2.value);
                }
            }
        }

        default_val
    }
}

/// A single audio track containing clips and effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: Uuid,
    pub name: String,
    pub gain_db: f32,
    pub pan: f32, // -1.0 (Left) to +1.0 (Right), 0.0 Center
    pub muted: bool,
    pub soloed: bool,
    #[serde(default)]
    pub armed: bool,
    #[serde(default)]
    pub input_routing: TrackInputRouting,
    #[serde(default)]
    pub group_id: Option<Uuid>,
    #[serde(default)]
    pub output_routing: TrackOutputRouting,
    #[serde(default)]
    pub sends: Vec<TrackSend>,
    #[serde(default)]
    pub automation_lanes: Vec<AutomationLane>,
    pub clips: Vec<Clip>,
    pub effects: Vec<EffectConfig>,
    #[serde(default)]
    pub plugins: Vec<PluginSlotConfig>,
    #[serde(default)]
    pub phase_invert_left: bool,
    #[serde(default)]
    pub phase_invert_right: bool,
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default)]
    pub mid_gain_db: f32,
    #[serde(default)]
    pub side_gain_db: f32,
    #[serde(default)]
    pub sidechain_source_track_id: Option<Uuid>,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub frozen_audio_path: Option<PathBuf>,
    #[serde(default)]
    pub surround_rear_mix: f32,
    #[serde(default = "default_surround_center")]
    pub surround_center_mix: f32,
    #[serde(default = "default_surround_lfe")]
    pub surround_lfe_mix: f32,
}

fn default_stereo_width() -> f32 {
    1.0
}

fn default_surround_center() -> f32 {
    0.5
}

fn default_surround_lfe() -> f32 {
    0.25
}

impl Track {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            gain_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            armed: false,
            input_routing: TrackInputRouting::default(),
            group_id: None,
            output_routing: TrackOutputRouting::default(),
            sends: Vec::new(),
            automation_lanes: vec![
                AutomationLane::new(AutomationTarget::Volume),
                AutomationLane::new(AutomationTarget::Pan),
            ],
            clips: Vec::new(),
            effects: Vec::new(),
            plugins: Vec::new(),
            phase_invert_left: false,
            phase_invert_right: false,
            stereo_width: 1.0,
            mid_gain_db: 0.0,
            side_gain_db: 0.0,
            sidechain_source_track_id: None,
            frozen: false,
            frozen_audio_path: None,
            surround_rear_mix: 0.0,
            surround_center_mix: 0.5,
            surround_lfe_mix: 0.25,
        }
    }

    pub fn total_end_frame(&self) -> u64 {
        self.clips
            .iter()
            .map(|c| c.timeline_end_frame())
            .max()
            .unwrap_or(0)
    }

    pub fn sort_clips(&mut self) {
        self.clips.sort_by_key(|c| c.track_offset_frames);
    }

    /// Evaluates gain factor for a clip at timeline frame `frame`, taking into account
    /// clip fades and automatic crossfades with overlapping adjacent clips on the same track.
    pub fn clip_gain_at_timeline_frame(&self, clip_idx: usize, frame: u64) -> f32 {
        let clip = match self.clips.get(clip_idx) {
            Some(c) => c,
            None => return 0.0,
        };

        if frame < clip.track_offset_frames || frame >= clip.timeline_end_frame() {
            return 0.0;
        }

        let rel_frame = frame - clip.track_offset_frames;
        let mut gain = clip.gain_at_relative_frame(rel_frame);

        // Check crossfade with next clip on the track (overlap at the end of this clip)
        if let Some(next_clip) = self.clips.get(clip_idx + 1) {
            let this_end = clip.timeline_end_frame();
            if next_clip.track_offset_frames < this_end && frame >= next_clip.track_offset_frames {
                let overlap_len = this_end
                    .saturating_sub(next_clip.track_offset_frames)
                    .max(1);
                let t = (frame - next_clip.track_offset_frames) as f32 / overlap_len as f32;
                let (gain_a, _) = next_clip.crossfade_type.evaluate(t);
                gain = gain.min(gain_a);
            }
        }

        // Check crossfade with previous clip on the track (overlap at the start of this clip)
        if clip_idx > 0 {
            if let Some(prev_clip) = self.clips.get(clip_idx - 1) {
                let prev_end = prev_clip.timeline_end_frame();
                if prev_end > clip.track_offset_frames && frame < prev_end {
                    let overlap_len = prev_end.saturating_sub(clip.track_offset_frames).max(1);
                    let t = (frame - clip.track_offset_frames) as f32 / overlap_len as f32;
                    let (_, gain_b) = clip.crossfade_type.evaluate(t);
                    gain = gain.min(gain_b);
                }
            }
        }

        gain
    }

    /// Evaluated volume gain in dB at given timeline frame, taking automation into account.
    pub fn evaluated_gain_db(&self, frame: u64) -> f32 {
        self.automation_lanes
            .iter()
            .find(|l| l.target == AutomationTarget::Volume && l.enabled)
            .map(|l| l.evaluate(frame, self.gain_db))
            .unwrap_or(self.gain_db)
    }

    /// Evaluated pan (-1.0 to +1.0) at given timeline frame, taking automation into account.
    pub fn evaluated_pan(&self, frame: u64) -> f32 {
        self.automation_lanes
            .iter()
            .find(|l| l.target == AutomationTarget::Pan && l.enabled)
            .map(|l| l.evaluate(frame, self.pan))
            .unwrap_or(self.pan)
    }

    /// Evaluated send gain in dB for `send_idx` at given timeline frame, taking automation into account.
    pub fn evaluated_send_gain_db(&self, send_idx: usize, frame: u64) -> f32 {
        let base_gain = self.sends.get(send_idx).map(|s| s.gain_db).unwrap_or(-60.0);
        self.automation_lanes
            .iter()
            .find(|l| l.target == AutomationTarget::SendLevel(send_idx) && l.enabled)
            .map(|l| l.evaluate(frame, base_gain))
            .unwrap_or(base_gain)
    }

    /// Evaluates any automation target on this track at `frame`, falling back to `default_val`.
    pub fn evaluated_param(&self, target: AutomationTarget, default_val: f32, frame: u64) -> f32 {
        self.automation_lanes
            .iter()
            .find(|l| l.target == target && l.enabled)
            .map(|l| l.evaluate(frame, default_val))
            .unwrap_or(default_val)
    }

    /// Returns the automation lane for `target`, creating and inserting it if missing.
    pub fn ensure_automation_lane(&mut self, target: AutomationTarget) -> &mut AutomationLane {
        if let Some(pos) = self
            .automation_lanes
            .iter()
            .position(|l| l.target == target)
        {
            return &mut self.automation_lanes[pos];
        }
        self.automation_lanes.push(AutomationLane::new(target));
        let last = self.automation_lanes.len() - 1;
        &mut self.automation_lanes[last]
    }
}

/// A timeline marker or CD track cue point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Marker {
    pub id: Uuid,
    pub frame: u64,
    pub name: String,
    pub color_rgb: (u8, u8, u8),
    pub is_cd_track: bool,
}

impl Marker {
    pub fn new(frame: u64, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            frame,
            name: name.into(),
            color_rgb: (80, 200, 255),
            is_cd_track: false,
        }
    }
}

/// Maximum number of tracks supported consistently across project validation and real-time audio engine.
pub const MAX_PROJECT_TRACKS: usize = 64;

fn default_schema_version() -> u32 {
    1
}

/// User-defined reusable insert effect chain preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectChainPreset {
    pub id: Uuid,
    pub name: String,
    pub effects: Vec<EffectConfig>,
}

impl EffectChainPreset {
    pub fn new(name: impl Into<String>, effects: Vec<EffectConfig>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            effects,
        }
    }

    pub fn to_ron(&self) -> Result<String, ProjectError> {
        let pretty_config = ron::ser::PrettyConfig::new()
            .depth_limit(6)
            .separate_tuple_members(true)
            .enumerate_arrays(true);
        Ok(ron::ser::to_string_pretty(self, pretty_config)?)
    }

    pub fn from_ron(s: &str) -> Result<Self, ProjectError> {
        const MAX_PRESET_RON_BYTES: usize = 1024 * 1024;
        if s.len() > MAX_PRESET_RON_BYTES {
            return Err(ProjectError::Validation(format!(
                "Effect preset RON exceeds maximum size ({} bytes)",
                MAX_PRESET_RON_BYTES
            )));
        }
        Ok(ron::from_str(s)?)
    }
}

/// The complete non-destructive project representation in steM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub sample_rate: u32,
    pub tempo_bpm: f64,
    pub time_signature: (u8, u8),
    pub sources: HashMap<Uuid, AudioSource>,
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub busses: Vec<SubmixBus>,
    #[serde(default)]
    pub groups: Vec<TrackGroup>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    pub master_gain_db: f32,
    #[serde(default)]
    pub master_effects: Vec<EffectConfig>,
    #[serde(default)]
    pub user_presets: Vec<EffectChainPreset>,
    pub loop_enabled: bool,
    pub loop_region: Option<(u64, u64)>,
    pub playhead_frame: u64,
    #[serde(default)]
    pub punch_in_enabled: bool,
    #[serde(default)]
    pub punch_region: Option<(u64, u64)>,
    #[serde(default)]
    pub count_in_mode: CountInMode,
    #[serde(default)]
    pub count_in_bars: u8,
    #[serde(default)]
    pub input_monitoring: bool,
    #[serde(default)]
    pub latency_compensation_ms: f32,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled Project", 48000)
    }
}

impl Project {
    pub fn new(name: impl Into<String>, sample_rate: u32) -> Self {
        let mut proj = Self {
            schema_version: 1,
            id: Uuid::new_v4(),
            name: name.into(),
            sample_rate,
            tempo_bpm: 120.0,
            time_signature: (4, 4),
            sources: HashMap::new(),
            tracks: Vec::new(),
            busses: Vec::new(),
            groups: Vec::new(),
            markers: Vec::new(),
            master_gain_db: 0.0,
            master_effects: Vec::new(),
            user_presets: Vec::new(),
            loop_enabled: false,
            loop_region: None,
            playhead_frame: 0,
            punch_in_enabled: false,
            punch_region: None,
            count_in_mode: CountInMode::default(),
            count_in_bars: 0,
            input_monitoring: false,
            latency_compensation_ms: 20.0, // standard 20ms audio buffer latency compensation
        };
        // Provide 2 default audio tracks
        proj.add_track("Track 1");
        proj.add_track("Track 2");
        proj
    }

    /// Latency compensation offset in samples/frames.
    #[inline]
    pub fn latency_compensation_frames(&self) -> u64 {
        ((self.latency_compensation_ms * self.sample_rate as f32) / 1000.0).round() as u64
    }

    pub fn add_track(&mut self, name: impl Into<String>) -> Uuid {
        let track = Track::new(name);
        let id = track.id;
        self.tracks.push(track);
        id
    }

    pub fn remove_track(&mut self, track_id: Uuid) -> Option<Track> {
        if let Some(pos) = self.tracks.iter().position(|t| t.id == track_id) {
            Some(self.tracks.remove(pos))
        } else {
            None
        }
    }

    pub fn get_track(&self, track_id: Uuid) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == track_id)
    }

    pub fn get_track_mut(&mut self, track_id: Uuid) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == track_id)
    }

    pub fn add_bus(&mut self, name: impl Into<String>) -> Uuid {
        let bus = SubmixBus::new(name);
        let id = bus.id;
        self.busses.push(bus);
        id
    }

    pub fn remove_bus(&mut self, bus_id: Uuid) -> Option<SubmixBus> {
        if let Some(pos) = self.busses.iter().position(|b| b.id == bus_id) {
            let removed = self.busses.remove(pos);
            // Re-route any tracks pointing to this bus back to Master
            for track in &mut self.tracks {
                if track.output_routing == TrackOutputRouting::Bus(bus_id) {
                    track.output_routing = TrackOutputRouting::Master;
                }
                track.sends.retain(|s| s.target_bus_id != bus_id);
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn get_bus(&self, bus_id: Uuid) -> Option<&SubmixBus> {
        self.busses.iter().find(|b| b.id == bus_id)
    }

    pub fn get_bus_mut(&mut self, bus_id: Uuid) -> Option<&mut SubmixBus> {
        self.busses.iter_mut().find(|b| b.id == bus_id)
    }

    pub fn add_group(&mut self, name: impl Into<String>) -> Uuid {
        let group = TrackGroup::new(name);
        let id = group.id;
        self.groups.push(group);
        id
    }

    pub fn remove_group(&mut self, group_id: Uuid) -> Option<TrackGroup> {
        if let Some(pos) = self.groups.iter().position(|g| g.id == group_id) {
            let removed = self.groups.remove(pos);
            for track in &mut self.tracks {
                if track.group_id == Some(group_id) {
                    track.group_id = None;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn get_group(&self, group_id: Uuid) -> Option<&TrackGroup> {
        self.groups.iter().find(|g| g.id == group_id)
    }

    pub fn get_group_mut(&mut self, group_id: Uuid) -> Option<&mut TrackGroup> {
        self.groups.iter_mut().find(|g| g.id == group_id)
    }

    pub fn add_marker(&mut self, frame: u64, name: impl Into<String>) -> Uuid {
        let marker = Marker::new(frame, name);
        let id = marker.id;
        self.markers.push(marker);
        self.markers.sort_by_key(|m| m.frame);
        id
    }

    pub fn remove_marker(&mut self, marker_id: Uuid) -> Option<Marker> {
        if let Some(pos) = self.markers.iter().position(|m| m.id == marker_id) {
            Some(self.markers.remove(pos))
        } else {
            None
        }
    }

    pub fn find_prev_marker_frame(&self, current_frame: u64) -> Option<u64> {
        self.markers
            .iter()
            .rev()
            .find(|m| m.frame + 10 < current_frame)
            .map(|m| m.frame)
    }

    pub fn find_next_marker_frame(&self, current_frame: u64) -> Option<u64> {
        self.markers
            .iter()
            .find(|m| m.frame > current_frame + 10)
            .map(|m| m.frame)
    }

    pub fn add_source(&mut self, source: AudioSource) {
        self.sources.insert(source.id, source);
    }

    pub fn add_clip_to_track(&mut self, track_id: Uuid, clip: Clip) -> Result<(), ProjectError> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(ProjectError::TrackNotFound(track_id))?;
        track.clips.push(clip);
        Ok(())
    }

    /// Non-destructively inserts a punch-in take into a track.
    /// Existing clips overlapping the punch region are cleanly trimmed or split around the punch take.
    pub fn punch_clip_into_track(
        &mut self,
        track_id: Uuid,
        punch_take: Clip,
    ) -> Result<(), ProjectError> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(ProjectError::TrackNotFound(track_id))?;

        let punch_start = punch_take.track_offset_frames;
        let punch_end = punch_take.timeline_end_frame();

        if punch_end <= punch_start {
            return Ok(());
        }

        let mut new_clips = Vec::with_capacity(track.clips.len() + 2);

        for clip in track.clips.drain(..) {
            let clip_start = clip.track_offset_frames;
            let clip_end = clip.timeline_end_frame();

            // Case A: No overlap
            if clip_end <= punch_start || clip_start >= punch_end {
                new_clips.push(clip);
            }
            // Case B: Completely covered by punch take
            else if clip_start >= punch_start && clip_end <= punch_end {
                // Superseded by punch take
                continue;
            }
            // Case C: Punch take splits clip in the middle
            else if clip_start < punch_start && clip_end > punch_end {
                let left_len = punch_start - clip_start;
                let right_len = clip_end - punch_end;

                let mut left = clip.clone();
                left.length_frames = left_len;
                left.fade_out_frames = left.fade_out_frames.min(left_len);
                new_clips.push(left);

                let mut right = clip.clone();
                right.id = Uuid::new_v4();
                right.track_offset_frames = punch_end;
                right.source_start_frame = clip.source_start_frame + (punch_end - clip_start);
                right.length_frames = right_len;
                right.fade_in_frames = right.fade_in_frames.min(right_len);
                new_clips.push(right);
            }
            // Case D: Overlaps left edge of punch region
            else if clip_start < punch_start && clip_end > punch_start {
                let left_len = punch_start - clip_start;
                let mut left = clip;
                left.length_frames = left_len;
                left.fade_out_frames = left.fade_out_frames.min(left_len);
                new_clips.push(left);
            }
            // Case E: Overlaps right edge of punch region
            else if clip_start < punch_end && clip_end > punch_end {
                let cut_delta = punch_end - clip_start;
                let right_len = clip_end - punch_end;
                let mut right = clip;
                right.track_offset_frames = punch_end;
                right.source_start_frame += cut_delta;
                right.length_frames = right_len;
                right.fade_in_frames = right.fade_in_frames.min(right_len);
                new_clips.push(right);
            }
        }

        new_clips.push(punch_take);
        new_clips.sort_by_key(|c| c.track_offset_frames);
        track.clips = new_clips;

        Ok(())
    }

    /// Split a clip at a specific timeline frame non-destructively.
    pub fn split_clip(
        &mut self,
        track_id: Uuid,
        clip_id: Uuid,
        split_timeline_frame: u64,
    ) -> Result<(Uuid, Uuid), ProjectError> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(ProjectError::TrackNotFound(track_id))?;
        let clip_idx = track
            .clips
            .iter()
            .position(|c| c.id == clip_id)
            .ok_or(ProjectError::ClipNotFound(clip_id))?;

        let orig = &track.clips[clip_idx];
        if split_timeline_frame <= orig.track_offset_frames
            || split_timeline_frame >= orig.timeline_end_frame()
        {
            return Err(ProjectError::InvalidClipBounds {
                start: orig.track_offset_frames,
                length: orig.length_frames,
                source_len: split_timeline_frame,
            });
        }

        let first_len = split_timeline_frame - orig.track_offset_frames;
        let second_len = orig.length_frames - first_len;

        let mut first = orig.clone();
        first.length_frames = first_len;
        first.fade_out_frames = first.fade_out_frames.min(first_len);

        let mut second = orig.clone();
        second.id = Uuid::new_v4();
        second.track_offset_frames = split_timeline_frame;
        second.source_start_frame = orig.source_start_frame + first_len;
        second.length_frames = second_len;
        second.fade_in_frames = 0; // Fresh split doesn't fade immediately
        second.fade_out_frames = orig.fade_out_frames.min(second_len);

        let id1 = first.id;
        let id2 = second.id;

        track.clips[clip_idx] = first;
        track.clips.insert(clip_idx + 1, second);

        Ok((id1, id2))
    }

    /// Total timeline length in frames.
    pub fn total_frames(&self) -> u64 {
        self.tracks
            .iter()
            .map(|t| t.total_end_frame())
            .max()
            .unwrap_or(0)
    }

    pub fn total_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.total_frames() as f64 / self.sample_rate as f64
        }
    }

    /// Validates all semantic project invariants to defend against malformed or malicious inputs.
    pub fn validate(&self) -> Result<(), ProjectError> {
        if !(8000..=192000).contains(&self.sample_rate) {
            return Err(ProjectError::Validation(format!(
                "Invalid sample rate: {} Hz (must be between 8000 and 192000)",
                self.sample_rate
            )));
        }

        if !self.tempo_bpm.is_finite() || self.tempo_bpm <= 0.0 || self.tempo_bpm > 1000.0 {
            return Err(ProjectError::Validation(format!(
                "Invalid tempo: {} BPM (must be finite and between 0.1 and 1000.0)",
                self.tempo_bpm
            )));
        }

        if self.time_signature.0 == 0 || self.time_signature.1 == 0 {
            return Err(ProjectError::Validation(format!(
                "Invalid time signature: {}/{}",
                self.time_signature.0, self.time_signature.1
            )));
        }

        if !self.master_gain_db.is_finite() || !(-120.0..=30.0).contains(&self.master_gain_db) {
            return Err(ProjectError::Validation(format!(
                "Invalid master gain: {} dB",
                self.master_gain_db
            )));
        }

        if !self.latency_compensation_ms.is_finite()
            || !(0.0..=5000.0).contains(&self.latency_compensation_ms)
        {
            return Err(ProjectError::Validation(format!(
                "Invalid latency compensation: {} ms",
                self.latency_compensation_ms
            )));
        }

        // Validate Audio Sources
        for (source_id, source) in &self.sources {
            if source.id != *source_id {
                return Err(ProjectError::Validation(format!(
                    "Source key ID mismatch: key {}, source id {}",
                    source_id, source.id
                )));
            }
            if !(1..=8).contains(&source.channels) {
                return Err(ProjectError::Validation(format!(
                    "Source {} has invalid channel count: {}",
                    source_id, source.channels
                )));
            }
            if !(8000..=192000).contains(&source.sample_rate) {
                return Err(ProjectError::Validation(format!(
                    "Source {} has invalid sample rate: {} Hz",
                    source_id, source.sample_rate
                )));
            }

            // Path Traversal Check
            if source.path.as_os_str().is_empty() {
                return Err(ProjectError::Validation(format!(
                    "Source {} has empty path",
                    source_id
                )));
            }
            if source
                .path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(ProjectError::Validation(format!(
                    "Source {} path contains prohibited '..' directory traversal: {:?}",
                    source_id, source.path
                )));
            }
        }

        // Validate Tracks
        if self.tracks.len() > MAX_PROJECT_TRACKS {
            return Err(ProjectError::Validation(format!(
                "Too many tracks: {} (max {})",
                self.tracks.len(),
                MAX_PROJECT_TRACKS
            )));
        }

        for (t_idx, track) in self.tracks.iter().enumerate() {
            if !track.gain_db.is_finite() || !(-120.0..=30.0).contains(&track.gain_db) {
                return Err(ProjectError::Validation(format!(
                    "Track {} ({}) has invalid gain: {} dB",
                    t_idx, track.name, track.gain_db
                )));
            }
            if !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan) {
                return Err(ProjectError::Validation(format!(
                    "Track {} ({}) has invalid pan: {}",
                    t_idx, track.name, track.pan
                )));
            }

            // Validate Clips
            for (c_idx, clip) in track.clips.iter().enumerate() {
                if clip.length_frames == 0 {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} has zero length",
                        t_idx, c_idx
                    )));
                }

                if clip
                    .source_start_frame
                    .checked_add(clip.length_frames)
                    .is_none()
                {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} source bounds overflow u64",
                        t_idx, c_idx
                    )));
                }

                if clip
                    .track_offset_frames
                    .checked_add(clip.length_frames)
                    .is_none()
                {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} timeline offset overflow u64",
                        t_idx, c_idx
                    )));
                }

                // If source is known in project, ensure clip does not exceed source length
                if let Some(src) = self.sources.get(&clip.source_id) {
                    if clip.source_start_frame + clip.length_frames > src.duration_frames {
                        return Err(ProjectError::InvalidClipBounds {
                            start: clip.source_start_frame,
                            length: clip.length_frames,
                            source_len: src.duration_frames,
                        });
                    }
                }

                if !clip.gain_db.is_finite() || !(-120.0..=30.0).contains(&clip.gain_db) {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} has invalid gain: {} dB",
                        t_idx, c_idx, clip.gain_db
                    )));
                }

                if clip.fade_in_frames > clip.length_frames {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} fade-in frames ({}) exceed clip length ({})",
                        t_idx, c_idx, clip.fade_in_frames, clip.length_frames
                    )));
                }

                if clip.fade_out_frames > clip.length_frames {
                    return Err(ProjectError::Validation(format!(
                        "Track {} clip {} fade-out frames ({}) exceed clip length ({})",
                        t_idx, c_idx, clip.fade_out_frames, clip.length_frames
                    )));
                }
            }
        }

        // Validate Loop Region
        if let Some((start, end)) = self.loop_region {
            if start >= end {
                return Err(ProjectError::Validation(format!(
                    "Loop region start ({}) must be less than end ({})",
                    start, end
                )));
            }
        }

        // Validate Punch Region
        if let Some((start, end)) = self.punch_region {
            if start >= end {
                return Err(ProjectError::Validation(format!(
                    "Punch region start ({}) must be less than end ({})",
                    start, end
                )));
            }
        }

        Ok(())
    }

    /// Serializes project state to RON.
    pub fn to_ron(&self) -> Result<String, ProjectError> {
        let pretty_config = ron::ser::PrettyConfig::new()
            .depth_limit(6)
            .separate_tuple_members(true)
            .enumerate_arrays(true);
        Ok(ron::ser::to_string_pretty(self, pretty_config)?)
    }

    /// Deserializes project from RON string and validates all semantic constraints.
    pub fn from_ron(s: &str) -> Result<Self, ProjectError> {
        const MAX_PROJECT_RON_BYTES: usize = 32 * 1024 * 1024;
        if s.len() > MAX_PROJECT_RON_BYTES {
            return Err(ProjectError::Validation(format!(
                "Project RON exceeds maximum size ({} bytes)",
                MAX_PROJECT_RON_BYTES
            )));
        }
        let proj: Self = ron::from_str(s)?;
        proj.validate()?;
        Ok(proj)
    }

    /// Saves the project atomically via a temporary sibling file, `sync_all`, and rename.
    pub fn save_atomic(&self, dest: &std::path::Path) -> Result<(), ProjectError> {
        let ron_data = self.to_ron()?;
        crate::fs_util::atomic_write_bytes(dest, ron_data.as_bytes())?;
        Ok(())
    }

    /// Creates a `.bak` backup copy of an existing project file if it exists.
    pub fn create_backup(project_path: &std::path::Path) -> Result<Option<PathBuf>, ProjectError> {
        if !project_path.exists() {
            return Ok(None);
        }
        let bak_path = project_path.with_extension("stem.bak");
        std::fs::copy(project_path, &bak_path)?;
        Ok(Some(bak_path))
    }

    /// Collect All and Save: copies all existing external audio sources into `<project_dir>/Audio/`,
    /// updates source paths, creates a backup if an existing file is present, and saves atomically.
    /// Returns the number of media files copied.
    pub fn collect_all_and_save(
        &mut self,
        dest_project_path: &std::path::Path,
    ) -> Result<usize, ProjectError> {
        let project_dir = dest_project_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let audio_dir = project_dir.join("Audio");
        std::fs::create_dir_all(&audio_dir)?;

        let mut claimed_targets: std::collections::HashSet<PathBuf> =
            std::collections::HashSet::new();
        let mut copied_count = 0usize;
        for src in self.sources.values_mut() {
            if !src.path.exists() {
                continue;
            }
            let mut target_path = if let Some(file_name) = src.path.file_name() {
                audio_dir.join(file_name)
            } else {
                audio_dir.join(format!("{}.wav", src.id))
            };
            if src.path != target_path
                && (claimed_targets.contains(&target_path) || target_path.exists())
            {
                let stem = src
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("audio");
                let ext = src
                    .path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("wav");
                let short_id = &src.id.to_string()[..8];
                target_path = audio_dir.join(format!("{stem}_{short_id}.{ext}"));
            }
            claimed_targets.insert(target_path.clone());
            if src.path != target_path {
                std::fs::copy(&src.path, &target_path)?;
                src.path = target_path;
                copied_count += 1;
            }
        }

        let _ = Self::create_backup(dest_project_path)?;
        self.save_atomic(dest_project_path)?;
        Ok(copied_count)
    }

    /// Creates a new project initialized from a built-in workflow template.
    pub fn from_template(template: ProjectTemplate, sample_rate: u32) -> Self {
        let mut proj = Self::new("Yeni Proje", sample_rate);
        proj.tracks.clear();

        match template {
            ProjectTemplate::Empty => {
                proj.tracks.push(Track::new("Kanal 1"));
            }
            ProjectTemplate::PodcastTwoHosts => {
                proj.name = "Podcast (2 Konuşmacı)".to_string();
                let vocal_chain = || {
                    vec![
                        EffectConfig::NoiseGate {
                            threshold_db: -48.0,
                            attack_ms: 2.0,
                            hold_ms: 80.0,
                            release_ms: 150.0,
                            range_db: -40.0,
                        },
                        EffectConfig::DeEsser {
                            frequency_hz: 6500.0,
                            threshold_db: -22.0,
                            ratio: 4.0,
                            range_db: -9.0,
                        },
                        EffectConfig::Compressor {
                            threshold_db: -18.0,
                            ratio: 3.0,
                            attack_ms: 10.0,
                            release_ms: 120.0,
                            makeup_gain_db: 3.0,
                        },
                    ]
                };

                let mut host = Track::new("Sunucu");
                host.input_routing = TrackInputRouting::Mono1;
                host.effects = vocal_chain();
                proj.tracks.push(host);

                let mut guest = Track::new("Konuk");
                guest.input_routing = TrackInputRouting::Mono2;
                guest.effects = vocal_chain();
                proj.tracks.push(guest);

                let mut music = Track::new("Jenerik / Müzik");
                music.gain_db = -12.0;
                proj.tracks.push(music);
            }
            ProjectTemplate::VocalAndBacking => {
                proj.name = "Vokal + Altyapı".to_string();
                let mut backing = Track::new("Altyapı (Beat)");
                backing.gain_db = -3.0;
                proj.tracks.push(backing);

                let mut vocal = Track::new("Ana Vokal");
                vocal.armed = true;
                vocal.input_routing = TrackInputRouting::Mono1;
                vocal.effects = vec![
                    EffectConfig::Compressor {
                        threshold_db: -16.0,
                        ratio: 3.5,
                        attack_ms: 8.0,
                        release_ms: 100.0,
                        makeup_gain_db: 4.0,
                    },
                    EffectConfig::Reverb {
                        room_size: 0.35,
                        damping: 0.5,
                        mix: 0.15,
                    },
                ];
                proj.tracks.push(vocal);
            }
            ProjectTemplate::AudioRestoration => {
                proj.name = "Ses Temizleme".to_string();
                let mut clean = Track::new("Temizlenecek Kayıt");
                clean.effects = vec![
                    EffectConfig::NoiseGate {
                        threshold_db: -45.0,
                        attack_ms: 3.0,
                        hold_ms: 60.0,
                        release_ms: 120.0,
                        range_db: -48.0,
                    },
                    EffectConfig::Limiter {
                        threshold_db: -1.0,
                        release_ms: 50.0,
                    },
                ];
                proj.tracks.push(clean);
            }
        }

        proj
    }

    /// Non-destructively splits all clips crossing `frame` (optionally scoped to `target_track_id`).
    /// Returns the number of clips split.
    pub fn split_all_at_frame(&mut self, frame: u64, target_track_id: Option<Uuid>) -> usize {
        let mut split_count = 0;
        let track_ids: Vec<Uuid> = self
            .tracks
            .iter()
            .filter(|t| target_track_id.is_none_or(|tid| tid == t.id))
            .map(|t| t.id)
            .collect();

        for tid in track_ids {
            let clip_ids: Vec<Uuid> = self
                .tracks
                .iter()
                .find(|t| t.id == tid)
                .map(|t| {
                    t.clips
                        .iter()
                        .filter(|c| frame > c.track_offset_frames && frame < c.timeline_end_frame())
                        .map(|c| c.id)
                        .collect()
                })
                .unwrap_or_default();

            for cid in clip_ids {
                if self.split_clip(tid, cid, frame).is_ok() {
                    split_count += 1;
                }
            }
        }
        split_count
    }

    /// Non-destructively removes `[start_frame, end_frame)` from clips without shifting later clips.
    /// Returns the number of affected clips.
    pub fn silence_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        target_track_id: Option<Uuid>,
    ) -> usize {
        if start_frame >= end_frame {
            return 0;
        }
        let mut affected = 0;

        for track in &mut self.tracks {
            if target_track_id.is_some_and(|tid| tid != track.id) {
                continue;
            }
            let mut new_clips = Vec::with_capacity(track.clips.len() + 2);
            for clip in &track.clips {
                let c_start = clip.track_offset_frames;
                let c_end = clip.timeline_end_frame();

                if c_end <= start_frame || c_start >= end_frame {
                    new_clips.push(clip.clone());
                    continue;
                }

                affected += 1;
                // Left remnant before start_frame
                if c_start < start_frame {
                    let mut left = clip.clone();
                    left.length_frames = start_frame - c_start;
                    left.fade_in_frames = left.fade_in_frames.min(left.length_frames);
                    left.fade_out_frames = 0;
                    new_clips.push(left);
                }

                // Right remnant after end_frame
                if c_end > end_frame {
                    let cut_offset = end_frame - c_start;
                    let mut right = clip.clone();
                    right.id = Uuid::new_v4();
                    right.track_offset_frames = end_frame;
                    right.source_start_frame = clip.source_start_frame + cut_offset;
                    right.length_frames = c_end - end_frame;
                    right.fade_in_frames = 0;
                    right.fade_out_frames = right.fade_out_frames.min(right.length_frames);
                    new_clips.push(right);
                }
            }
            new_clips.sort_by_key(|c| c.track_offset_frames);
            track.clips = new_clips;
        }
        affected
    }

    /// Non-destructively cuts `[start_frame, end_frame)` and shifts downstream clips left by `end - start`.
    /// Returns the number of affected clips.
    pub fn ripple_delete_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        target_track_id: Option<Uuid>,
    ) -> usize {
        if start_frame >= end_frame {
            return 0;
        }
        let delta = end_frame - start_frame;
        let mut affected = self.silence_range(start_frame, end_frame, target_track_id);

        for track in &mut self.tracks {
            if target_track_id.is_some_and(|tid| tid != track.id) {
                continue;
            }
            for clip in &mut track.clips {
                if clip.track_offset_frames >= end_frame {
                    clip.track_offset_frames -= delta;
                    affected += 1;
                }
            }
        }
        affected
    }

    /// Applies automatic equal-power crossfades when `moved_clip_id` overlaps adjacent clips on `track_id`.
    pub fn apply_auto_crossfades_on_track(&mut self, track_id: Uuid, moved_clip_id: Uuid) {
        let Some(track) = self.get_track_mut(track_id) else {
            return;
        };
        let Some(moved_idx) = track.clips.iter().position(|c| c.id == moved_clip_id) else {
            return;
        };

        let moved_start = track.clips[moved_idx].track_offset_frames;
        let moved_end = track.clips[moved_idx].timeline_end_frame();
        let moved_len = track.clips[moved_idx].length_frames;

        for idx in 0..track.clips.len() {
            if idx == moved_idx {
                continue;
            }
            let other_start = track.clips[idx].track_offset_frames;
            let other_end = track.clips[idx].timeline_end_frame();
            let other_len = track.clips[idx].length_frames;

            // Check overlap
            let overlap_start = moved_start.max(other_start);
            let overlap_end = moved_end.min(other_end);
            if overlap_start >= overlap_end {
                continue;
            }
            let overlap = overlap_end - overlap_start;
            // Avoid full engulfment turning entire clip into fade
            let max_fade = overlap.min(moved_len / 2).min(other_len / 2);
            if max_fade == 0 {
                continue;
            }

            if other_start <= moved_start {
                track.clips[idx].fade_out_frames = max_fade;
                track.clips[idx].fade_out_curve = FadeCurve::SCurve;
                track.clips[moved_idx].fade_in_frames = max_fade;
                track.clips[moved_idx].fade_in_curve = FadeCurve::SCurve;
                track.clips[moved_idx].crossfade_type = CrossfadeType::EqualPower;
            } else {
                track.clips[moved_idx].fade_out_frames = max_fade;
                track.clips[moved_idx].fade_out_curve = FadeCurve::SCurve;
                track.clips[idx].fade_in_frames = max_fade;
                track.clips[idx].fade_in_curve = FadeCurve::SCurve;
                track.clips[idx].crossfade_type = CrossfadeType::EqualPower;
            }
        }
    }
}

/// Snap-to-grid mode for timeline editing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SnapGridMode {
    #[default]
    Off,
    Seconds,
    HalfSecond,
    Beats,
    Bars,
}

/// Snaps a timeline frame to the nearest grid interval for the given mode.
pub fn snap_frame(
    frame: u64,
    sample_rate: u32,
    bpm: f64,
    beats_per_bar: u8,
    mode: SnapGridMode,
) -> u64 {
    if mode == SnapGridMode::Off || sample_rate == 0 {
        return frame;
    }
    let sr = sample_rate as f64;
    let step_frames = match mode {
        SnapGridMode::Off => return frame,
        SnapGridMode::Seconds => sr,
        SnapGridMode::HalfSecond => sr * 0.5,
        SnapGridMode::Beats => (60.0 / bpm.max(20.0)) * sr,
        SnapGridMode::Bars => (60.0 / bpm.max(20.0)) * (beats_per_bar.max(1) as f64) * sr,
    };
    if step_frames <= 1.0 {
        return frame;
    }
    ((frame as f64 / step_frames).round() * step_frames).max(0.0) as u64
}

/// Built-in starter project templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectTemplate {
    Empty,
    PodcastTwoHosts,
    VocalAndBacking,
    AudioRestoration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_from_template_and_validation() {
        for tpl in [
            ProjectTemplate::Empty,
            ProjectTemplate::PodcastTwoHosts,
            ProjectTemplate::VocalAndBacking,
            ProjectTemplate::AudioRestoration,
        ] {
            let proj = Project::from_template(tpl, 48_000);
            assert!(proj.validate().is_ok());
            assert!(!proj.tracks.is_empty());
        }
        let podcast = Project::from_template(ProjectTemplate::PodcastTwoHosts, 48_000);
        assert_eq!(podcast.tracks.len(), 3);
        assert!(podcast.tracks[0]
            .effects
            .iter()
            .any(|e| matches!(e, EffectConfig::DeEsser { .. })));
    }

    #[test]
    fn test_snap_frame_modes() {
        assert_eq!(
            snap_frame(12_345, 48_000, 120.0, 4, SnapGridMode::Off),
            12_345
        );
        assert_eq!(
            snap_frame(50_000, 48_000, 120.0, 4, SnapGridMode::Seconds),
            48_000
        );
        assert_eq!(
            snap_frame(30_000, 48_000, 120.0, 4, SnapGridMode::HalfSecond),
            24_000
        );
        // 120 BPM => 1 beat = 0.5s = 24_000 frames; 1 bar (4 beats) = 96_000 frames
        assert_eq!(
            snap_frame(25_000, 48_000, 120.0, 4, SnapGridMode::Beats),
            24_000
        );
        assert_eq!(
            snap_frame(90_000, 48_000, 120.0, 4, SnapGridMode::Bars),
            96_000
        );
    }

    #[test]
    fn test_silence_and_ripple_delete_range() {
        let mut proj = Project::new("Test", 48_000);
        let asset_id = uuid::Uuid::new_v4();
        let clip = Clip::new("C1", asset_id, 0, 10_000, 0);
        proj.tracks[0].clips.push(clip);

        // Silence middle [2000, 5000) -> leaves [0, 2000) and [5000, 10000)
        proj.silence_range(2_000, 5_000, None);
        assert_eq!(proj.tracks[0].clips.len(), 2);
        assert_eq!(proj.tracks[0].clips[0].track_offset_frames, 0);
        assert_eq!(proj.tracks[0].clips[0].length_frames, 2_000);
        assert_eq!(proj.tracks[0].clips[1].track_offset_frames, 5_000);
        assert_eq!(proj.tracks[0].clips[1].length_frames, 5_000);

        // Ripple delete [2000, 5000) -> shifts second clip left by 3000 frames to 2000
        proj.ripple_delete_range(2_000, 5_000, None);
        assert_eq!(proj.tracks[0].clips.len(), 2);
        assert_eq!(proj.tracks[0].clips[1].track_offset_frames, 2_000);
    }

    #[test]
    fn test_auto_crossfades_on_track() {
        let mut proj = Project::new("Xfade", 48_000);
        let track_id = proj.tracks[0].id;
        let asset_id = uuid::Uuid::new_v4();
        let c1 = Clip::new("A", asset_id, 0, 10_000, 0);
        let c2 = Clip::new("B", asset_id, 0, 10_000, 8_000);
        let c2_id = c2.id;
        proj.tracks[0].clips.push(c1);
        proj.tracks[0].clips.push(c2);
        proj.apply_auto_crossfades_on_track(track_id, c2_id);
        assert_eq!(proj.tracks[0].clips[0].fade_out_frames, 2_000);
        assert_eq!(proj.tracks[0].clips[0].fade_out_curve, FadeCurve::SCurve);
        assert_eq!(proj.tracks[0].clips[1].fade_in_frames, 2_000);
        assert_eq!(proj.tracks[0].clips[1].fade_in_curve, FadeCurve::SCurve);
        assert_eq!(
            proj.tracks[0].clips[1].crossfade_type,
            CrossfadeType::EqualPower
        );
    }
}
