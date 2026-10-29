use std::collections::HashMap;
use stem_core::project::AudioSource;
use stem_core::waveform::{find_nearest_zero_crossing, WaveformDocument, WaveformHistory};
use uuid::Uuid;

fn create_test_source_and_buffer(
    sample_rate: u32,
    duration_frames: u64,
    frequency: f32,
) -> (AudioSource, Vec<Vec<f32>>) {
    let source_id = Uuid::new_v4();
    let num_frames = duration_frames as usize;
    let mut left = Vec::with_capacity(num_frames);
    let mut right = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * frequency * t).sin();
        left.push(s);
        right.push(s);
    }

    let source = AudioSource {
        id: source_id,
        path: std::path::PathBuf::from("test.wav"),
        filename: "test.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames,
        file_hash: None,
    };

    (source, vec![left, right])
}

#[test]
fn test_waveform_cut_copy_paste_across_documents() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    // Doc A: 440 Hz sine (1 second = 48000 frames)
    let (src_a, buf_a) = create_test_source_and_buffer(sample_rate, 48000, 440.0);
    buffers.insert(src_a.id, buf_a);
    let mut doc_a = WaveformDocument::from_source(&src_a);

    // Doc B: 880 Hz sine (1 second = 48000 frames)
    let (src_b, buf_b) = create_test_source_and_buffer(sample_rate, 48000, 880.0);
    buffers.insert(src_b.id, buf_b);
    let mut doc_b = WaveformDocument::from_source(&src_b);

    // 1. Cut 10000 frames from Doc A (frames 10000..20000)
    let clip = doc_a
        .cut_range(10000, 20000, &buffers)
        .expect("Cut should return clipboard");
    assert_eq!(clip.duration_frames, 10000);
    assert_eq!(doc_a.total_frames(), 38000);

    // 2. Paste cut clip into Doc B at frame 5000
    doc_b.paste_clipboard(5000, &clip, &mut buffers);
    assert_eq!(doc_b.total_frames(), 58000);

    // 3. Render Doc B and verify pasted section
    let rendered_b = doc_b.render_planar(&buffers);
    assert_eq!(rendered_b[0].len(), 58000);
    assert_eq!(rendered_b[1].len(), 58000);

    // 4. Copy from Doc B and paste back into Doc A
    let clip2 = doc_b
        .copy_range(0, 10000, &buffers)
        .expect("Copy should return clipboard");
    assert_eq!(clip2.duration_frames, 10000);
    doc_a.paste_clipboard(doc_a.total_frames(), &clip2, &mut buffers);
    assert_eq!(doc_a.total_frames(), 48000);
}

#[test]
fn test_waveform_delete_and_trim() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    let (src, buf) = create_test_source_and_buffer(sample_rate, 50000, 200.0);
    buffers.insert(src.id, buf);
    let mut doc = WaveformDocument::from_source(&src);

    // Delete 10000..20000 (10000 frames deleted)
    doc.delete_range(10000, 20000);
    assert_eq!(doc.total_frames(), 40000);

    // Trim to 5000..25000 (20000 frames kept)
    doc.trim_to_range(5000, 25000);
    assert_eq!(doc.total_frames(), 20000);

    let rendered = doc.render_planar(&buffers);
    assert_eq!(rendered[0].len(), 20000);
}

#[test]
fn test_waveform_insert_silence() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    let (src, buf) = create_test_source_and_buffer(sample_rate, 10000, 440.0);
    buffers.insert(src.id, buf);
    let mut doc = WaveformDocument::from_source(&src);

    // Insert 5000 frames of silence at frame 2000
    doc.insert_silence(2000, 5000);
    assert_eq!(doc.total_frames(), 15000);

    let rendered = doc.render_planar(&buffers);
    assert_eq!(rendered[0].len(), 15000);

    // Frames 2000..7000 must be exactly 0.0
    for ch in &rendered {
        for &s in &ch[2000..7000] {
            assert_eq!(s, 0.0);
        }
    }
}

#[test]
fn test_waveform_reverse_and_invert_phase() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    // Linearly ramped buffer [0.0, 0.1, 0.2, ... 1.0]
    let source_id = Uuid::new_v4();
    let ramp: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
    buffers.insert(source_id, vec![ramp.clone(), ramp.clone()]);

    let source = AudioSource {
        id: source_id,
        path: std::path::PathBuf::from("ramp.wav"),
        filename: "ramp.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: 100,
        file_hash: None,
    };

    let mut doc = WaveformDocument::from_source(&source);

    // Invert phase of middle range [20, 80)
    doc.invert_phase_range(20, 80);
    let rendered_inv = doc.render_planar(&buffers);
    assert!((rendered_inv[0][10] - 0.10).abs() < 1e-6);
    assert!((rendered_inv[0][50] - (-0.50)).abs() < 1e-6);

    // Reverse range [0, 100)
    let mut doc_rev = WaveformDocument::from_source(&source);
    doc_rev.reverse_range(0, 100);
    let rendered_rev = doc_rev.render_planar(&buffers);

    // First sample should now be the last sample of ramp
    assert!((rendered_rev[0][0] - 0.99).abs() < 1e-6);
    assert!((rendered_rev[0][99] - 0.00).abs() < 1e-6);
}

#[test]
fn test_find_nearest_zero_crossing() {
    // A sine wave starting at index 0 (0.0), going up, and crossing 0 again near sample 50
    let mut signal = Vec::with_capacity(100);
    for i in 0..100 {
        signal.push((2.0 * std::f32::consts::PI * i as f32 / 100.0).sin());
    }

    // Near sample 48 (which is close to sample 50, the zero crossing):
    let zc = find_nearest_zero_crossing(&signal, 48, 10);
    assert!(
        zc == 49 || zc == 50,
        "Zero crossing should be at 49 or 50, got {zc}"
    );
    assert!(signal[zc as usize].abs() < 0.1);
}

#[test]
fn test_waveform_undo_redo_history() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    let (src, buf) = create_test_source_and_buffer(sample_rate, 40000, 440.0);
    buffers.insert(src.id, buf);
    let mut doc = WaveformDocument::from_source(&src);
    let mut history = WaveformHistory::new();

    // Action 1: Delete 10000..20000
    history.push_state(doc.clone());
    doc.delete_range(10000, 20000);
    assert_eq!(doc.total_frames(), 30000);

    // Action 2: Insert silence 5000 frames
    history.push_state(doc.clone());
    doc.insert_silence(0, 5000);
    assert_eq!(doc.total_frames(), 35000);

    // Undo Action 2
    assert!(history.can_undo());
    history.undo(&mut doc);
    assert_eq!(doc.total_frames(), 30000);

    // Undo Action 1
    assert!(history.can_undo());
    history.undo(&mut doc);
    assert_eq!(doc.total_frames(), 40000);
    assert!(!history.can_undo());

    // Redo Action 1
    assert!(history.can_redo());
    history.redo(&mut doc);
    assert_eq!(doc.total_frames(), 30000);

    // Redo Action 2
    assert!(history.can_redo());
    history.redo(&mut doc);
    assert_eq!(doc.total_frames(), 35000);
    assert!(!history.can_redo());
}

#[test]
fn test_waveform_fade_curves_math() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    // Constant 1.0 signal (1000 frames)
    let source_id = Uuid::new_v4();
    let ones = vec![vec![1.0f32; 1000], vec![1.0f32; 1000]];
    buffers.insert(source_id, ones);

    let source = AudioSource {
        id: source_id,
        path: std::path::PathBuf::from("ones.wav"),
        filename: "ones.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: 1000,
        file_hash: None,
    };

    use stem_core::waveform::FadeCurve;

    // Test Linear Fade In: starts at 0.0, ends at 1.0
    let mut doc_lin = WaveformDocument::from_source(&source);
    doc_lin.apply_fade_in_range(0, 1000, FadeCurve::Linear, &mut buffers);
    let rendered_lin = doc_lin.render_planar(&buffers);
    assert!((rendered_lin[0][0] - 0.0).abs() < 1e-4);
    assert!((rendered_lin[0][500] - 0.5).abs() < 0.01);
    assert!((rendered_lin[0][999] - 1.0).abs() < 1e-4);

    // Test S-Curve Fade In: starts at 0.0, midpoint is 0.5, ends at 1.0
    let mut doc_scurve = WaveformDocument::from_source(&source);
    doc_scurve.apply_fade_in_range(0, 1000, FadeCurve::SCurve, &mut buffers);
    let rendered_scurve = doc_scurve.render_planar(&buffers);
    assert!((rendered_scurve[0][0] - 0.0).abs() < 1e-4);
    assert!((rendered_scurve[0][500] - 0.5).abs() < 0.01);
    assert!((rendered_scurve[0][999] - 1.0).abs() < 1e-4);

    // Test Logarithmic vs Exponential: Logarithmic is concave down (louder faster), Exponential is concave up (quieter longer)
    let mut doc_log = WaveformDocument::from_source(&source);
    doc_log.apply_fade_in_range(0, 1000, FadeCurve::Logarithmic, &mut buffers);
    let rendered_log = doc_log.render_planar(&buffers);

    let mut doc_exp = WaveformDocument::from_source(&source);
    doc_exp.apply_fade_in_range(0, 1000, FadeCurve::Exponential, &mut buffers);
    let rendered_exp = doc_exp.render_planar(&buffers);

    // At 25% duration, Logarithmic gain > Linear gain > Exponential gain
    let t250 = 250;
    assert!(rendered_log[0][t250] > rendered_lin[0][t250]);
    assert!(rendered_lin[0][t250] > rendered_exp[0][t250]);

    // Test Fade Out (SCurve): starts at 1.0, ends at 0.0
    let mut doc_fade_out = WaveformDocument::from_source(&source);
    doc_fade_out.apply_fade_out_range(0, 1000, FadeCurve::SCurve, &mut buffers);
    let rendered_out = doc_fade_out.render_planar(&buffers);
    assert!((rendered_out[0][0] - 1.0).abs() < 1e-4);
    assert!((rendered_out[0][500] - 0.5).abs() < 0.01);
    assert!((rendered_out[0][999] - 0.0).abs() < 1e-4);
}

#[test]
fn test_waveform_peak_normalize() {
    let sample_rate = 48000;
    let mut buffers = HashMap::new();

    // Signal with peak at 0.5
    let source_id = Uuid::new_v4();
    let half_signal = vec![vec![0.5f32; 1000], vec![0.25f32; 1000]];
    buffers.insert(source_id, half_signal);

    let source = AudioSource {
        id: source_id,
        path: std::path::PathBuf::from("half.wav"),
        filename: "half.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: 1000,
        file_hash: None,
    };

    let mut doc = WaveformDocument::from_source(&source);

    // Normalize to 0.0 dBFS (peak = 1.0)
    doc.normalize_range(0, 1000, 0.0, &mut buffers);
    let rendered = doc.render_planar(&buffers);
    let max_peak_l = rendered[0].iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    let max_peak_r = rendered[1].iter().fold(0.0f32, |m, &s| m.max(s.abs()));

    assert!((max_peak_l - 1.0).abs() < 1e-4);
    assert!((max_peak_r - 0.5).abs() < 1e-4); // Relative channel balance preserved!

    // Normalize to -6.0 dBFS (approx 0.501)
    doc.normalize_range(0, 1000, -6.0, &mut buffers);
    let rendered_m6 = doc.render_planar(&buffers);
    let max_peak_m6 = rendered_m6[0].iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    let expected = 10.0f32.powf(-6.0 / 20.0);
    assert!((max_peak_m6 - expected).abs() < 1e-3);
}
