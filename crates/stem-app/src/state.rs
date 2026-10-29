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
        }
    }
}

// =========================================================================
// Phase F: Markers, Batch Export, Auto-Save, and Keybindings State
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AppAction {
    PlayPause,
    Stop,
    Record,
    SplitClip,
    DropMarker,
    NamedMarkerDialog,
    PrevMarker,
    NextMarker,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    ZoomIn,
    ZoomOut,
    ToggleSpectrogram,
}

impl AppAction {
    pub fn all() -> &'static [AppAction] {
        &[
            AppAction::PlayPause,
            AppAction::Stop,
            AppAction::Record,
            AppAction::SplitClip,
            AppAction::DropMarker,
            AppAction::NamedMarkerDialog,
            AppAction::PrevMarker,
            AppAction::NextMarker,
            AppAction::Undo,
            AppAction::Redo,
            AppAction::Cut,
            AppAction::Copy,
            AppAction::Paste,
            AppAction::Delete,
            AppAction::ZoomIn,
            AppAction::ZoomOut,
            AppAction::ToggleSpectrogram,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PlayPause => "Play / Pause",
            Self::Stop => "Stop",
            Self::Record => "Record",
            Self::SplitClip => "Split Clip",
            Self::DropMarker => "Drop Marker",
            Self::NamedMarkerDialog => "Create Named Marker",
            Self::PrevMarker => "Jump to Previous Marker",
            Self::NextMarker => "Jump to Next Marker",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Delete => "Delete",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ToggleSpectrogram => "Toggle Spectrogram",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Shortcut {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Shortcut {
    pub fn new(key: impl Into<String>, ctrl: bool, shift: bool, alt: bool) -> Self {
        Self {
            key: key.into(),
            ctrl,
            shift,
            alt,
        }
    }

    pub fn display_string(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(self.key.as_str());
        parts.join("+")
    }

    pub fn matches(&self, input: &egui::InputState) -> bool {
        let ctrl_pressed = input.modifiers.command || input.modifiers.ctrl;
        if self.ctrl != ctrl_pressed {
            return false;
        }
        if self.shift != input.modifiers.shift {
            return false;
        }
        if self.alt != input.modifiers.alt {
            return false;
        }

        let key = match self.key.as_str() {
            "Space" => egui::Key::Space,
            "Escape" => egui::Key::Escape,
            "Delete" => egui::Key::Delete,
            "Backspace" => egui::Key::Backspace,
            "Home" => egui::Key::Home,
            "[" => egui::Key::OpenBracket,
            "]" => egui::Key::CloseBracket,
            "Equals" => egui::Key::Equals,
            "Minus" => egui::Key::Minus,
            "F12" => egui::Key::F12,
            "A" => egui::Key::A,
            "B" => egui::Key::B,
            "C" => egui::Key::C,
            "D" => egui::Key::D,
            "E" => egui::Key::E,
            "I" => egui::Key::I,
            "M" => egui::Key::M,
            "R" => egui::Key::R,
            "S" => egui::Key::S,
            "T" => egui::Key::T,
            "V" => egui::Key::V,
            "X" => egui::Key::X,
            "Y" => egui::Key::Y,
            "Z" => egui::Key::Z,
            "1" => egui::Key::Num1,
            "2" => egui::Key::Num2,
            _ => return false,
        };

        input.key_pressed(key)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyBindingMap {
    pub bindings: std::collections::HashMap<AppAction, Shortcut>,
}

impl Default for KeyBindingMap {
    fn default() -> Self {
        let mut bindings = std::collections::HashMap::new();
        bindings.insert(
            AppAction::PlayPause,
            Shortcut::new("Space", false, false, false),
        );
        bindings.insert(
            AppAction::Stop,
            Shortcut::new("Escape", false, false, false),
        );
        bindings.insert(AppAction::Record, Shortcut::new("R", false, false, false));
        bindings.insert(
            AppAction::SplitClip,
            Shortcut::new("S", false, false, false),
        );
        bindings.insert(
            AppAction::DropMarker,
            Shortcut::new("M", false, false, false),
        );
        bindings.insert(
            AppAction::NamedMarkerDialog,
            Shortcut::new("M", false, true, false),
        );
        bindings.insert(
            AppAction::PrevMarker,
            Shortcut::new("[", false, false, false),
        );
        bindings.insert(
            AppAction::NextMarker,
            Shortcut::new("]", false, false, false),
        );
        bindings.insert(AppAction::Undo, Shortcut::new("Z", true, false, false));
        bindings.insert(AppAction::Redo, Shortcut::new("Y", true, false, false));
        bindings.insert(AppAction::Cut, Shortcut::new("X", true, false, false));
        bindings.insert(AppAction::Copy, Shortcut::new("C", true, false, false));
        bindings.insert(AppAction::Paste, Shortcut::new("V", true, false, false));
        bindings.insert(
            AppAction::Delete,
            Shortcut::new("Delete", false, false, false),
        );
        bindings.insert(
            AppAction::ZoomIn,
            Shortcut::new("Equals", true, false, false),
        );
        bindings.insert(
            AppAction::ZoomOut,
            Shortcut::new("Minus", true, false, false),
        );
        bindings.insert(
            AppAction::ToggleSpectrogram,
            Shortcut::new("F12", false, false, false),
        );

        Self { bindings }
    }
}

impl KeyBindingMap {
    pub fn is_action_triggered(&self, action: AppAction, input: &egui::InputState) -> bool {
        self.bindings
            .get(&action)
            .map(|sc| sc.matches(input))
            .unwrap_or(false)
    }

    pub fn detect_conflicts(&self) -> Vec<(AppAction, AppAction, Shortcut)> {
        let mut conflicts = Vec::new();
        let items: Vec<(&AppAction, &Shortcut)> = self.bindings.iter().collect();
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                if items[i].1 == items[j].1 {
                    conflicts.push((*items[i].0, *items[j].0, items[i].1.clone()));
                }
            }
        }
        conflicts
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn reset_to_defaults(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone)]
pub struct NamedMarkerDialogState {
    pub show: bool,
    pub marker_name: String,
    pub target_frame: u64,
    pub is_cd_track: bool,
    pub edit_marker_id: Option<uuid::Uuid>,
}

impl Default for NamedMarkerDialogState {
    fn default() -> Self {
        Self {
            show: false,
            marker_name: "Marker".to_string(),
            target_frame: 0,
            is_cd_track: false,
            edit_marker_id: None,
        }
    }
}

impl NamedMarkerDialogState {
    pub fn open_for_new(&mut self, frame: u64, default_name: String) {
        self.show = true;
        self.target_frame = frame;
        self.marker_name = default_name;
        self.is_cd_track = false;
        self.edit_marker_id = None;
    }

    pub fn open_for_edit(&mut self, marker: &stem_core::Marker) {
        self.show = true;
        self.target_frame = marker.frame;
        self.marker_name = marker.name.clone();
        self.is_cd_track = marker.is_cd_track;
        self.edit_marker_id = Some(marker.id);
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarkerListDialogState {
    pub show: bool,
}

#[derive(Debug, Clone)]
pub struct BatchExportDialogState {
    pub show: bool,
    pub export_stems: bool,
    pub export_marker_regions: bool,
    pub filename_pattern: String,
    pub output_dir: std::path::PathBuf,
    pub format: stem_audio::ExportFormat,
    pub bit_depth: stem_audio::BitDepth,
    pub in_progress: bool,
    pub status_msg: Option<String>,
}

impl Default for BatchExportDialogState {
    fn default() -> Self {
        Self {
            show: false,
            export_stems: true,
            export_marker_regions: false,
            filename_pattern: "{project}_{track}_{index}".to_string(),
            output_dir: std::env::current_dir().unwrap_or_default(),
            format: stem_audio::ExportFormat::Wav,
            bit_depth: stem_audio::BitDepth::Bit24,
            in_progress: false,
            status_msg: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeybindingsDialogState {
    pub show: bool,
    pub filter: String,
}

#[derive(Debug, Clone, Default)]
pub struct RecoveryDialogState {
    pub show: bool,
    pub recovery_path: Option<std::path::PathBuf>,
    pub recovery_time_str: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybindings_defaults_and_conflicts() {
        let mut map = KeyBindingMap::default();
        let conflicts = map.detect_conflicts();
        assert!(
            conflicts.is_empty(),
            "Default keybindings must have no conflicts"
        );

        // Force a conflict
        map.bindings.insert(
            AppAction::PlayPause,
            Shortcut::new("Space", false, false, false),
        );
        map.bindings
            .insert(AppAction::Stop, Shortcut::new("Space", false, false, false));

        let detected = map.detect_conflicts();
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].2.key, "Space");
    }

    #[test]
    fn test_keybindings_json_roundtrip() {
        let map = KeyBindingMap::default();
        let json = map.to_json().expect("JSON serialization");
        let restored = KeyBindingMap::from_json(&json).expect("JSON deserialization");

        assert_eq!(restored.bindings.len(), map.bindings.len());
        assert_eq!(
            restored.bindings.get(&AppAction::PlayPause).unwrap().key,
            "Space"
        );
    }
}
