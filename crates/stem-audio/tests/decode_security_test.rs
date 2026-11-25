use stem_audio::decode::{decode_bytes, decode_bytes_with_limit, DecodeError, MAX_DECODE_CHANNELS};

#[test]
fn test_decode_empty_bytes() {
    let result = decode_bytes(&[], Some("wav"));
    assert!(
        result.is_err(),
        "Empty byte stream must return error, not panic"
    );
}

#[test]
fn test_decode_garbage_bytes() {
    let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF, 0x42, 0x13];
    let result = decode_bytes(&garbage, Some("wav"));
    assert!(
        result.is_err(),
        "Garbage bytes must return error, not panic"
    );
}

#[test]
fn test_decode_truncated_wav_header() {
    let truncated_wav = b"RIFF\x24\x00\x00\x00WAVEfmt ";
    let result = decode_bytes(truncated_wav, Some("wav"));
    assert!(
        result.is_err(),
        "Truncated header must return error, not panic"
    );
}

#[test]
fn test_channel_and_sample_rate_limits() {
    assert_eq!(MAX_DECODE_CHANNELS, 8);
}

#[test]
fn test_decode_fuzzer_oom_flac_reproducer() {
    let reproducer = [
        102, 76, 97, 67, 6, 128, 0, 0, 102, 102, 67, 0, 231, 0, 0, 0, 42, 255, 42,
    ];
    let result = decode_bytes(&reproducer, Some("flac"));
    assert!(result.is_err());
}

#[test]
fn test_decode_fuzzer_adts_step_panic_reproducer() {
    let reproducer = [
        7, 0, 9, 32, 98, 116, 0, 0, 0, 31, 255, 102, 116, 121, 255, 241, 70, 79, 82, 77, 116, 121,
        67, 91, 121, 0, 37, 48, 255, 49,
    ];
    let result = decode_bytes(&reproducer, None);
    assert!(result.is_err());
}

#[test]
fn test_decode_fuzzer_aac_bounds_panic_reproducer() {
    let reproducer = [
        255, 241, 146, 116, 5, 255, 36, 121, 82, 255, 0, 0, 77, 254, 223, 255, 255, 0, 0, 0, 18,
        18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 1, 7, 0, 162, 85, 206, 255, 242, 18, 18, 97, 18, 0,
        102, 0, 0, 18, 18,
    ];
    let result = decode_bytes(&reproducer, None);
    assert!(result.is_err());
}

fn build_pcm_wav_bytes(
    sample_rate: u32,
    channels: u16,
    samples: &[i16],
    header_data_bytes: Option<u32>,
) -> Vec<u8> {
    let actual_data_bytes = (samples.len() * 2) as u32;
    let claimed_data_bytes = header_data_bytes.unwrap_or(actual_data_bytes);
    let riff_size = 36u32.saturating_add(claimed_data_bytes);
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;

    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&claimed_data_bytes.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[test]
fn test_decode_rejects_when_byte_limit_exceeded() {
    // 2000 stereo frames * 2 channels * 4 bytes (f32) = 16,000 bytes
    let pcm = vec![0i16; 2000 * 2];
    let wav = build_pcm_wav_bytes(48_000, 2, &pcm, None);

    // Limit of 4,096 bytes must reject cleanly with DecodeError::TooLarge
    let err = decode_bytes_with_limit(&wav, Some("wav"), 4096)
        .err()
        .expect("must reject audio exceeding byte limit");
    assert!(
        matches!(
            err,
            DecodeError::TooLarge {
                limit_bytes: 4096,
                ..
            }
        ),
        "expected TooLarge, got {err:?}"
    );
}

#[test]
fn test_decode_enforces_limit_even_with_misleading_small_header() {
    // Header claims only 128 bytes of data, but actual stream holds 4000 stereo frames (32,000 f32 bytes)
    let pcm = vec![100i16; 4000 * 2];
    let wav = build_pcm_wav_bytes(48_000, 2, &pcm, Some(128));
    // Must either reject with TooLarge or clamp to claimed 128 bytes (32 frames * 8 = 256 bytes <= 2048);
    // verify with a 1024-byte limit that decoded output never exceeds limit_bytes
    match decode_bytes_with_limit(&wav, Some("wav"), 1024) {
        Ok(decoded) => {
            let decoded_bytes = decoded.channels.len()
                * decoded.channels.first().map(|c| c.len()).unwrap_or(0)
                * std::mem::size_of::<f32>();
            assert!(decoded_bytes <= 1024);
        }
        Err(DecodeError::TooLarge { limit_bytes, .. }) => assert_eq!(limit_bytes, 1024),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}
