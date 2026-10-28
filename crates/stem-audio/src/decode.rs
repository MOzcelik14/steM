//! Audio decoding using Symphonia
//!
//! Supports WAV, FLAC, MP3, OGG/Vorbis, and AAC/M4A. Decodes into 32-bit floating point PCM.
//! Runs completely in background threads off the UI and real-time audio threads.

use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;
use uuid::Uuid;

use stem_core::project::AudioSource;

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("I/O error reading audio file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Symphonia media error: {0}")]
    Symphonia(#[from] SymphoniaError),

    #[error("No supported audio track found in media container")]
    NoTrackFound,

    #[error("Unsupported codec")]
    UnsupportedCodec,
}

/// Decoded audio data stored as planar 32-bit float channels.
pub struct DecodedAudio {
    pub source: AudioSource,
    /// Planar channel buffers: channels[0] = Left, channels[1] = Right.
    pub channels: Vec<Vec<f32>>,
}

/// Decodes an audio file at `path` into normalized 32-bit float buffers.
pub fn decode_file<P: AsRef<Path>>(path: P) -> Result<DecodedAudio, DecodeError> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    let mut format = probed.format;

    // Find the first supported audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoTrackFound)?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let num_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    let mut planar_channels: Vec<Vec<f32>> = vec![Vec::new(); num_channels];
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(DecodeError::Symphonia(err)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;

                let sbuf = sample_buf.get_or_insert_with(|| SampleBuffer::new(duration, spec));

                sbuf.copy_interleaved_ref(decoded);
                let samples = sbuf.samples();

                // De-interleave into planar channels
                for chunk in samples.chunks(num_channels) {
                    for (ch_idx, &sample) in chunk.iter().enumerate() {
                        if ch_idx < planar_channels.len() {
                            planar_channels[ch_idx].push(sample);
                        }
                    }
                }
            }
            Err(SymphoniaError::DecodeError(_)) => {
                // Recoverable decode error, skip frame
                continue;
            }
            Err(err) => return Err(DecodeError::Symphonia(err)),
        }
    }

    let total_frames = planar_channels.first().map(|ch| ch.len()).unwrap_or(0) as u64;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio")
        .to_string();

    let source = AudioSource {
        id: Uuid::new_v4(),
        path: PathBuf::from(path),
        filename,
        sample_rate,
        channels: num_channels,
        duration_frames: total_frames,
        file_hash: None,
    };

    Ok(DecodedAudio {
        source,
        channels: planar_channels,
    })
}
