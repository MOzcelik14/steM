//! Audio I/O, device management, real-time playback engine, decoder, and export for steM.

pub mod callback;
pub mod decode;
pub mod engine;
pub mod export;
pub mod record;

pub use callback::{AudioCommand, AudioProcessor, AudioTelemetry, SharedAudioTimeline};
pub use decode::{decode_file, DecodeError, DecodedAudio};
pub use engine::{AudioEngine, AudioEngineError};
pub use export::{
    export_project_to_file, render_project_mixdown, render_project_mixdown_range, BitDepth,
    ExportError, ExportFormat, ExportRange, ExportSettings,
};
pub use record::{AudioRecorder, RecordError};
