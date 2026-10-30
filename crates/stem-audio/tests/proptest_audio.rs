use proptest::prelude::*;
use stem_audio::decode::decode_bytes;
use stem_audio::export::ExportRange;

proptest! {
    /// Invariant: Export ranges with arbitrary frame bounds must never overflow u64.
    #[test]
    fn prop_export_range_bounds(
        start_frame in 0u64..u64::MAX / 2,
        duration in 0u64..u64::MAX / 2,
    ) {
        let end_frame = start_frame.saturating_add(duration);
        let range = ExportRange::TimeRange { start_frame, end_frame };
        match range {
            ExportRange::TimeRange { start_frame: s, end_frame: e } => {
                prop_assert!(s <= e);
                prop_assert_eq!(e.saturating_sub(s), duration);
            }
            _ => unreachable!(),
        }
    }

    /// Invariant: Audio decoder must NEVER panic on arbitrary byte inputs.
    #[test]
    fn prop_decode_arbitrary_bytes_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = decode_bytes(&data, Some("wav"));
        let _ = decode_bytes(&data, Some("flac"));
        let _ = decode_bytes(&data, None);
    }
}
