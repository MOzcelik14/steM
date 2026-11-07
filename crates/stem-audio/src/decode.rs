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

/// Maximum allowed channels for decoded audio (stereo, 5.1, 7.1).
pub const MAX_DECODE_CHANNELS: usize = 8;
/// Minimum supported audio sample rate (8 kHz).
pub const MIN_DECODE_SAMPLE_RATE: u32 = 8_000;
/// Maximum supported audio sample rate (192 kHz).
pub const MAX_DECODE_SAMPLE_RATE: u32 = 192_000;
/// Maximum allowable decoded audio frames (4 hours at 192 kHz) to prevent OOM exhaustion.
pub const MAX_DECODE_FRAMES: usize = 192_000 * 3600 * 4;

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

    #[error("Invalid channel count: {0} (supported: 1..={MAX_DECODE_CHANNELS})")]
    InvalidChannelCount(usize),

    #[error("Invalid sample rate: {0} Hz (supported: {MIN_DECODE_SAMPLE_RATE}..={MAX_DECODE_SAMPLE_RATE})")]
    InvalidSampleRate(u32),

    #[error("Exceeded maximum audio frame limit ({MAX_DECODE_FRAMES} frames)")]
    FrameLimitExceeded,

    #[error("Corrupted container header: {0}")]
    CorruptedHeader(String),
}

/// Decoded audio data stored as planar 32-bit float channels.
pub struct DecodedAudio {
    pub source: AudioSource,
    /// Planar channel buffers: channels[0] = Left, channels[1] = Right.
    pub channels: Vec<Vec<f32>>,
}

/// Decodes an audio file at `path` into normalized 32-bit float buffers.
pub fn decode_file<P: AsRef<Path>>(path: P) -> Result<DecodedAudio, DecodeError> {
    use std::io::{Read, Seek, SeekFrom};

    let path = path.as_ref();
    let mut file = File::open(path)?;

    let file_len = file.metadata()?.len();
    if file_len < 4 {
        return Err(DecodeError::CorruptedHeader(
            "Audio file is too small".to_string(),
        ));
    }

    // Read up to first 64KB to validate headers before Symphonia parses untrusted metadata
    let check_len = (file_len as usize).min(65536);
    let mut header_buf = vec![0u8; check_len];
    file.read_exact(&mut header_buf)?;
    validate_audio_bytes(&header_buf)?;
    file.seek(SeekFrom::Start(0))?;
    let safe_source = SafeSource {
        inner: file,
        seekable: true,
    };
    let mss = MediaSourceStream::new(Box::new(safe_source), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio")
        .to_string();

    decode_stream(mss, hint, PathBuf::from(path), filename)
}

/// Decodes an in-memory byte slice (useful for fuzzing and unit testing).
pub fn decode_bytes(
    bytes: &[u8],
    extension_hint: Option<&str>,
) -> Result<DecodedAudio, DecodeError> {
    validate_audio_bytes(bytes)?;

    let cursor = std::io::Cursor::new(bytes.to_vec());
    let safe_source = SafeSource {
        inner: cursor,
        seekable: true,
    };
    let mss = MediaSourceStream::new(Box::new(safe_source), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = extension_hint {
        hint.with_extension(ext);
    }

    decode_stream(mss, hint, PathBuf::from("memory"), "memory".to_string())
}

/// Wraps an underlying media stream to provide safe `MediaSource` operations.
/// Suppresses `byte_len` to disarm upstream heuristics that divide by zero or underflow on corrupt streams.
struct SafeSource<R> {
    inner: R,
    seekable: bool,
}

impl<R: std::io::Read> std::io::Read for SafeSource<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: std::io::Seek> std::io::Seek for SafeSource<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl<R: std::io::Read + std::io::Seek + Send + Sync + 'static> symphonia::core::io::MediaSource
    for SafeSource<R>
{
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// Pre-validates audio byte containers before delegating to third-party decoders.
/// Protects against known memory-exhaustion and OOM attacks in container metadata parsers.
pub fn validate_audio_bytes(bytes: &[u8]) -> Result<(), DecodeError> {
    if let Some(pos) = bytes.windows(4).position(|w| w == b"fLaC") {
        validate_flac_bytes(&bytes[pos..])?;
    }
    Ok(())
}

fn validate_flac_bytes(bytes: &[u8]) -> Result<(), DecodeError> {
    let total_len = bytes.len();
    // FLAC marker (4) + STREAMINFO header (4) + STREAMINFO block (34) = 42 bytes minimum
    if total_len < 42 {
        return Err(DecodeError::CorruptedHeader(
            "FLAC truncated before STREAMINFO block".to_string(),
        ));
    }

    // By RFC 9636 FLAC spec, the first metadata block MUST be STREAMINFO (type 0)
    let first_block_type = bytes[4] & 0x7F;
    if first_block_type != 0 {
        return Err(DecodeError::CorruptedHeader(format!(
            "First FLAC metadata block must be STREAMINFO (type 0), found type {first_block_type}"
        )));
    }

    let mut offset = 4;
    while offset < total_len {
        if offset + 4 > total_len {
            return Err(DecodeError::CorruptedHeader(
                "Truncated FLAC metadata block header".to_string(),
            ));
        }
        let is_last = (bytes[offset] & 0x80) != 0;
        let block_type = bytes[offset] & 0x7F;
        let block_len =
            u32::from_be_bytes([0, bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
                as usize;

        // Block length must not claim more bytes than available in the input stream
        if offset + 4 + block_len > total_len {
            return Err(DecodeError::CorruptedHeader(format!(
                "FLAC metadata block type {block_type} claims {block_len} bytes, exceeding stream length {total_len}"
            )));
        }

        // Validate STREAMINFO block length
        if block_type == 0 && block_len != 34 {
            return Err(DecodeError::CorruptedHeader(format!(
                "FLAC STREAMINFO block must be exactly 34 bytes, found {block_len}"
            )));
        }

        // Validate PICTURE block fields to prevent unchecked allocations in upstream demuxer
        if block_type == 6 {
            // Need at least 8 bytes: picture type (4) + media type length (4)
            if block_len < 8 {
                return Err(DecodeError::CorruptedHeader(
                    "Truncated FLAC PICTURE metadata block".to_string(),
                ));
            }
            let media_type_len = u32::from_be_bytes([
                bytes[offset + 8],
                bytes[offset + 9],
                bytes[offset + 10],
                bytes[offset + 11],
            ]) as usize;

            if media_type_len > block_len.saturating_sub(8) {
                return Err(DecodeError::CorruptedHeader(format!(
                    "FLAC PICTURE media_type_len {media_type_len} exceeds block length {block_len}"
                )));
            }

            let desc_len_offset = offset + 12 + media_type_len;
            if desc_len_offset + 4 > offset + 4 + block_len {
                return Err(DecodeError::CorruptedHeader(
                    "Truncated FLAC PICTURE description length".to_string(),
                ));
            }
            let desc_len = u32::from_be_bytes([
                bytes[desc_len_offset],
                bytes[desc_len_offset + 1],
                bytes[desc_len_offset + 2],
                bytes[desc_len_offset + 3],
            ]) as usize;

            if desc_len > (offset + 4 + block_len).saturating_sub(desc_len_offset + 4) {
                return Err(DecodeError::CorruptedHeader(format!(
                    "FLAC PICTURE desc_len {desc_len} exceeds block length {block_len}"
                )));
            }

            // width(4) + height(4) + bpp(4) + indexed_colors(4) + data_len(4) = 20 bytes
            let img_data_len_offset = desc_len_offset + 4 + desc_len + 16;
            if img_data_len_offset + 4 > offset + 4 + block_len {
                return Err(DecodeError::CorruptedHeader(
                    "Truncated FLAC PICTURE image dimensions".to_string(),
                ));
            }
            let img_data_len = u32::from_be_bytes([
                bytes[img_data_len_offset],
                bytes[img_data_len_offset + 1],
                bytes[img_data_len_offset + 2],
                bytes[img_data_len_offset + 3],
            ]) as usize;

            if img_data_len > (offset + 4 + block_len).saturating_sub(img_data_len_offset + 4) {
                return Err(DecodeError::CorruptedHeader(format!(
                    "FLAC PICTURE img_data_len {img_data_len} exceeds block length {block_len}"
                )));
            }
        }

        offset += 4 + block_len;
        if is_last {
            break;
        }
    }

    Ok(())
}

fn catch_unwind_silent<F, R>(f: F) -> std::thread::Result<R>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    use std::sync::Mutex;
    static HOOK_LOCK: Mutex<()> = Mutex::new(());
    let _guard = HOOK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(prev_hook);
    result
}

fn decode_stream(
    mss: MediaSourceStream,
    hint: Hint,
    path_buf: PathBuf,
    filename: String,
) -> Result<DecodedAudio, DecodeError> {
    let result = catch_unwind_silent(std::panic::AssertUnwindSafe(|| {
        decode_stream_inner(mss, hint, path_buf, filename)
    }));

    match result {
        Ok(res) => res,
        Err(_) => Err(DecodeError::CorruptedHeader(
            "Third-party media container or decoder panicked on corrupt stream".to_string(),
        )),
    }
}

fn decode_stream_inner(
    mss: MediaSourceStream,
    hint: Hint,
    path_buf: PathBuf,
    filename: String,
) -> Result<DecodedAudio, DecodeError> {
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(DecodeError::Symphonia)?;

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

    if !(1..=MAX_DECODE_CHANNELS).contains(&num_channels) {
        return Err(DecodeError::InvalidChannelCount(num_channels));
    }

    if !(MIN_DECODE_SAMPLE_RATE..=MAX_DECODE_SAMPLE_RATE).contains(&sample_rate) {
        return Err(DecodeError::InvalidSampleRate(sample_rate));
    }

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(DecodeError::Symphonia)?;

    let mut planar_channels: Vec<Vec<f32>> = vec![Vec::new(); num_channels];
    if let Some(est_frames) = track.codec_params.n_frames {
        let cap = (est_frames as usize).min(MAX_DECODE_FRAMES);
        for ch in &mut planar_channels {
            ch.reserve(cap);
        }
    }
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

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => {
                // Recoverable decode error, skip frame
                continue;
            }
            Err(err) => return Err(DecodeError::Symphonia(err)),
        };

        let spec = *decoded.spec();
        let decoded_channels = spec.channels.count();
        if !(1..=MAX_DECODE_CHANNELS).contains(&decoded_channels) {
            return Err(DecodeError::InvalidChannelCount(decoded_channels));
        }
        let duration = decoded.capacity() as u64;

        let sbuf = sample_buf.get_or_insert_with(|| SampleBuffer::new(duration, spec));
        sbuf.copy_interleaved_ref(decoded);

        let samples = sbuf.samples();

        // Check frame limit to avoid OOM
        let current_frames = planar_channels.first().map(|ch| ch.len()).unwrap_or(0);
        let incoming_frames = samples.len() / num_channels;
        if current_frames + incoming_frames > MAX_DECODE_FRAMES {
            return Err(DecodeError::FrameLimitExceeded);
        }

        for ch in &mut planar_channels {
            ch.reserve(incoming_frames);
        }

        // De-interleave into planar channels with NaN/Inf sanitization
        for chunk in samples.chunks(num_channels) {
            for (ch_idx, &sample) in chunk.iter().enumerate() {
                if ch_idx < planar_channels.len() {
                    let sanitized = if sample.is_nan() || sample.is_infinite() {
                        0.0
                    } else {
                        sample.clamp(-1.0, 1.0)
                    };
                    planar_channels[ch_idx].push(sanitized);
                }
            }
        }
    }

    let total_frames = planar_channels.first().map(|ch| ch.len()).unwrap_or(0) as u64;
    if total_frames == 0 {
        return Err(DecodeError::CorruptedHeader(
            "Decoded stream contains zero audio frames".to_string(),
        ));
    }

    let source = AudioSource {
        id: Uuid::new_v4(),
        path: path_buf,
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
