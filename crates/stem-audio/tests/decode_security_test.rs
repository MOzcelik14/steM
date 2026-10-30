use stem_audio::decode::{decode_bytes, MAX_DECODE_CHANNELS};

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
