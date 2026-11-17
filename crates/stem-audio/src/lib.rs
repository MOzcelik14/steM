//! Audio I/O, device management, real-time playback engine, decoder, and export for steM.

pub mod callback;
pub mod decode;
pub mod engine;
pub mod export;
pub mod plugins;
pub mod record;
pub mod stems;

pub use callback::{AudioCommand, AudioProcessor, AudioTelemetry, SharedAudioTimeline};
pub use decode::{decode_file, DecodeError, DecodedAudio};
pub use engine::{AudioEngine, AudioEngineConfig, AudioEngineError};
pub use export::{
    export_buffer_to_file, export_project_to_file, export_surround_51_wav, format_export_filename,
    freeze_track_to_wav, render_project_mixdown, render_project_mixdown_range,
    render_project_tracks, render_track_stem, BitDepth, ExportError, ExportFormat, ExportRange,
    ExportSettings,
};
pub use plugins::{
    PluginDescriptor, PluginHostInstance, PluginLoadError, PluginScanner, PluginStatus,
};
pub use record::{AudioRecorder, InputTelemetry, RecordError};
pub use stems::{
    DemucsModel, DemucsRunner, SeparationQuality, StemKind, StemSeparationError,
    StemSeparationRequest,
};
