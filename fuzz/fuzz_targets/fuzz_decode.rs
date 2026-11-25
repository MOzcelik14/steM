#![no_main]
use libfuzzer_sys::fuzz_target;
use stem_audio::decode::decode_bytes_with_limit;

const FUZZ_MAX_DECODE_BYTES: usize = 16 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    let _ = decode_bytes_with_limit(data, Some("wav"), FUZZ_MAX_DECODE_BYTES);
    let _ = decode_bytes_with_limit(data, Some("flac"), FUZZ_MAX_DECODE_BYTES);
    let _ = decode_bytes_with_limit(data, None, FUZZ_MAX_DECODE_BYTES);
});
