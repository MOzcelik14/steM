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

/// Output destination routing for a track.
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

/// Target parameter controlled by an automation lane.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutomationTarget {
    Volume, // decibels (-60.0 to +12.0)
    Pan,    // -1.0 to +1.0
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

    /// Piecewise evaluation at timeline frame.
    pub fn evaluate(&self, frame: u64, default_val: f32) -> f32 {
        if !self.enabled || self.points.is_empty() {
            return default_val;
        }

        if frame <= self.points.first().unwrap().frame {
            return self.points.first().unwrap().value;
        }
        if frame >= self.points.last().unwrap().frame {
            return self.points.last().unwrap().value;
        }

        for i in 0..(self.points.len() - 1) {
            let p1 = &self.points[i];
            let p2 = &self.points[i + 1];
            if frame >= p1.frame && frame <= p2.frame {
                let range = (p2.frame - p1.frame).max(1) as f32;
                let t = (frame - p1.frame) as f32 / range;
                return p1.curve.interpolate(t, p1.value, p2.value);
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
}

/// The complete non-destructive project representation in steM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
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
    pub master_gain_db: f32,
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
            id: Uuid::new_v4(),
            name: name.into(),
            sample_rate,
            tempo_bpm: 120.0,
            time_signature: (4, 4),
            sources: HashMap::new(),
            tracks: Vec::new(),
            busses: Vec::new(),
            groups: Vec::new(),
            master_gain_db: 0.0,
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

    /// Serializes project state to RON.
    pub fn to_ron(&self) -> Result<String, ProjectError> {
        let pretty_config = ron::ser::PrettyConfig::new()
            .depth_limit(6)
            .separate_tuple_members(true)
            .enumerate_arrays(true);
        Ok(ron::ser::to_string_pretty(self, pretty_config)?)
    }

    /// Deserializes project from RON string.
    pub fn from_ron(s: &str) -> Result<Self, ProjectError> {
        Ok(ron::from_str(s)?)
    }
}
