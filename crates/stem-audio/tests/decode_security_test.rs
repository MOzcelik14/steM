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

#[test]
fn test_decode_fuzzer_oom_flac_reproducer() {
    let reproducer = [102, 76, 97, 67, 6, 128, 0, 0, 102, 102, 67, 0, 231, 0, 0, 0, 42, 255, 42];
    let result = decode_bytes(&reproducer, Some("flac"));
    assert!(result.is_err());
}

#[test]
fn test_decode_fuzzer_adts_step_panic_reproducer() {
    let reproducer = [
        7, 0, 9, 32, 98, 116, 0, 0, 0, 31, 255, 102, 116, 121, 255, 241, 70, 79, 82, 77, 116,
        121, 67, 91, 121, 0, 37, 48, 255, 49,
    ];
    let result = decode_bytes(&reproducer, None);
    assert!(result.is_err());
}

#[test]
fn test_decode_fuzzer_aac_bounds_panic_reproducer() {
    let reproducer = [
        255, 241, 146, 116, 5, 255, 36, 121, 82, 255, 0, 0, 77, 254, 223, 255, 255, 0, 0, 0, 18,
        18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 1, 7, 0, 162, 85, 206, 255, 242, 18, 18, 97, 18,
        0, 102, 0, 0, 18, 18,
    ];
    let result = decode_bytes(&reproducer, None);
    assert!(result.is_err());
}


