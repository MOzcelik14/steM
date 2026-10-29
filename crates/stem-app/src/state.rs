//! Timeline view state, selection, and interactive drag operations

use stem_audio::DecodedAudio;
use stem_core::peaks::PeakPyramid;
use stem_core::waveform::{WaveformDocument, WaveformHistory};
use stem_dsp::{LoudnessResult, NoiseProfile};
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
    /// Track whose automation lane is expanded.
    pub expanded_automation_track_id: Option<Uuid>,
    /// Active target parameter for expanded automation lane.
    pub active_automation_target: stem_core::project::AutomationTarget,
    /// Currently dragged automation point (track_id, point_idx).
    pub drag_automation_point: Option<(Uuid, usize)>,
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
            expanded_automation_track_id: None,
            active_automation_target: stem_core::project::AutomationTarget::Volume,
            drag_automation_point: None,
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

#[derive(Debug, Clone)]
pub struct NoiseReductionDialogState {
    pub show: bool,
    pub reduction_db: f32,
    pub sensitivity: f32,
    pub noise_profile: Option<NoiseProfile>,
    pub status_message: Option<String>,
}

impl Default for NoiseReductionDialogState {
    fn default() -> Self {
        Self {
            show: false,
            reduction_db: 18.0,
            sensitivity: 1.2,
            noise_profile: None,
            status_message: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClickRemoverDialogState {
    pub show: bool,
    pub sensitivity: f32,
    pub max_width: usize,
    pub status_message: Option<String>,
}

impl Default for ClickRemoverDialogState {
    fn default() -> Self {
        Self {
            show: false,
            sensitivity: 3.5,
            max_width: 32,
            status_message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoudnessStandard {
    #[default]
    EbuR128, // Target: -23.0 LUFS
    Streaming,  // Target: -14.0 LUFS (Spotify / YouTube)
    AppleMusic, // Target: -16.0 LUFS
    AesTd1004,  // Target: -24.0 LUFS
}

impl LoudnessStandard {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EbuR128 => "EBU R128 (-23 LUFS)",
            Self::Streaming => "Streaming / Spotify (-14 LUFS)",
            Self::AppleMusic => "Apple Music (-16 LUFS)",
            Self::AesTd1004 => "AES TD1004 (-24 LUFS)",
        }
    }

    pub fn target_lufs(&self) -> f32 {
        match self {
            Self::EbuR128 => -23.0,
            Self::Streaming => -14.0,
            Self::AppleMusic => -16.0,
            Self::AesTd1004 => -24.0,
        }
    }

    pub fn max_true_peak_dbfs(&self) -> f32 {
        -1.0
    }
}

#[derive(Debug, Clone)]
pub struct LoudnessReportDialogState {
    pub show: bool,
    pub standard: LoudnessStandard,
    pub result: Option<LoudnessResult>,
    pub true_peak_dbfs: f32,
}

impl Default for LoudnessReportDialogState {
    fn default() -> Self {
        Self {
            show: false,
            standard: LoudnessStandard::default(),
            result: None,
            true_peak_dbfs: -90.0,
        }
    }
}

/// Dynamic recording session and hardware state.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RecordingState {
    pub input_telemetry: stem_audio::InputTelemetry,
    pub count_in_active: bool,
    pub count_in_seconds_remaining: f32,
    pub count_in_total_seconds: f32,
    pub record_start_frame: u64,
    pub is_punching: bool,
    pub available_input_devices: Vec<String>,
    pub selected_input_device: Option<String>,
    pub selected_sample_rate: u32,
    pub device_lost_warning: bool,
    pub show_settings_dialog: bool,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            input_telemetry: stem_audio::InputTelemetry::default(),
            count_in_active: false,
            count_in_seconds_remaining: 0.0,
            count_in_total_seconds: 0.0,
            record_start_frame: 0,
            is_punching: false,
            available_input_devices: Vec::new(),
            selected_input_device: None,
            selected_sample_rate: 48000,
            device_lost_warning: false,
            show_settings_dialog: false,
        }
    }
}
