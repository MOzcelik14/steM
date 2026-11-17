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
        ..ExportSettings::default()
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
        ..ExportSettings::default()
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

    // 5b. Test MP3 export & decode round-trip
    let mp3_path = temp_dir.join("test_output.mp3");
    let mp3_settings = ExportSettings {
        format: ExportFormat::Mp3,
        sample_rate,
        mp3_bitrate_kbps: 192,
        ..ExportSettings::default()
    };
    export_project_to_file(&proj, &source_buffers, &mp3_path, &mp3_settings, None, None)
        .expect("MP3 export should succeed");
    assert!(mp3_path.exists());
    let decoded_mp3 = decode_file(&mp3_path).expect("Decode exported MP3");
    assert_eq!(decoded_mp3.source.sample_rate, sample_rate);
    assert_eq!(decoded_mp3.channels.len(), 2);

    // 5c. Test OGG Vorbis export & decode round-trip
    let ogg_path = temp_dir.join("test_output.ogg");
    let ogg_settings = ExportSettings {
        format: ExportFormat::Ogg,
        sample_rate,
        ogg_quality: 0.6,
        ..ExportSettings::default()
    };
    export_project_to_file(&proj, &source_buffers, &ogg_path, &ogg_settings, None, None)
        .expect("OGG export should succeed");
    assert!(ogg_path.exists());
    let decoded_ogg = decode_file(&ogg_path).expect("Decode exported OGG");
    assert_eq!(decoded_ogg.source.sample_rate, sample_rate);
    assert_eq!(decoded_ogg.channels.len(), 2);

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
        ..ExportSettings::default()
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

#[test]
fn test_batch_export_filename_token_formatting() {
    use stem_audio::format_export_filename;

    let template = "{project}_{track}_{index}_{marker}";
    let result = format_export_filename(template, "MySong", "Vocals", 0, "Chorus");
    assert_eq!(result, "MySong_Vocals_01_Chorus");

    let template2 = "Stem_{index}_{track}";
    let result2 = format_export_filename(template2, "Album", "Drums", 4, "");
    assert_eq!(result2, "Stem_05_Drums");
}

#[test]
fn test_offline_export_applies_track_bus_and_master_effects_parity() {
    use stem_audio::{render_project_mixdown_range, render_track_stem, ExportRange};
    use stem_core::project::SubmixBus;
    use stem_core::{AudioSource, Clip, EffectConfig, Project};

    let sr = 48000;
    let num_frames = 4800;
    let mut proj = Project::new("FX Parity", sr);
    proj.tracks.clear();

    let src_id = Uuid::new_v4();
    proj.sources.insert(
        src_id,
        AudioSource {
            id: src_id,
            path: std::path::PathBuf::from("tone.wav"),
            filename: "tone.wav".to_string(),
            sample_rate: sr,
            channels: 2,
            duration_frames: num_frames as u64,
            file_hash: None,
        },
    );

    let mut tone = Vec::with_capacity(num_frames);
    for i in 0..num_frames {
        let t = i as f32 / sr as f32;
        tone.push((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.9);
    }
    let mut source_buffers = HashMap::new();
    source_buffers.insert(src_id, vec![tone.clone(), tone]);

    // Track 0 with a -12 dB Limiter routed to a SubmixBus
    let mut bus = SubmixBus::new("Vocal Bus");
    bus.effects.push(EffectConfig::Limiter {
        threshold_db: -18.0,
        release_ms: 50.0,
    });
    let bus_id = bus.id;
    proj.busses.push(bus);

    let t_id = proj.add_track("Lead");
    let t = proj.get_track_mut(t_id).unwrap();
    t.output_routing = stem_core::project::TrackOutputRouting::Bus(bus_id);
    t.effects.push(EffectConfig::Limiter {
        threshold_db: -12.0,
        release_ms: 50.0,
    });
    t.clips
        .push(Clip::new("Tone", src_id, 0, num_frames as u64, 0));

    // Master limiter at -24 dB
    proj.master_effects.push(EffectConfig::Limiter {
        threshold_db: -24.0,
        release_ms: 50.0,
    });

    let mix = render_project_mixdown_range(
        &proj,
        &source_buffers,
        ExportRange::EntireProject,
        None,
        None,
    )
    .expect("mixdown should succeed");
    let peak = mix[0]
        .iter()
        .chain(mix[1].iter())
        .fold(0.0f32, |m, &s| m.max(s.abs()));
    // -24 dBFS ≈ 0.0631
    assert!(
        peak < 0.08,
        "Master limiter (-24 dBFS) should limit mixdown peak below 0.08, got {peak}"
    );

    let stem = render_track_stem(&proj, &source_buffers, 0, ExportRange::EntireProject)
        .expect("stem render should succeed");
    let stem_peak = stem[0]
        .iter()
        .chain(stem[1].iter())
        .fold(0.0f32, |m, &s| m.max(s.abs()));
    // Track limiter (-12 dBFS ≈ 0.2512) with 0 dB pan (-3 dB center = 0.707)
    assert!(
        stem_peak < 0.22,
        "Track limiter (-12 dBFS) should limit stem peak below 0.22, got {stem_peak}"
    );
}

#[test]
fn test_track_capacity_limit_consistency() {
    use stem_audio::render_project_tracks;
    use stem_core::{Project, MAX_PROJECT_TRACKS};

    let mut proj = Project::new("Capacity Test", 48000);
    while proj.tracks.len() < MAX_PROJECT_TRACKS {
        proj.add_track(format!("T{}", proj.tracks.len() + 1));
    }
    assert!(
        proj.validate().is_ok(),
        "Project at MAX_PROJECT_TRACKS should validate"
    );

    let rendered = render_project_tracks(&proj, &HashMap::new());
    assert_eq!(rendered.len(), 0); // empty timeline returns 0 frames cleanly

    proj.add_track("Overflow");
    assert!(
        proj.validate().is_err(),
        "Project exceeding MAX_PROJECT_TRACKS should fail validation"
    );
}

#[test]
fn test_track_freeze_and_surround_51_export() {
    use stem_audio::{export_surround_51_wav, freeze_track_to_wav};
    use stem_core::project::{Clip, Project};

    let sr = 48000;
    let mut proj = Project::new("Surround & Freeze Test", sr);
    let src_id = uuid::Uuid::new_v4();
    let mut source_buffers = HashMap::new();
    let len = 4800usize;
    source_buffers.insert(src_id, vec![vec![0.5f32; len], vec![0.3f32; len]]);

    proj.tracks[0]
        .clips
        .push(Clip::new("tone", src_id, 0, len as u64, 0));

    let tmp_dir = std::env::temp_dir();
    let freeze_path = tmp_dir.join("stem_test_freeze.wav");
    freeze_track_to_wav(&proj, &source_buffers, 0, &freeze_path)
        .expect("freeze_track_to_wav should succeed");
    let freeze_reader = hound::WavReader::open(&freeze_path).expect("freeze wav readable");
    assert_eq!(freeze_reader.spec().channels, 2);
    let _ = std::fs::remove_file(&freeze_path);

    let surround_path = tmp_dir.join("stem_test_51.wav");
    export_surround_51_wav(&proj, &source_buffers, &surround_path)
        .expect("export_surround_51_wav should succeed");
    let s51_reader = hound::WavReader::open(&surround_path).expect("5.1 wav readable");
    assert_eq!(s51_reader.spec().channels, 6);
    assert_eq!(s51_reader.len() as usize, len * 6);
    let _ = std::fs::remove_file(&surround_path);
}

#[test]
fn test_export_sample_rates_and_bit_depths() {
    use stem_audio::{
        decode_file, export_project_to_file, BitDepth, ExportFormat, ExportRange, ExportSettings,
    };
    use stem_core::project::{Clip, Project};

    let sample_rates = [44100u32, 48000, 96000];
    let bit_depths = [BitDepth::Bit16, BitDepth::Bit24, BitDepth::Float32];
    let tmp_dir = std::env::temp_dir();

    for &sr in &sample_rates {
        let mut proj = Project::new("Rate Test", sr);
        let src_id = uuid::Uuid::new_v4();
        let len = (sr / 10) as usize; // 100ms
        let tone: Vec<f32> = (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr as f32)).sin() * 0.4)
            .collect();
        let mut source_buffers = HashMap::new();
        source_buffers.insert(src_id, vec![tone.clone(), tone]);
        proj.tracks[0]
            .clips
            .push(Clip::new("tone", src_id, 0, len as u64, 0));

        for &bd in &bit_depths {
            let dest = tmp_dir.join(format!("stem_qa_{sr}_{bd:?}.wav"));
            let settings = ExportSettings {
                format: ExportFormat::Wav,
                sample_rate: sr,
                bit_depth: bd,
                range: ExportRange::EntireProject,
                ..Default::default()
            };
            export_project_to_file(&proj, &source_buffers, &dest, &settings, None, None)
                .expect("export should succeed");
            let decoded = decode_file(&dest).expect("decoded wav should succeed");
            assert_eq!(decoded.source.sample_rate, sr);
            assert_eq!(decoded.channels.len(), 2);
            let _ = std::fs::remove_file(&dest);
        }
    }
}

#[test]
fn test_long_duration_sparse_project_rendering() {
    use stem_audio::{render_project_mixdown_range, ExportRange};
    use stem_core::project::{Clip, Project};

    let sr = 48000;
    let mut proj = Project::new("Long Duration", sr);
    let src_id = uuid::Uuid::new_v4();
    let len = 4800usize;
    let mut source_buffers = HashMap::new();
    source_buffers.insert(src_id, vec![vec![0.25f32; len], vec![0.25f32; len]]);

    // Place clip at 10 minutes (28,800,000 frames) and render only the 10-min window
    let ten_min_frame = 48000u64 * 600;
    proj.tracks[0]
        .clips
        .push(Clip::new("late_clip", src_id, 0, len as u64, ten_min_frame));

    let mix = render_project_mixdown_range(
        &proj,
        &source_buffers,
        ExportRange::TimeRange {
            start_frame: ten_min_frame,
            end_frame: ten_min_frame + len as u64,
        },
        None,
        None,
    )
    .expect("sparse range render should succeed");
    assert_eq!(mix[0].len(), len);
    assert!(mix[0].iter().any(|&s| s.abs() > 0.1));
}

#[test]
fn test_audio_callback_stability_under_sustained_load() {
    use rtrb::RingBuffer;
    use std::sync::Arc;
    use stem_audio::callback::{
        AudioCommand, AudioProcessor, AudioTelemetry, RealtimeAudioCallback, SharedAudioTimeline,
    };

    let (mut cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(256);
    let (telem_tx, mut telem_rx) = RingBuffer::<AudioTelemetry>::new(256);
    let (_mon_tx, mon_rx) = RingBuffer::<f32>::new(1024);
    let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(16);
    let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(16);

    let processor = AudioProcessor::new(cmd_rx, telem_tx, Some(mon_rx));
    let mut callback = RealtimeAudioCallback::new(processor, timeline_rx, retired_tx);

    let frames = 48000usize;
    let timeline = Arc::new(SharedAudioTimeline {
        channels: vec![vec![0.5f32; frames], vec![0.5f32; frames]],
        track_channels: vec![vec![vec![0.25f32; frames], vec![0.25f32; frames]]],
        total_frames: frames as u64,
    });
    timeline_tx.send(timeline).unwrap();
    cmd_tx.push(AudioCommand::Play).unwrap();

    let mut out_buffer = vec![0.0f32; 512];
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        callback.process(&mut out_buffer);
        assert!(out_buffer.iter().all(|s| s.is_finite()));
        while telem_rx.pop().is_ok() {}
        while retired_rx.try_recv().is_ok() {}
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs_f32() < 2.0,
        "1000 audio callbacks took too long: {elapsed:?}"
    );
}
