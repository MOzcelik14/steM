use proptest::prelude::*;
use stem_core::project::{Clip, Project};
use uuid::Uuid;

proptest! {
    /// Invariant: Non-destructive split of a clip at any valid inner frame preserves total length
    /// and does not lose any audio frames.
    #[test]
    fn prop_split_clip_preserves_length(
        orig_length in 2u64..1_000_000u64,
        split_ratio in 0.01f64..0.99f64,
    ) {
        let mut proj = Project::default();
        let src_id = Uuid::new_v4();
        let t_id = proj.tracks[0].id;
        let track = proj.get_track_mut(t_id).unwrap();

        let clip = Clip::new("TestClip", src_id, 0, orig_length, 0);
        let clip_id = clip.id;
        track.clips.push(clip);

        let split_frame = ((orig_length as f64) * split_ratio).round().clamp(1.0, (orig_length - 1) as f64) as u64;
        let res = proj.split_clip(t_id, clip_id, split_frame);
        prop_assert!(res.is_ok());

        let track_after = proj.get_track(t_id).unwrap();
        prop_assert_eq!(track_after.clips.len(), 2);
        let c1 = &track_after.clips[0];
        let c2 = &track_after.clips[1];
        prop_assert_eq!(c1.length_frames + c2.length_frames, orig_length);
        prop_assert_eq!(c1.track_offset_frames, 0);
        prop_assert_eq!(c2.track_offset_frames, split_frame);
    }

    /// Invariant: Sample rate to time conversions round trip accurately without overflow or NaN.
    #[test]
    fn prop_time_frame_roundtrip(
        sample_rate in 8000u32..192000u32,
        frames in 0u64..1_000_000_000u64,
    ) {
        let seconds = frames as f64 / sample_rate as f64;
        prop_assert!(seconds.is_finite());
        let recovered_frames = (seconds * sample_rate as f64).round() as u64;
        prop_assert_eq!(recovered_frames, frames);
    }

    /// Invariant: Deserializing arbitrary random strings via from_ron must NEVER panic.
    #[test]
    fn prop_arbitrary_ron_never_panics(s in "\\PC*") {
        let _ = Project::from_ron(&s);
    }
}
