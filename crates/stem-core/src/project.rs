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
            FadeCurve::Exponential => t * t,
            FadeCurve::SCurve => 0.5 * (1.0 - (std::f32::consts::PI * t).cos()),
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

/// A single audio track containing clips and effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: Uuid,
    pub name: String,
    pub gain_db: f32,
    pub pan: f32, // -1.0 (Left) to +1.0 (Right), 0.0 Center
    pub muted: bool,
    pub soloed: bool,
    pub armed: bool,
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
    pub master_gain_db: f32,
    pub loop_enabled: bool,
    pub loop_region: Option<(u64, u64)>,
    pub playhead_frame: u64,
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
            master_gain_db: 0.0,
            loop_enabled: false,
            loop_region: None,
            playhead_frame: 0,
        };
        // Provide 2 default audio tracks
        proj.add_track("Track 1");
        proj.add_track("Track 2");
        proj
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
