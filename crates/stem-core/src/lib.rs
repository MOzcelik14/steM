//! Core data structures, project model, undo/redo history, and peak caches for steM.

pub mod demo;
pub mod error;
pub mod fs_util;
pub mod history;
pub mod peaks;
pub mod project;
pub mod waveform;

pub use demo::generate_demo_project;
pub use error::ProjectError;
pub use fs_util::{atomic_rename, atomic_write_bytes};
pub use history::{Action, UndoRedoStack};
pub use peaks::{PeakBucket, PeakLevel, PeakPyramid};
pub use project::{
    AudioSource, Clip, EffectConfig, EqBandConfig, FadeCurve, FilterType, Marker, PluginFormat,
    PluginParamValue, PluginSlotConfig, Project, Track, MAX_PROJECT_TRACKS,
};
pub use waveform::{
    find_nearest_zero_crossing, AudioClipboard, AudioSegment, WaveformDocument, WaveformHistory,
};

/// The user-facing display name constant re-exported from stem-i18n.
pub const APP_DISPLAY_NAME: &str = stem_i18n::APP_DISPLAY_NAME;
