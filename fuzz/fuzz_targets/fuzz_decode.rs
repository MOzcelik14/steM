#![no_main]
use libfuzzer_sys::fuzz_target;
use stem_audio::decode::decode_bytes;

fuzz_target!(|data: &[u8]| {
    let _ = decode_bytes(data, Some("wav"));
    let _ = decode_bytes(data, Some("flac"));
    let _ = decode_bytes(data, None);
});
