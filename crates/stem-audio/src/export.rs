//! Offline audio export / bounce engine
//!
//! Non-destructively mixes all project tracks and clips into a high-quality master file.
//! Supports WAV (16/24/32-bit float) and FLAC with TPDF dithering and loudness normalization.

use crossbeam_channel::Sender;
use flacenc::bitsink::MemSink;
use flacenc::error::Verify;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

use stem_core::project::Project;
use stem_dsp::dither::TpdfDither;
use stem_dsp::resample_audio;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("I/O error during export: {0}")]
    Io(#[from] std::io::Error),

    #[error("WAV encoder error: {0}")]
    Hound(#[from] hound::Error),

    #[error("FLAC encoder error: {0}")]
    Flac(String),

    #[error("Resampling error: {0}")]
    Resample(#[from] stem_dsp::resample::ResampleError),

    #[error("Audio analysis error: {0}")]
    Analysis(#[from] stem_dsp::analysis::AnalysisError),

    #[error("Empty timeline - nothing to export")]
    EmptyTimeline,

    #[error("Export cancelled by user")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    Wav,
    Flac,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportRange {
    EntireProject,
    TimeRange { start_frame: u64, end_frame: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BitDepth {
    Bit16,
    Bit24,
    Float32,
}

#[derive(Debug, Clone)]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub sample_rate: u32,
    pub bit_depth: BitDepth,
    pub apply_dither: bool,
    pub normalize_target_lufs: Option<f32>,
    pub range: ExportRange,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Wav,
            sample_rate: 48000,
            bit_depth: BitDepth::Bit24,
            apply_dither: true,
            normalize_target_lufs: None,
            range: ExportRange::EntireProject,
        }
    }
}

/// Renders project audio offline into a stereo mixdown buffer [Left, Right].
pub fn render_project_mixdown(
    project: &Project,
    source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    progress_tx: Option<&Sender<f32>>,
) -> Result<Vec<Vec<f32>>, ExportError> {
    render_project_mixdown_range(
        project,
        source_buffers,
        ExportRange::EntireProject,
        progress_tx,
        None,
    )
}

/// Renders project audio offline for a specified ExportRange with optional cancellation token.
pub fn render_project_mixdown_range(
    project: &Project,
    source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    range: ExportRange,
    progress_tx: Option<&Sender<f32>>,
    cancellation_token: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<Vec<f32>>, ExportError> {
    let (export_start, export_end) = match range {
        ExportRange::EntireProject => (0, project.total_frames()),
        ExportRange::TimeRange {
            start_frame,
            end_frame,
        } => (start_frame, end_frame.max(start_frame)),
    };

    let total_frames = export_end.saturating_sub(export_start);
    if total_frames == 0 {
        return Err(ExportError::EmptyTimeline);
    }

    let mut master_left = vec![0.0f32; total_frames as usize];
    let mut master_right = vec![0.0f32; total_frames as usize];

    let has_solos = project.tracks.iter().any(|t| t.soloed);

    let num_tracks = project.tracks.len();
    for (t_idx, track) in project.tracks.iter().enumerate() {
        if let Some(token) = cancellation_token {
            if token.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ExportError::Cancelled);
            }
        }
        if let Some(tx) = progress_tx {
            let _ = tx.send((t_idx as f32) / (num_tracks as f32 * 2.0));
        }

        // Mute / Solo logic
        if track.muted || (has_solos && !track.soloed) {
            continue;
        }

        let track_gain = 10.0f32.powf(track.gain_db / 20.0);
        let pan = track.pan.clamp(-1.0, 1.0);
        // Constant power panning law
        let pan_rad = (pan + 1.0) * (std::f32::consts::PI / 4.0);
        let gain_left = track_gain * pan_rad.cos();
        let gain_right = track_gain * pan_rad.sin();

        for clip in &track.clips {
            let src_data = match source_buffers.get(&clip.source_id) {
                Some(buf) if !buf.is_empty() => buf,
                _ => continue,
            };

            let src_left = &src_data[0];
            let src_right = if src_data.len() > 1 {
                &src_data[1]
            } else {
                &src_data[0]
            };

            let clip_gain = 10.0f32.powf(clip.gain_db / 20.0);
            let clip_len = clip.length_frames as usize;
            let src_start = clip.source_start_frame as usize;
            let timeline_start = clip.track_offset_frames;
            let timeline_end = timeline_start + clip.length_frames;

            // Check if clip intersects [export_start, export_end)
            if timeline_end <= export_start || timeline_start >= export_end {
                continue;
            }

            for i in 0..clip_len {
                let t_frame = timeline_start + i as u64;
                if t_frame < export_start {
                    continue;
                }
                if t_frame >= export_end {
                    break;
                }

                let s_idx = src_start + i;
                let out_idx = (t_frame - export_start) as usize;

                let in_l = src_left.get(s_idx).copied().unwrap_or(0.0);
                let in_r = src_right.get(s_idx).copied().unwrap_or(0.0);

                let fade_gain = clip.gain_at_relative_frame(i as u64);
                let net_gain = clip_gain * fade_gain;

                master_left[out_idx] += in_l * net_gain * gain_left;
                master_right[out_idx] += in_r * net_gain * gain_right;
            }
        }
    }

    // Apply master track gain
    let master_gain_linear = 10.0f32.powf(project.master_gain_db / 20.0);
    for s in master_left.iter_mut() {
        *s *= master_gain_linear;
    }
    for s in master_right.iter_mut() {
        *s *= master_gain_linear;
    }

    Ok(vec![master_left, master_right])
}

/// Renders planar audio buffers for each individual track without track fader/pan.
/// This allows real-time per-track insert effects and mixing in the audio thread callback.
pub fn render_project_tracks(
    project: &Project,
    source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
) -> Vec<Vec<Vec<f32>>> {
    let total_frames = project.total_frames() as usize;
    if total_frames == 0 {
        return Vec::new();
    }

    let mut track_buffers = Vec::with_capacity(project.tracks.len());

    for track in &project.tracks {
        let mut track_left = vec![0.0f32; total_frames];
        let mut track_right = vec![0.0f32; total_frames];

        for clip in &track.clips {
            let src_data = match source_buffers.get(&clip.source_id) {
                Some(buf) if !buf.is_empty() => buf,
                _ => continue,
            };

            let src_left = &src_data[0];
            let src_right = if src_data.len() > 1 {
                &src_data[1]
            } else {
                &src_data[0]
            };

            let clip_gain = 10.0f32.powf(clip.gain_db / 20.0);
            let clip_len = clip.length_frames as usize;
            let src_start = clip.source_start_frame as usize;
            let timeline_start = clip.track_offset_frames as usize;

            for i in 0..clip_len {
                let out_idx = timeline_start + i;
                if out_idx >= total_frames {
                    break;
                }
                let s_idx = src_start + i;
                let in_l = src_left.get(s_idx).copied().unwrap_or(0.0);
                let in_r = src_right.get(s_idx).copied().unwrap_or(0.0);

                let fade_gain = clip.gain_at_relative_frame(i as u64);
                let net_gain = clip_gain * fade_gain;

                track_left[out_idx] += in_l * net_gain;
                track_right[out_idx] += in_r * net_gain;
            }
        }

        track_buffers.push(vec![track_left, track_right]);
    }

    track_buffers
}

/// Offline export of the project to a file with all options applied.
pub fn export_project_to_file<P: AsRef<Path>>(
    project: &Project,
    source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    destination_path: P,
    settings: &ExportSettings,
    progress_tx: Option<Sender<f32>>,
    cancellation_token: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<(), ExportError> {
    let dest = destination_path.as_ref();

    let check_cancelled = || -> Result<(), ExportError> {
        if let Some(ref token) = cancellation_token {
            if token.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ExportError::Cancelled);
            }
        }
        Ok(())
    };

    check_cancelled()?;

    // 1. Render mixdown
    let mut channels = render_project_mixdown_range(
        project,
        source_buffers,
        settings.range,
        progress_tx.as_ref(),
        cancellation_token.as_deref(),
    )?;

    check_cancelled()?;

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(0.5);
    }

    // 2. Resample if necessary
    if settings.sample_rate != project.sample_rate {
        channels = resample_audio(&channels, project.sample_rate, settings.sample_rate)?;
    }

    check_cancelled()?;

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(0.7);
    }

    // 3. Optional Loudness Normalization
    if let Some(target_lufs) = settings.normalize_target_lufs {
        let mut interleaved = Vec::with_capacity(channels[0].len() * 2);
        for (&l, &r) in channels[0].iter().zip(channels[1].iter()) {
            interleaved.push(l);
            interleaved.push(r);
        }

        let current_loudness = stem_dsp::analyze_loudness(&interleaved, 2, settings.sample_rate)?;
        let gain_db = target_lufs - current_loudness.integrated_lufs;
        let gain_linear = 10.0f32.powf(gain_db / 20.0);

        for ch in channels.iter_mut() {
            for sample in ch.iter_mut() {
                *sample *= gain_linear;
            }
        }
    }

    check_cancelled()?;

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(0.85);
    }

    // 4. Encode to destination format
    let res = match settings.format {
        ExportFormat::Wav => write_wav(dest, &channels, settings),
        ExportFormat::Flac => write_flac(dest, &channels, settings),
    };

    if let Err(e) = res {
        let _ = std::fs::remove_file(dest);
        return Err(e);
    }

    if let Some(ref tx) = progress_tx {
        let _ = tx.send(1.0);
    }

    Ok(())
}

fn write_wav(
    path: &Path,
    channels: &[Vec<f32>],
    settings: &ExportSettings,
) -> Result<(), ExportError> {
    let (sample_format, bits_per_sample) = match settings.bit_depth {
        BitDepth::Bit16 => (hound::SampleFormat::Int, 16),
        BitDepth::Bit24 => (hound::SampleFormat::Int, 24),
        BitDepth::Float32 => (hound::SampleFormat::Float, 32),
    };

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: settings.sample_rate,
        bits_per_sample,
        sample_format,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    let mut dither = TpdfDither::new();

    match settings.bit_depth {
        BitDepth::Bit16 => {
            for (&l, &r) in channels[0].iter().zip(channels[1].iter()) {
                let l_i16 = if settings.apply_dither {
                    dither.quantize_i16(l)
                } else {
                    (l * 32767.0).round().clamp(-32768.0, 32767.0) as i16
                };
                let r_i16 = if settings.apply_dither {
                    dither.quantize_i16(r)
                } else {
                    (r * 32767.0).round().clamp(-32768.0, 32767.0) as i16
                };
                writer.write_sample(l_i16)?;
                writer.write_sample(r_i16)?;
            }
        }
        BitDepth::Bit24 => {
            for (&l, &r) in channels[0].iter().zip(channels[1].iter()) {
                let l_i24 = if settings.apply_dither {
                    dither.quantize_i24(l)
                } else {
                    (l * 8388607.0).round().clamp(-8388608.0, 8388607.0) as i32
                };
                let r_i24 = if settings.apply_dither {
                    dither.quantize_i24(r)
                } else {
                    (r * 8388607.0).round().clamp(-8388608.0, 8388607.0) as i32
                };
                writer.write_sample(l_i24)?;
                writer.write_sample(r_i24)?;
            }
        }
        BitDepth::Float32 => {
            for (&l, &r) in channels[0].iter().zip(channels[1].iter()) {
                writer.write_sample(l)?;
                writer.write_sample(r)?;
            }
        }
    }

    writer.finalize()?;
    Ok(())
}

fn write_flac(
    path: &Path,
    channels: &[Vec<f32>],
    settings: &ExportSettings,
) -> Result<(), ExportError> {
    let bits_per_sample = match settings.bit_depth {
        BitDepth::Bit16 => 16,
        _ => 24,
    };

    let mut dither = TpdfDither::new();
    let num_frames = channels[0].len();

    // Prepare interleaved PCM i32 samples for FLAC encoder
    let mut pcm_samples = Vec::with_capacity(num_frames * 2);
    for (&l, &r) in channels[0].iter().zip(channels[1].iter()) {
        if bits_per_sample == 16 {
            let l_val = if settings.apply_dither {
                dither.quantize_i16(l) as i32
            } else {
                (l * 32767.0).round().clamp(-32768.0, 32767.0) as i32
            };
            let r_val = if settings.apply_dither {
                dither.quantize_i16(r) as i32
            } else {
                (r * 32767.0).round().clamp(-32768.0, 32767.0) as i32
            };
            pcm_samples.push(l_val);
            pcm_samples.push(r_val);
        } else {
            let l_val = if settings.apply_dither {
                dither.quantize_i24(l)
            } else {
                (l * 8388607.0).round().clamp(-8388608.0, 8388607.0) as i32
            };
            let r_val = if settings.apply_dither {
                dither.quantize_i24(r)
            } else {
                (r * 8388607.0).round().clamp(-8388608.0, 8388607.0) as i32
            };
            pcm_samples.push(l_val);
            pcm_samples.push(r_val);
        }
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| ExportError::Flac(format!("Invalid FLAC encoder config: {e:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        &pcm_samples,
        2,
        bits_per_sample,
        settings.sample_rate as usize,
    );

    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, 4096)
        .map_err(|e| ExportError::Flac(format!("FLAC encoding failed: {e:?}")))?;

    let mut sink = MemSink::<u8>::new();
    flacenc::component::BitRepr::write(&flac_stream, &mut sink)
        .map_err(|e| ExportError::Flac(format!("FLAC bit sink write failed: {e:?}")))?;

    let mut file = File::create(path)?;
    file.write_all(sink.as_slice())?;

    Ok(())
}
