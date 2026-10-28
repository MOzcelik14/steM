use std::collections::HashMap;
use std::fs;
use stem_audio::{decode_file, export_project_to_file, BitDepth, ExportFormat, ExportSettings};
use stem_core::project::{AudioSource, Clip, Project};
use uuid::Uuid;

#[test]
fn test_export_and_decode_wav_and_flac_roundtrip() {
    let temp_dir = std::env::temp_dir().join(format!("stem_test_{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).expect("Create temp test dir");

    let wav_path = temp_dir.join("test_output.wav");
    let flac_path = temp_dir.join("test_output.flac");

    // 1. Build a synthetic project
    let sample_rate = 48000;
    let mut proj = Project::new("Export Test", sample_rate);
    let track_id = proj.tracks[0].id;

    // Generate 1 second of synthetic 440 Hz stereo audio source
    let source_id = Uuid::new_v4();
    let num_frames = sample_rate as usize;
    let mut left = Vec::with_capacity(num_frames);
    let mut right = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        left.push(s);
        right.push(s);
    }

    let source = AudioSource {
        id: source_id,
        path: temp_dir.join("virtual_source.wav"),
        filename: "virtual_source.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: num_frames as u64,
        file_hash: None,
    };
    proj.add_source(source);

    let clip = Clip::new("Synth 440Hz", source_id, 0, num_frames as u64, 0);
    proj.add_clip_to_track(track_id, clip).unwrap();

    let mut source_buffers = HashMap::new();
    source_buffers.insert(source_id, vec![left, right]);

    // 2. Test WAV 24-bit export with TPDF dither
    let wav_settings = ExportSettings {
        format: ExportFormat::Wav,
        sample_rate,
        bit_depth: BitDepth::Bit24,
        apply_dither: true,
        normalize_target_lufs: None,
        range: stem_audio::ExportRange::EntireProject,
    };

    export_project_to_file(&proj, &source_buffers, &wav_path, &wav_settings, None, None)
        .expect("WAV export should succeed");

    assert!(wav_path.exists());

    // 3. Decode exported WAV file using Symphonia
    let decoded_wav = decode_file(&wav_path).expect("Decode exported WAV");
    assert_eq!(decoded_wav.source.sample_rate, sample_rate);
    assert_eq!(decoded_wav.channels.len(), 2);
    assert_eq!(decoded_wav.source.duration_frames, num_frames as u64);

    // 4. Test FLAC 16-bit export with TPDF dither
    let flac_settings = ExportSettings {
        format: ExportFormat::Flac,
        sample_rate,
        bit_depth: BitDepth::Bit16,
        apply_dither: true,
        normalize_target_lufs: None,
        range: stem_audio::ExportRange::EntireProject,
    };

    export_project_to_file(
        &proj,
        &source_buffers,
        &flac_path,
        &flac_settings,
        None,
        None,
    )
    .expect("FLAC export should succeed");

    assert!(flac_path.exists());

    // 5. Decode exported FLAC file using Symphonia
    let decoded_flac = decode_file(&flac_path).expect("Decode exported FLAC");
    assert_eq!(decoded_flac.source.sample_rate, sample_rate);
    assert_eq!(decoded_flac.channels.len(), 2);
    // FLAC fixed block encoding may pad the last block up to block boundary
    assert!(decoded_flac.source.duration_frames >= num_frames as u64);

    // 6. Test TimeRange Selection export (frames 1000..21000 => 20000 frames)
    let range_wav_path = temp_dir.join("selection_output.wav");
    let range_settings = ExportSettings {
        format: ExportFormat::Wav,
        sample_rate,
        bit_depth: BitDepth::Bit16,
        apply_dither: false,
        normalize_target_lufs: None,
        range: stem_audio::ExportRange::TimeRange {
            start_frame: 1000,
            end_frame: 21000,
        },
    };
    export_project_to_file(
        &proj,
        &source_buffers,
        &range_wav_path,
        &range_settings,
        None,
        None,
    )
    .expect("Selection export should succeed");
    let decoded_range = decode_file(&range_wav_path).expect("Decode selection WAV");
    assert_eq!(decoded_range.source.duration_frames, 20000);

    // 7. Test Export Cancellation
    let cancel_path = temp_dir.join("cancel_output.wav");
    let cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)); // Pre-cancelled
    let cancel_res = export_project_to_file(
        &proj,
        &source_buffers,
        &cancel_path,
        &wav_settings,
        None,
        Some(cancel_token),
    );
    assert!(cancel_res.is_err());
    assert!(
        !cancel_path.exists(),
        "Cancelled export should not leave output file"
    );

    // Clean up temporary files
    let _ = fs::remove_dir_all(&temp_dir);
}
