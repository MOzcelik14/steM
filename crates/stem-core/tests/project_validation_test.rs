use std::path::PathBuf;
use stem_core::project::{AudioSource, Clip, Project};
use uuid::Uuid;

#[test]
fn test_valid_project_validates() {
    let proj = Project::default();
    assert!(proj.validate().is_ok());
}

#[test]
fn test_rejects_path_traversal() {
    let mut proj = Project::default();
    let src_id = Uuid::new_v4();
    proj.sources.insert(
        src_id,
        AudioSource {
            id: src_id,
            path: PathBuf::from("../../etc/shadow"),
            filename: "shadow".to_string(),
            sample_rate: 48000,
            channels: 2,
            duration_frames: 48000,
            file_hash: None,
        },
    );

    let err = proj.validate();
    assert!(err.is_err());
    let err_msg = err.unwrap_err().to_string();
    assert!(err_msg.contains("directory traversal"));
}

#[test]
fn test_rejects_clip_exceeding_source_duration() {
    let mut proj = Project::default();
    let src_id = Uuid::new_v4();
    proj.sources.insert(
        src_id,
        AudioSource {
            id: src_id,
            path: PathBuf::from("audio/vocals.wav"),
            filename: "vocals.wav".to_string(),
            sample_rate: 48000,
            channels: 2,
            duration_frames: 1000,
            file_hash: None,
        },
    );

    let t_id = proj.tracks[0].id;
    let track = proj.get_track_mut(t_id).unwrap();
    let mut clip = Clip::new("Vocals", src_id, 0, 1000, 0);
    clip.source_start_frame = 500;
    clip.length_frames = 600; // 500 + 600 = 1100 > 1000
    track.clips.push(clip);

    let err = proj.validate();
    assert!(err.is_err());
}

#[test]
fn test_rejects_zero_length_clip() {
    let mut proj = Project::default();
    let t_id = proj.tracks[0].id;
    let track = proj.get_track_mut(t_id).unwrap();
    let clip = Clip::new("Zero", Uuid::new_v4(), 0, 0, 0);
    track.clips.push(clip);

    assert!(proj.validate().is_err());
}

#[test]
fn test_rejects_invalid_sample_rate() {
    let proj_low = Project {
        sample_rate: 100,
        ..Default::default()
    };
    assert!(proj_low.validate().is_err());

    let proj_high = Project {
        sample_rate: 500_000,
        ..Default::default()
    };
    assert!(proj_high.validate().is_err());
}

#[test]
fn test_rejects_malicious_ron_string() {
    let malicious_ron = r#"
        Project(
            id: "00000000-0000-0000-0000-000000000000",
            name: "Malicious",
            sample_rate: 48000,
            tempo_bpm: 120.0,
            time_signature: (4, 4),
            sources: {
                "00000000-0000-0000-0000-000000000001": AudioSource(
                    id: "00000000-0000-0000-0000-000000000001",
                    path: "../../../secret.key",
                    filename: "secret",
                    sample_rate: 48000,
                    channels: 2,
                    duration_frames: 10000,
                )
            },
            tracks: [],
            master_gain_db: 0.0,
            loop_enabled: false,
            playhead_frame: 0,
        )
    "#;

    let res = Project::from_ron(malicious_ron);
    assert!(res.is_err(), "Must reject RON with path traversal");
}

#[test]
fn test_malformed_project_files_rejected() {
    let malformed_cases = [
        "",
        "not valid ron at all {{{",
        "Project(name: \"Truncated\"",
        "Project(id: \"invalid-uuid\", name: \"Bad\", sample_rate: 48000)",
    ];
    for case in malformed_cases {
        assert!(
            Project::from_ron(case).is_err(),
            "Malformed RON should be rejected: {case}"
        );
    }
}

#[test]
fn test_large_track_count_and_serialization_perf() {
    let mut proj = Project::new("Stress 100+ Tracks", 48000);
    while proj.tracks.len() < stem_core::MAX_PROJECT_TRACKS {
        proj.add_track(format!("Track {}", proj.tracks.len() + 1));
    }
    assert!(proj.validate().is_ok());

    let start = std::time::Instant::now();
    let ron = proj.to_ron().expect("serialize MAX_PROJECT_TRACKS");
    let restored = Project::from_ron(&ron).expect("deserialize MAX_PROJECT_TRACKS");
    let elapsed = start.elapsed();
    assert_eq!(restored.tracks.len(), stem_core::MAX_PROJECT_TRACKS);
    assert!(
        elapsed.as_secs_f32() < 2.0,
        "Serialization roundtrip took too long: {elapsed:?}"
    );

    // 100+ tracks (120) must be cleanly rejected by validation
    while proj.tracks.len() < 120 {
        proj.add_track(format!("Overflow {}", proj.tracks.len() + 1));
    }
    assert_eq!(proj.tracks.len(), 120);
    assert!(proj.validate().is_err());
}

#[test]
fn test_rejects_future_schema_version() {
    let proj = Project::new("Future Project", 48000);
    let ron = proj.to_ron().expect("serialize project");
    let future_ron = ron.replacen("schema_version: 1,", "schema_version: 999,", 1);
    assert!(future_ron.contains("schema_version: 999,"));

    let err = Project::from_ron(&future_ron).expect_err("future schema_version must be rejected");
    assert!(
        matches!(
            err,
            stem_core::ProjectError::UnsupportedSchemaVersion {
                found: 999,
                supported
            } if supported == stem_core::project::CURRENT_SCHEMA_VERSION
        ),
        "expected UnsupportedSchemaVersion, got {err:?}"
    );
}

#[test]
fn test_v1_project_loads_and_migrates_legacy_v0() {
    let proj = Project::new("V1 Project", 48000);
    let ron = proj.to_ron().expect("serialize v1 project");
    let restored = Project::from_ron(&ron).expect("v1 project must load cleanly");
    assert_eq!(
        restored.schema_version,
        stem_core::project::CURRENT_SCHEMA_VERSION
    );

    // Simulate legacy v0 schema_version and verify migration upgrades to CURRENT_SCHEMA_VERSION
    let legacy_ron = ron.replacen("schema_version: 1,", "schema_version: 0,", 1);
    let migrated = Project::from_ron(&legacy_ron).expect("v0 project must migrate to current");
    assert_eq!(
        migrated.schema_version,
        stem_core::project::CURRENT_SCHEMA_VERSION
    );
}
