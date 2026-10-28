//! Built-in synthetic demo project generator
//!
//! Generates a multi-track musical session with synthesized drums, bassline, and lead arpeggios
//! directly in memory without requiring external audio assets.

use crate::project::{AudioSource, Clip, Project};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Generates a complete 4-bar 120 BPM demo session with 3 tracks and synthetic PCM audio.
pub fn generate_demo_project(sample_rate: u32) -> (Project, HashMap<Uuid, Vec<Vec<f32>>>) {
    let mut project = Project::new("Demo Session - Electronic Groove", sample_rate);
    project.tempo_bpm = 120.0;
    project.time_signature = (4, 4);

    let mut buffers = HashMap::new();

    // 4 bars at 120 BPM in 4/4 = 8.0 seconds of audio
    let duration_seconds = 8.0;
    let total_frames = (duration_seconds * sample_rate as f64).round() as usize;

    // ---------------- 1. Drums Track (Kick & Snare) ----------------
    let drums_src_id = Uuid::new_v4();
    let mut drums_l = vec![0.0f32; total_frames];
    let mut drums_r = vec![0.0f32; total_frames];

    let beat_frames = (sample_rate as f64 * 60.0 / 120.0).round() as usize;
    let total_beats = 16; // 4 bars * 4 beats

    for beat in 0..total_beats {
        let beat_start = beat * beat_frames;

        // Kick on every beat
        let kick_len = (sample_rate as f32 * 0.22) as usize;
        for i in 0..kick_len {
            if beat_start + i < total_frames {
                let t = i as f32 / sample_rate as f32;
                // Pitch drop from 150 Hz to 45 Hz
                let freq = 45.0 + 105.0 * (-t * 30.0).exp();
                let env = (-t * 18.0).exp();
                let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.8;
                drums_l[beat_start + i] += sample;
                drums_r[beat_start + i] += sample;
            }
        }

        // Snare on beats 2 and 4 (1-indexed: beats 1, 3 in 0-indexed per bar)
        if beat % 2 == 1 {
            let snare_len = (sample_rate as f32 * 0.18) as usize;
            for i in 0..snare_len {
                if beat_start + i < total_frames {
                    let t = i as f32 / sample_rate as f32;
                    let tone =
                        (2.0 * std::f32::consts::PI * 220.0 * t).sin() * (-t * 25.0).exp() * 0.4;
                    // Pseudo-random noise for snare snap
                    let noise = (((i * 1103515245 + 12345) % 65536) as f32 / 32768.0 - 1.0)
                        * (-t * 35.0).exp()
                        * 0.5;
                    let sample = tone + noise;
                    drums_l[beat_start + i] += sample;
                    drums_r[beat_start + i] += sample;
                }
            }
        }
    }

    let drums_source = AudioSource {
        id: drums_src_id,
        path: PathBuf::from("demo_drums.wav"),
        filename: "Drums (Kick & Snare).wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: total_frames as u64,
        file_hash: None,
    };
    project.add_source(drums_source);
    buffers.insert(drums_src_id, vec![drums_l, drums_r]);

    // ---------------- 2. Bassline Track ----------------
    let bass_src_id = Uuid::new_v4();
    let mut bass_l = vec![0.0f32; total_frames];
    let mut bass_r = vec![0.0f32; total_frames];

    // Bass notes (A1=55Hz, C2=65.4Hz, D2=73.4Hz, E2=82.4Hz)
    let bass_notes = [55.0, 55.0, 65.41, 73.42, 55.0, 55.0, 82.41, 73.42];
    let note_len = beat_frames / 2; // Eighth notes

    for (idx, &freq) in bass_notes.iter().cycle().take(32).enumerate() {
        let note_start = idx * note_len;
        let samples = (note_len as f32 * 0.85) as usize;

        for i in 0..samples {
            if note_start + i < total_frames {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 6.0).exp();
                let fundamental = (2.0 * std::f32::consts::PI * freq * t).sin();
                let harmonic = (2.0 * std::f32::consts::PI * freq * 2.0 * t).sin() * 0.4;
                let sample = (fundamental + harmonic) * env * 0.65;
                bass_l[note_start + i] += sample;
                bass_r[note_start + i] += sample;
            }
        }
    }

    let bass_source = AudioSource {
        id: bass_src_id,
        path: PathBuf::from("demo_bass.wav"),
        filename: "Bassline (A Minor).wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: total_frames as u64,
        file_hash: None,
    };
    project.add_source(bass_source);
    buffers.insert(bass_src_id, vec![bass_l, bass_r]);

    // ---------------- 3. Melody Arpeggio Track ----------------
    let lead_src_id = Uuid::new_v4();
    let mut lead_l = vec![0.0f32; total_frames];
    let mut lead_r = vec![0.0f32; total_frames];

    // Arpeggio notes (A4=440Hz, C5=523.25Hz, E5=659.25Hz, G5=783.99Hz)
    let lead_notes = [440.0, 523.25, 659.25, 783.99, 659.25, 523.25, 440.0, 329.63];
    let arp_note_len = beat_frames / 4; // 16th notes

    for (idx, &freq) in lead_notes.iter().cycle().take(64).enumerate() {
        let note_start = idx * arp_note_len;
        let samples = (arp_note_len as f32 * 0.75) as usize;
        let pan = (idx as f32 * 0.3).sin() * 0.4; // Stereo panning swirl

        for i in 0..samples {
            if note_start + i < total_frames {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 12.0).exp();
                let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.35;
                lead_l[note_start + i] += sample * (1.0 - pan);
                lead_r[note_start + i] += sample * (1.0 + pan);
            }
        }
    }

    let lead_source = AudioSource {
        id: lead_src_id,
        path: PathBuf::from("demo_lead.wav"),
        filename: "Synth Arpeggio Lead.wav".to_string(),
        sample_rate,
        channels: 2,
        duration_frames: total_frames as u64,
        file_hash: None,
    };
    project.add_source(lead_source);
    buffers.insert(lead_src_id, vec![lead_l, lead_r]);

    // ---------------- Setup Project Tracks & Clips ----------------
    project.tracks.clear();

    // Track 1: Drums
    let mut track_drums = crate::project::Track::new("1. Drums");
    track_drums.gain_db = -1.5;
    track_drums.pan = 0.0;
    let clip_drums = Clip::new("Drums Pattern", drums_src_id, 0, total_frames as u64, 0);
    track_drums.clips.push(clip_drums);
    project.tracks.push(track_drums);

    // Track 2: Bass
    let mut track_bass = crate::project::Track::new("2. Bassline");
    track_bass.gain_db = -2.0;
    track_bass.pan = 0.0;
    let clip_bass = Clip::new("Sub Bass Groove", bass_src_id, 0, total_frames as u64, 0);
    track_bass.clips.push(clip_bass);
    project.tracks.push(track_bass);

    // Track 3: Lead
    let mut track_lead = crate::project::Track::new("3. Synth Lead");
    track_lead.gain_db = -3.5;
    track_lead.pan = 0.15;
    let mut clip_lead = Clip::new("Arp Melody", lead_src_id, 0, total_frames as u64, 0);
    clip_lead.fade_in_frames = sample_rate as u64 / 2; // 0.5s fade in
    clip_lead.fade_out_frames = sample_rate as u64; // 1.0s fade out
    track_lead.clips.push(clip_lead);
    project.tracks.push(track_lead);

    (project, buffers)
}
