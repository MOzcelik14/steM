use std::collections::HashMap;
use std::path::PathBuf;
use stem_audio::export::{export_project_to_file, ExportError, ExportSettings};
use stem_core::project::{AudioSource, Project};
use uuid::Uuid;

#[test]
fn test_export_prevents_source_file_overwrite() {
    let mut proj = Project::default();
    let src_id = Uuid::new_v4();
    let source_path = PathBuf::from("my_vocal_recording.wav");
    proj.sources.insert(
        src_id,
        AudioSource {
            id: src_id,
            path: source_path.clone(),
            filename: "my_vocal_recording.wav".to_string(),
            sample_rate: 48000,
            channels: 2,
            duration_frames: 48000,
            file_hash: None,
        },
    );

    let buffers = HashMap::new();
    let settings = ExportSettings::default();

    let result = export_project_to_file(&proj, &buffers, &source_path, &settings, None, None);
    match result {
        Err(ExportError::DestinationCollidesWithSource(p)) => {
            assert_eq!(p, source_path);
        }
        other => panic!(
            "Expected DestinationCollidesWithSource error, got {:?}",
            other
        ),
    }
}
