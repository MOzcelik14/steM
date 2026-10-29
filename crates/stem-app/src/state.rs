//! Timeline view state, selection, and interactive drag operations

use stem_audio::DecodedAudio;
use stem_core::peaks::PeakPyramid;
use stem_core::waveform::{WaveformDocument, WaveformHistory};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppViewMode {
    Multitrack,
    Waveform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveformDragOp {
    Selecting { anchor_frame: u64 },
    DragLeftEdge,
    DragRightEdge,
    ScrubPlayhead,
}

#[derive(Debug, Clone)]
pub struct WaveformViewState {
    pub pixels_per_second: f32,
    pub scroll_x: f32,
    pub selection: Option<(u64, u64)>,
    pub snap_to_zero_crossing: bool,
    pub loop_selection: bool,
    pub drag_op: Option<WaveformDragOp>,
    pub document: Option<WaveformDocument>,
    pub history: WaveformHistory,
    pub active_source_id: Option<Uuid>,
    pub active_clip_info: Option<(Uuid, Uuid)>, // (clip_id, track_id)
    pub file_name: String,
    pub playhead_frame: u64,
    pub pyramid: Option<PeakPyramid>,
    pub cached_planar: Option<Vec<Vec<f32>>>,
    pub show_normalize_dialog: bool,
    pub normalize_target_db: f32,
}

impl Default for WaveformViewState {
    fn default() -> Self {
        Self {
            pixels_per_second: 120.0,
            scroll_x: 0.0,
            selection: None,
            snap_to_zero_crossing: false,
            loop_selection: false,
            drag_op: None,
            document: None,
            history: WaveformHistory::new(),
            active_source_id: None,
            active_clip_info: None,
            file_name: String::new(),
            playhead_frame: 0,
            pyramid: None,
            cached_planar: None,
            show_normalize_dialog: false,
            normalize_target_db: -0.1,
        }
    }
}

#[allow(dead_code)]
impl WaveformViewState {
    #[inline]
    pub fn frame_to_x(&self, frame: u64, sample_rate: u32) -> f32 {
        let sec = frame as f32 / sample_rate as f32;
        sec * self.pixels_per_second - self.scroll_x
    }

    #[inline]
    pub fn x_to_frame(&self, x: f32, sample_rate: u32) -> u64 {
        let total_x = (x + self.scroll_x).max(0.0);
        let sec = total_x / self.pixels_per_second;
        (sec * sample_rate as f32).round() as u64
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragOp {
    MoveClip {
        clip_id: Uuid,
        track_id: Uuid,
        original_track_offset: u64,
        drag_start_x: f32,
    },
    TrimLeft {
        clip_id: Uuid,
        track_id: Uuid,
        orig_source_start: u64,
        orig_track_offset: u64,
        orig_length: u64,
        drag_start_x: f32,
    },
    TrimRight {
        clip_id: Uuid,
        track_id: Uuid,
        orig_length: u64,
        drag_start_x: f32,
    },
    FadeIn {
        clip_id: Uuid,
        track_id: Uuid,
        orig_fade_in: u64,
        drag_start_x: f32,
    },
    FadeOut {
        clip_id: Uuid,
        track_id: Uuid,
        orig_fade_out: u64,
        drag_start_x: f32,
    },
    ScrubPlayhead,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TimelineViewState {
    /// Pixels per second along the time axis.
    pub pixels_per_second: f32,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Height in pixels for a single audio track.
    pub track_height: f32,
    /// Active drag operation.
    pub drag_op: Option<DragOp>,
    /// Selected track ID.
    pub selected_track_id: Option<Uuid>,
    /// Selected clip ID.
    pub selected_clip_id: Option<Uuid>,
}

impl Default for TimelineViewState {
    fn default() -> Self {
        Self {
            pixels_per_second: 60.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            track_height: 110.0,
            drag_op: None,
            selected_track_id: None,
            selected_clip_id: None,
        }
    }
}

#[allow(dead_code)]
impl TimelineViewState {
    #[inline]
    pub fn frame_to_x(&self, frame: u64, sample_rate: u32) -> f32 {
        let sec = frame as f32 / sample_rate as f32;
        sec * self.pixels_per_second - self.scroll_x
    }

    #[inline]
    pub fn x_to_frame(&self, x: f32, sample_rate: u32) -> u64 {
        let total_x = (x + self.scroll_x).max(0.0);
        let sec = total_x / self.pixels_per_second;
        (sec * sample_rate as f32).round() as u64
    }
}

/// Messages sent from background worker threads to the UI.
pub enum WorkerMessage {
    ImportComplete {
        decoded: Box<DecodedAudio>,
        pyramid: Box<PeakPyramid>,
    },
    ImportFailed(String),
    ExportProgress(f32),
    ExportFinished(Result<String, String>),
}
