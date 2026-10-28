use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("Audio source not found with ID: {0}")]
    SourceNotFound(uuid::Uuid),

    #[error("Track not found with ID: {0}")]
    TrackNotFound(uuid::Uuid),

    #[error("Clip not found with ID: {0}")]
    ClipNotFound(uuid::Uuid),

    #[error("RON serialization error: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),

    #[error("RON error: {0}")]
    RonError(#[from] ron::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "Invalid clip bounds: start {start} + length {length} exceeds source length {source_len}"
    )]
    InvalidClipBounds {
        start: u64,
        length: u64,
        source_len: u64,
    },
}
