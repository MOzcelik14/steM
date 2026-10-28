use std::path::PathBuf;
use stem_core::history::{Action, UndoRedoStack};
use stem_core::project::{AudioSource, Clip, Project, Track};
use uuid::Uuid;

#[test]
fn test_project_creation_and_ron_roundtrip() {
    let mut proj = Project::new("My Session", 48000);
    assert_eq!(proj.name, "My Session");
    assert_eq!(proj.sample_rate, 48000);
    assert_eq!(proj.tracks.len(), 2);

    let src = AudioSource {
        id: Uuid::new_v4(),
        path: PathBuf::from("take1.wav"),
        filename: "take1.wav".to_string(),
        sample_rate: 48000,
        channels: 2,
        duration_frames: 48000 * 10,
        file_hash: None,
    };
    let src_id = src.id;
    proj.add_source(src);

    let clip = Clip::new("Vocal Take", src_id, 0, 48000 * 5, 0);
    let track_id = proj.tracks[0].id;
    proj.add_clip_to_track(track_id, clip).unwrap();

    assert_eq!(proj.total_frames(), 48000 * 5);

    // Test RON serialization / deserialization
    let ron_str = proj.to_ron().expect("Serialization to RON should succeed");
    let loaded_proj = Project::from_ron(&ron_str).expect("Deserialization from RON should succeed");

    assert_eq!(loaded_proj.name, proj.name);
    assert_eq!(loaded_proj.sample_rate, proj.sample_rate);
    assert_eq!(loaded_proj.tracks.len(), proj.tracks.len());
    assert_eq!(loaded_proj.total_frames(), proj.total_frames());
}

#[test]
fn test_non_destructive_split_clip() {
    let mut proj = Project::new("Split Test", 48000);
    let track_id = proj.tracks[0].id;
    let src_id = Uuid::new_v4();

    let clip = Clip::new("Guitar", src_id, 1000, 4000, 2000);
    let clip_id = clip.id;
    proj.add_clip_to_track(track_id, clip).unwrap();

    // Split at timeline frame 3000 (which is 1000 frames into the clip)
    let (c1_id, c2_id) = proj.split_clip(track_id, clip_id, 3000).unwrap();

    let track = proj.get_track(track_id).unwrap();
    assert_eq!(track.clips.len(), 2);

    let c1 = track.clips.iter().find(|c| c.id == c1_id).unwrap();
    let c2 = track.clips.iter().find(|c| c.id == c2_id).unwrap();

    assert_eq!(c1.track_offset_frames, 2000);
    assert_eq!(c1.length_frames, 1000);
    assert_eq!(c1.source_start_frame, 1000);

    assert_eq!(c2.track_offset_frames, 3000);
    assert_eq!(c2.length_frames, 3000);
    assert_eq!(c2.source_start_frame, 2000);
}

#[test]
fn test_undo_redo_stack() {
    let mut proj = Project::new("Undo Test", 44100);
    let mut history = UndoRedoStack::new();

    let initial_tracks_len = proj.tracks.len();
    let new_track = Track::new("Guitar 2");
    let new_track_id = new_track.id;

    history
        .push_and_execute(Action::AddTrack(new_track), &mut proj)
        .unwrap();
    assert_eq!(proj.tracks.len(), initial_tracks_len + 1);
    assert!(history.can_undo());
    assert!(!history.can_redo());

    // Undo
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.tracks.len(), initial_tracks_len);
    assert!(proj.get_track(new_track_id).is_none());
    assert!(history.can_redo());

    // Redo
    history.redo(&mut proj).unwrap();
    assert_eq!(proj.tracks.len(), initial_tracks_len + 1);
    assert!(proj.get_track(new_track_id).is_some());
}

#[test]
fn test_generate_demo_project() {
    let (proj, buffers) = stem_core::generate_demo_project(48000);
    assert_eq!(proj.tracks.len(), 3);
    assert_eq!(buffers.len(), 3);
    assert!(proj.total_frames() > 0);
    for buf in buffers.values() {
        assert_eq!(buf.len(), 2);
        assert_eq!(buf[0].len(), proj.total_frames() as usize);
    }
}

#[test]
fn test_edit_list_undo_redo_full_sequence() {
    let mut proj = Project::new("Full Undo Test", 48000);
    let mut history = UndoRedoStack::new();

    // 1. Add Track
    let track = Track::new("Bass");
    let track_id = track.id;
    history
        .push_and_execute(Action::AddTrack(track), &mut proj)
        .unwrap();
    assert_eq!(proj.tracks.len(), 3); // 2 default + 1 added

    // 2. Add Clip
    let src_id = Uuid::new_v4();
    let clip1 = Clip::new("Take 1", src_id, 0, 48000, 1000);
    let clip1_id = clip1.id;
    history
        .push_and_execute(
            Action::AddClip {
                track_id,
                clip: clip1.clone(),
            },
            &mut proj,
        )
        .unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().clips.len(), 1);

    // 3. Move Clip
    history
        .push_and_execute(
            Action::MoveClip {
                track_id,
                clip_id: clip1_id,
                old_track_offset: 1000,
                new_track_offset: 5000,
            },
            &mut proj,
        )
        .unwrap();
    assert_eq!(
        proj.get_track(track_id).unwrap().clips[0].track_offset_frames,
        5000
    );

    // 4. Trim Clip
    history
        .push_and_execute(
            Action::TrimClip {
                track_id,
                clip_id: clip1_id,
                old_source_start: 0,
                old_length: 48000,
                old_track_offset: 5000,
                new_source_start: 500,
                new_length: 24000,
                new_track_offset: 5500,
            },
            &mut proj,
        )
        .unwrap();
    let c = &proj.get_track(track_id).unwrap().clips[0];
    assert_eq!(c.source_start_frame, 500);
    assert_eq!(c.length_frames, 24000);
    assert_eq!(c.track_offset_frames, 5500);

    // 5. Set Track Gain & Pan
    history
        .push_and_execute(
            Action::SetTrackGain {
                track_id,
                old_gain: 0.0,
                new_gain: -3.5,
            },
            &mut proj,
        )
        .unwrap();
    history
        .push_and_execute(
            Action::SetTrackPan {
                track_id,
                old_pan: 0.0,
                new_pan: -0.4,
            },
            &mut proj,
        )
        .unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().gain_db, -3.5);
    assert_eq!(proj.get_track(track_id).unwrap().pan, -0.4);

    // 6. Set Master Gain
    history
        .push_and_execute(
            Action::SetMasterGain {
                old_gain: 0.0,
                new_gain: 2.0,
            },
            &mut proj,
        )
        .unwrap();
    assert_eq!(proj.master_gain_db, 2.0);

    // 7. Remove Clip
    let current_clip = proj.get_track(track_id).unwrap().clips[0].clone();
    history
        .push_and_execute(
            Action::RemoveClip {
                track_id,
                clip: current_clip,
                index: 0,
            },
            &mut proj,
        )
        .unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().clips.len(), 0);

    // Undo step 7 (Re-adds clip)
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().clips.len(), 1);

    // Undo step 6 (Master Gain reverts to 0.0)
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.master_gain_db, 0.0);

    // Undo step 5 (Track Pan reverts to 0.0, Gain reverts to 0.0)
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().pan, 0.0);
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().gain_db, 0.0);

    // Undo step 4 (Trim reverts)
    history.undo(&mut proj).unwrap();
    let c = &proj.get_track(track_id).unwrap().clips[0];
    assert_eq!(c.source_start_frame, 0);
    assert_eq!(c.length_frames, 48000);
    assert_eq!(c.track_offset_frames, 5000);

    // Undo step 3 (Move reverts offset to 1000)
    history.undo(&mut proj).unwrap();
    assert_eq!(
        proj.get_track(track_id).unwrap().clips[0].track_offset_frames,
        1000
    );

    // Undo step 2 (Clip removed)
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.get_track(track_id).unwrap().clips.len(), 0);

    // Undo step 1 (Track removed)
    history.undo(&mut proj).unwrap();
    assert_eq!(proj.tracks.len(), 2);
    assert!(!history.can_undo());

    // Redo all steps back forward
    while history.can_redo() {
        history.redo(&mut proj).unwrap();
    }
    assert_eq!(proj.tracks.len(), 3);
    assert_eq!(proj.master_gain_db, 2.0);
    assert_eq!(proj.get_track(track_id).unwrap().gain_db, -3.5);
    assert_eq!(proj.get_track(track_id).unwrap().pan, -0.4);
    assert_eq!(proj.get_track(track_id).unwrap().clips.len(), 0); // Since step 7 removed it
}

#[test]
fn test_fade_curves_and_gain_math() {
    use stem_core::project::FadeCurve;

    let curves = [
        FadeCurve::Linear,
        FadeCurve::Logarithmic,
        FadeCurve::Exponential,
        FadeCurve::SCurve,
    ];

    for curve in &curves {
        // Boundary tests
        assert!((curve.evaluate(0.0) - 0.0).abs() < 1e-6);
        assert!((curve.evaluate(1.0) - 1.0).abs() < 1e-6);

        // Clamping tests
        assert_eq!(curve.evaluate(-0.5), 0.0);
        assert_eq!(curve.evaluate(1.5), 1.0);

        // Monotonicity test
        let mut prev = 0.0f32;
        for i in 1..=100 {
            let t = i as f32 / 100.0;
            let val = curve.evaluate(t);
            assert!(
                val >= prev,
                "Fade curve {:?} is not monotonic: prev={prev}, curr={val} at t={t}",
                curve
            );
            prev = val;
        }
    }

    // Midpoint characteristics
    let lin_mid = FadeCurve::Linear.evaluate(0.5);
    let log_mid = FadeCurve::Logarithmic.evaluate(0.5);
    let exp_mid = FadeCurve::Exponential.evaluate(0.5);
    let s_mid = FadeCurve::SCurve.evaluate(0.5);

    assert!((lin_mid - 0.5).abs() < 1e-6);
    assert!((s_mid - 0.5).abs() < 1e-6);
    assert!(
        log_mid > 0.5,
        "Logarithmic curve should be convex (>0.5 at midpoint)"
    );
    assert!(
        exp_mid < 0.5,
        "Exponential curve should be concave (<0.5 at midpoint)"
    );

    // Gain Math: dB to linear
    let db_to_linear = |db: f32| 10.0f32.powf(db / 20.0);
    assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
    assert!((db_to_linear(6.0206) - 2.0).abs() < 1e-4);
    assert!((db_to_linear(-6.0206) - 0.5).abs() < 1e-4);
    assert!((db_to_linear(-20.0) - 0.1).abs() < 1e-4);
    assert!((db_to_linear(20.0) - 10.0).abs() < 1e-4);
}
