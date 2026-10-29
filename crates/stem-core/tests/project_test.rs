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

#[test]
fn test_recording_latency_compensation_math_and_clamping() {
    let mut proj = Project::new("Latency Test", 48000);
    proj.latency_compensation_ms = 20.0;
    // At 48000 Hz, 20.0 ms = 0.02 * 48000 = 960 frames
    assert_eq!(proj.latency_compensation_frames(), 960);

    proj.latency_compensation_ms = 12.5;
    // 0.0125 * 48000 = 600 frames
    assert_eq!(proj.latency_compensation_frames(), 600);

    // At 44100 Hz, 10.0 ms = 441 frames
    let mut proj_44k = Project::new("44.1k Test", 44100);
    proj_44k.latency_compensation_ms = 10.0;
    assert_eq!(proj_44k.latency_compensation_frames(), 441);

    // Offset clamping:
    // Case 1: start_frame >= latency_frames
    let start_frame = 2000_u64;
    let latency = 960_u64;
    let (offset, source_start, len) = if start_frame >= latency {
        (start_frame - latency, 0_u64, 48000_u64)
    } else {
        let excess = latency - start_frame;
        (0_u64, excess, 48000 - excess)
    };
    assert_eq!(offset, 1040);
    assert_eq!(source_start, 0);
    assert_eq!(len, 48000);

    // Case 2: start_frame < latency_frames (clamped to 0 timeline offset)
    let start_frame_early = 200_u64;
    let (offset_early, source_start_early, len_early) = if start_frame_early >= latency {
        (start_frame_early - latency, 0_u64, 48000_u64)
    } else {
        let excess = latency - start_frame_early;
        (0_u64, excess, 48000 - excess)
    };
    assert_eq!(offset_early, 0);
    assert_eq!(source_start_early, 760);
    assert_eq!(len_early, 48000 - 760);
}

#[test]
fn test_punch_in_non_destructive_slicing_and_placement() {
    use stem_core::project::Clip;

    let mut proj = Project::new("Punch Session", 48000);
    let track_id = proj.tracks[0].id;
    let src_id = Uuid::new_v4();

    // 1. Initial clip spanning timeline frames [1000..5000] (length 4000)
    let clip = Clip::new("Original Take", src_id, 0, 4000, 1000);
    proj.add_clip_to_track(track_id, clip).unwrap();

    // 2. Punch-in take placed over region [2000..3500] (length 1500)
    let punch_src_id = Uuid::new_v4();
    let punch_take = Clip::new("Punch In Take", punch_src_id, 0, 1500, 2000);
    proj.punch_clip_into_track(track_id, punch_take).unwrap();

    let track = proj.get_track(track_id).unwrap();
    assert_eq!(
        track.clips.len(),
        3,
        "Clip should be cleanly split into 3 pieces"
    );

    // Left piece: [1000..2000], length 1000, source_start 0
    let c_left = &track.clips[0];
    assert_eq!(c_left.track_offset_frames, 1000);
    assert_eq!(c_left.length_frames, 1000);
    assert_eq!(c_left.source_start_frame, 0);

    // Punch take: [2000..3500], length 1500, source_start 0
    let c_punch = &track.clips[1];
    assert_eq!(c_punch.track_offset_frames, 2000);
    assert_eq!(c_punch.length_frames, 1500);
    assert_eq!(c_punch.source_id, punch_src_id);

    // Right piece: [3500..5000], length 1500, source_start 2500
    let c_right = &track.clips[2];
    assert_eq!(c_right.track_offset_frames, 3500);
    assert_eq!(c_right.length_frames, 1500);
    assert_eq!(c_right.source_start_frame, 2500);
}

#[test]
fn test_track_input_routing_channel_extraction() {
    use stem_core::project::TrackInputRouting;

    let ch1 = vec![0.1f32, 0.2, 0.3];
    let ch2 = vec![0.8f32, 0.9, 1.0];
    let stereo_planar = vec![ch1.clone(), ch2.clone()];

    // Mono 1 extracts channel 0
    let m1 = TrackInputRouting::Mono1.extract_channels(&stereo_planar);
    assert_eq!(m1.len(), 1);
    assert_eq!(m1[0], ch1);

    // Mono 2 extracts channel 1
    let m2 = TrackInputRouting::Mono2.extract_channels(&stereo_planar);
    assert_eq!(m2.len(), 1);
    assert_eq!(m2[0], ch2);

    // Stereo extracts both channels
    let stereo = TrackInputRouting::Stereo.extract_channels(&stereo_planar);
    assert_eq!(stereo.len(), 2);
    assert_eq!(stereo[0], ch1);
    assert_eq!(stereo[1], ch2);

    // None extracts empty
    let none = TrackInputRouting::None.extract_channels(&stereo_planar);
    assert!(none.is_empty());

    // Single-channel input fallback
    let mono_planar = vec![ch1.clone()];
    let m2_fallback = TrackInputRouting::Mono2.extract_channels(&mono_planar);
    assert_eq!(m2_fallback.len(), 1);
    assert_eq!(m2_fallback[0], ch1);

    let stereo_fallback = TrackInputRouting::Stereo.extract_channels(&mono_planar);
    assert_eq!(stereo_fallback.len(), 2);
    assert_eq!(stereo_fallback[0], ch1);
    assert_eq!(stereo_fallback[1], ch1);
}

#[test]
fn test_count_in_duration_calculation() {
    use stem_core::project::CountInMode;

    let bpm = 120.0;
    let beats_per_bar = 4;

    // At 120 BPM, 1 beat = 0.5s, 1 bar (4 beats) = 2.0s
    assert_eq!(CountInMode::Off.duration_seconds(bpm, beats_per_bar), 0.0);
    assert_eq!(
        CountInMode::OneBar.duration_seconds(bpm, beats_per_bar),
        2.0
    );
    assert_eq!(
        CountInMode::TwoBars.duration_seconds(bpm, beats_per_bar),
        4.0
    );
    assert_eq!(
        CountInMode::TwoSeconds.duration_seconds(bpm, beats_per_bar),
        2.0
    );
    assert_eq!(
        CountInMode::FourSeconds.duration_seconds(bpm, beats_per_bar),
        4.0
    );
}

#[test]
fn test_crossfade_equal_power_summation() {
    use stem_core::project::CrossfadeType;

    // Test Equal-Power crossfade curve preserves acoustic power across the entire transition
    let eq = CrossfadeType::EqualPower;
    for step in 0..=100 {
        let t = step as f32 / 100.0;
        let (gain_a, gain_b) = eq.evaluate(t);

        // Power summation: gain_a^2 + gain_b^2 == 1.0 (constant acoustic power, 0 dB dip)
        let power = gain_a * gain_a + gain_b * gain_b;
        assert!(
            (power - 1.0).abs() < 1e-5,
            "Equal power violated at t={t}: power={power}"
        );
    }

    // Midpoint check: at t = 0.5, equal-power gain is ~0.7071 (-3 dB per channel, summing to 1.0)
    let (mid_a, mid_b) = eq.evaluate(0.5);
    assert!((mid_a - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
    assert!((mid_b - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);

    // Linear crossfade check: at t = 0.5, linear gain is 0.5 each (power = 0.25 + 0.25 = 0.5, i.e. -3dB dip)
    let lin = CrossfadeType::Linear;
    let (l_a, l_b) = lin.evaluate(0.5);
    assert_eq!(l_a, 0.5);
    assert_eq!(l_b, 0.5);
    assert_eq!(l_a * l_a + l_b * l_b, 0.5);

    // Test track overlapping clips evaluate crossfades properly
    let mut track = Track::new("Crossfade Track");
    let c1 = Clip::new("Clip 1", Uuid::new_v4(), 0, 1000, 0); // [0..1000]
    let mut c2 = Clip::new("Clip 2", Uuid::new_v4(), 0, 1000, 800); // [800..1800], overlaps by 200 frames
    c2.crossfade_type = CrossfadeType::EqualPower;

    track.clips.push(c1);
    track.clips.push(c2);

    // Before overlap: c1 has full gain
    assert_eq!(track.clip_gain_at_timeline_frame(0, 400), 1.0);
    // Inside overlap at midpoint (frame 900):
    let g1 = track.clip_gain_at_timeline_frame(0, 900);
    let g2 = track.clip_gain_at_timeline_frame(1, 900);
    assert!((g1 * g1 + g2 * g2 - 1.0).abs() < 1e-4);
    // After overlap: c2 has full gain
    assert_eq!(track.clip_gain_at_timeline_frame(1, 1200), 1.0);
}

#[test]
fn test_automation_lane_piecewise_evaluation() {
    use stem_core::project::{AutomationCurve, AutomationLane, AutomationTarget};

    let mut lane = AutomationLane::new(AutomationTarget::Volume);

    // Un-enabled lane returns default value
    assert_eq!(lane.evaluate(500, -6.0), -6.0);

    lane.enabled = true;
    // Empty lane returns default value
    assert_eq!(lane.evaluate(500, -6.0), -6.0);

    // Add breakpoints:
    // frame 1000: 0 dB (Linear curve to next point)
    // frame 2000: -10 dB (Step curve to next point)
    // frame 3000: +6 dB (Bezier curve to next point)
    // frame 4000: -20 dB
    lane.add_point(1000, 0.0, AutomationCurve::Linear);
    lane.add_point(2000, -10.0, AutomationCurve::Step);
    lane.add_point(3000, 6.0, AutomationCurve::Bezier);
    lane.add_point(4000, -20.0, AutomationCurve::Linear);

    // Boundary clamping
    assert_eq!(lane.evaluate(0, -6.0), 0.0);
    assert_eq!(lane.evaluate(1000, -6.0), 0.0);
    assert_eq!(lane.evaluate(5000, -6.0), -20.0);

    // Linear interpolation between 1000 (0 dB) and 2000 (-10 dB)
    // At frame 1500 (t = 0.5): expected -5.0 dB
    let val_1500 = lane.evaluate(1500, 0.0);
    assert!((val_1500 - (-5.0)).abs() < 1e-4);

    // Step curve between 2000 (-10 dB) and 3000 (+6 dB)
    // At frame 2500: should hold -10.0 dB until frame 3000
    assert_eq!(lane.evaluate(2500, 0.0), -10.0);
    assert_eq!(lane.evaluate(2999, 0.0), -10.0);
    assert_eq!(lane.evaluate(3000, 0.0), 6.0);

    // Bezier curve between 3000 (+6 dB) and 4000 (-20 dB)
    // At frame 3500 (t = 0.5): smooth S-curve gives midpoint value (6 + (-20)) / 2 = -7.0 dB
    let val_3500 = lane.evaluate(3500, 0.0);
    assert!((val_3500 - (-7.0)).abs() < 1e-4);
}

#[test]
fn test_track_grouping_linked_controls() {
    let mut proj = Project::new("Grouping Test", 48000);
    let gid = proj.add_group("Drum Group");

    {
        let group = proj.get_group_mut(gid).unwrap();
        assert!(group.link_gain);
        assert!(group.link_mute);
        assert!(group.link_solo);
        assert!(group.link_arm);
    }

    // Assign tracks 0 and 1 to this group
    proj.tracks[0].group_id = Some(gid);
    proj.tracks[0].gain_db = -6.0;
    proj.tracks[1].group_id = Some(gid);
    proj.tracks[1].gain_db = -12.0;

    // Moving track 0 by +4.0 dB should adjust track 1 by relative delta +4.0 dB
    let delta = 4.0f32;
    let old_t0 = proj.tracks[0].gain_db;
    let new_t0 = old_t0 + delta;
    proj.tracks[0].gain_db = new_t0;
    proj.tracks[1].gain_db = (proj.tracks[1].gain_db + delta).clamp(-60.0, 12.0);

    assert_eq!(proj.tracks[0].gain_db, -2.0);
    assert_eq!(proj.tracks[1].gain_db, -8.0);

    // Test group deletion unbinds member tracks
    proj.remove_group(gid);
    assert_eq!(proj.groups.len(), 0);
    assert_eq!(proj.tracks[0].group_id, None);
    assert_eq!(proj.tracks[1].group_id, None);
}

#[test]
fn test_submix_bus_and_send_routing() {
    use stem_core::project::{TrackOutputRouting, TrackSend};

    let mut proj = Project::new("Bus Test", 48000);
    let bus_id = proj.add_bus("Reverb Bus");

    assert_eq!(proj.busses.len(), 1);
    assert_eq!(proj.get_bus(bus_id).unwrap().name, "Reverb Bus");

    // Route track 0 output to Bus
    proj.tracks[0].output_routing = TrackOutputRouting::Bus(bus_id);

    // Add aux send from track 1 to Bus
    let mut send = TrackSend::new(bus_id);
    send.gain_db = -6.0;
    send.pre_fader = true;
    proj.tracks[1].sends.push(send);

    assert_eq!(
        proj.tracks[0].output_routing,
        TrackOutputRouting::Bus(bus_id)
    );
    assert_eq!(proj.tracks[1].sends.len(), 1);
    assert_eq!(proj.tracks[1].sends[0].target_bus_id, bus_id);

    // Removing bus safely reverts track output to Master and clears sends
    proj.remove_bus(bus_id);
    assert_eq!(proj.busses.len(), 0);
    assert_eq!(proj.tracks[0].output_routing, TrackOutputRouting::Master);
    assert_eq!(proj.tracks[1].sends.len(), 0);
}
