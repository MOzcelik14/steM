//! Audio analysis: EBU R128 loudness metering and FFT spectral analysis

use realfft::RealFftPlanner;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("EBU R128 error: {0:?}")]
    Ebu(#[from] ebur128::Error),
}

/// Loudness measurement results adhering to EBU R128 / ITU-R BS.1770-4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessResult {
    /// Integrated loudness across the entire analyzed audio in LUFS.
    pub integrated_lufs: f32,
    /// Maximum momentary loudness (400ms window) in LUFS.
    pub momentary_max_lufs: f32,
    /// Maximum short-term loudness (3s window) in LUFS.
    pub short_term_max_lufs: f32,
    /// Loudness Range (LRA) in LU.
    pub loudness_range_lu: f32,
}

/// Computes EBU R128 loudness on interleaved float samples.
pub fn analyze_loudness(
    interleaved_samples: &[f32],
    channels: u32,
    sample_rate: u32,
) -> Result<LoudnessResult, AnalysisError> {
    let mode = ebur128::Mode::I | ebur128::Mode::M | ebur128::Mode::S | ebur128::Mode::LRA;
    let mut ebu = ebur128::EbuR128::new(channels, sample_rate, mode)?;

    let chunk_frames = (sample_rate / 10).max(1) as usize;
    let chunk_samples = chunk_frames * channels.max(1) as usize;
    let mut momentary_max = f32::NEG_INFINITY;
    let mut short_term_max = f32::NEG_INFINITY;

    for chunk in interleaved_samples.chunks(chunk_samples) {
        ebu.add_frames_f32(chunk)?;
        if let Ok(m) = ebu.loudness_momentary() {
            if m.is_finite() {
                momentary_max = momentary_max.max(m as f32);
            }
        }
        if let Ok(s) = ebu.loudness_shortterm() {
            if s.is_finite() {
                short_term_max = short_term_max.max(s as f32);
            }
        }
    }

    let integrated = ebu.loudness_global().unwrap_or(-70.0) as f32;
    let momentary = if momentary_max.is_finite() {
        momentary_max
    } else {
        ebu.loudness_momentary().unwrap_or(-70.0) as f32
    };
    let short_term = if short_term_max.is_finite() {
        short_term_max
    } else {
        ebu.loudness_shortterm().unwrap_or(-70.0) as f32
    };
    let lra = ebu.loudness_range().unwrap_or(0.0) as f32;

    Ok(LoudnessResult {
        integrated_lufs: integrated,
        momentary_max_lufs: momentary,
        short_term_max_lufs: short_term,
        loudness_range_lu: lra,
    })
}

/// FFT Spectral analyzer for real-time and offline spectrogram visualization.
pub struct SpectrumAnalyzer {
    fft_size: usize,
    window: Vec<f32>,
    planner: RealFftPlanner<f32>,
}

impl SpectrumAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        assert!(fft_size.is_power_of_two());

        // Precompute Hann window to minimize spectral leakage
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos())
            })
            .collect();

        Self {
            fft_size,
            window,
            planner: RealFftPlanner::new(),
        }
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn set_fft_size(&mut self, fft_size: usize) {
        assert!(fft_size.is_power_of_two());
        self.fft_size = fft_size;
        self.window = (0..fft_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos())
            })
            .collect();
    }

    /// Compute magnitude spectrum in dB for a slice of audio of length `fft_size`.
    /// Returns vector of magnitudes of length `fft_size / 2 + 1`.
    pub fn compute_magnitude_db(&mut self, audio: &[f32]) -> Vec<f32> {
        let r2c = self.planner.plan_fft_forward(self.fft_size);
        let mut input = vec![0.0; self.fft_size];

        let copy_len = audio.len().min(self.fft_size);
        for i in 0..copy_len {
            input[i] = audio[i] * self.window[i];
        }

        let mut output = r2c.make_output_vec();
        r2c.process(&mut input, &mut output).unwrap_or_default();

        let norm = 1.0 / self.fft_size as f32;
        output
            .iter()
            .map(|c| {
                let mag = (c.norm() * norm).max(1e-6);
                20.0 * mag.log10()
            })
            .collect()
    }
}

/// Computes estimated True Peak in dBFS with 4x oversampled interpolation.
pub fn compute_true_peak_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -90.0;
    }

    let mut max_abs = 0.0f32;
    // 4x cubic Hermite oversampling around peak candidates
    for i in 0..samples.len() {
        let s = samples[i].abs();
        if s > max_abs {
            max_abs = s;
        }

        // Interpolate between samples for inter-sample peak detection
        if i >= 1 && i + 2 < samples.len() {
            let p0 = samples[i - 1];
            let p1 = samples[i];
            let p2 = samples[i + 1];
            let p3 = samples[i + 2];

            let m1 = 0.5 * (p2 - p0);
            let m2 = 0.5 * (p3 - p1);

            for step in 1..4 {
                let t = step as f32 * 0.25;
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                let interpolated = (h00 * p1 + h10 * m1 + h01 * p2 + h11 * m2).abs();
                if interpolated > max_abs {
                    max_abs = interpolated;
                }
            }
        }
    }

    if max_abs > 1e-6 {
        20.0 * max_abs.log10()
    } else {
        -90.0
    }
}

/// Color palettes for spectrogram visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrogramPalette {
    #[default]
    Viridis,
    Magma,
    Inferno,
    ClassicHeat,
    Cyberpunk,
}

impl SpectrogramPalette {
    /// Maps normalized intensity `0.0..=1.0` to RGB `(u8, u8, u8)`.
    pub fn color_rgb(&self, norm: f32) -> (u8, u8, u8) {
        let t = norm.clamp(0.0, 1.0);
        match self {
            Self::Viridis => {
                // Approximate Viridis colormap
                let r = (255.0 * (0.28 + 0.72 * t * t - 0.2 * (1.0 - t).powi(3))).clamp(0.0, 255.0)
                    as u8;
                let g = (255.0 * (0.01 + 0.85 * t + 0.14 * (1.0 - (2.0 * t - 1.0).powi(2))))
                    .clamp(0.0, 255.0) as u8;
                let b = (255.0 * (0.33 + 0.67 * (1.0 - (t - 0.3).abs()))).clamp(0.0, 255.0) as u8;
                (r, g, b)
            }
            Self::Magma => {
                // Black -> Dark Purple -> Hot Orange -> Yellow
                let r = (255.0 * (1.5 * t - 0.2).clamp(0.0, 1.0)) as u8;
                let g = (255.0 * (1.8 * t - 0.8).clamp(0.0, 1.0)) as u8;
                let b =
                    (255.0 * (t * 0.8 + 0.4 * (1.0 - (2.0 * t - 1.0).abs())).clamp(0.0, 1.0)) as u8;
                (r, g, b)
            }
            Self::Inferno => {
                // Black -> Purple -> Orange/Red -> Bright Gold
                let r = (255.0 * (1.8 * t).clamp(0.0, 1.0)) as u8;
                let g = (255.0 * (1.6 * t - 0.6).clamp(0.0, 1.0)) as u8;
                let b = (255.0 * (0.8 * (1.0 - t) * t * 4.0 + 0.2 * t).clamp(0.0, 1.0)) as u8;
                (r, g, b)
            }
            Self::ClassicHeat => {
                // Dark Blue -> Green -> Yellow -> Red
                if t < 0.33 {
                    let f = t / 0.33;
                    (0, (255.0 * f) as u8, (255.0 * (1.0 - f)) as u8)
                } else if t < 0.66 {
                    let f = (t - 0.33) / 0.33;
                    ((255.0 * f) as u8, 255, 0)
                } else {
                    let f = (t - 0.66) / 0.34;
                    (255, (255.0 * (1.0 - f)) as u8, 0)
                }
            }
            Self::Cyberpunk => {
                // Deep Navy -> Neon Purple -> Cyan -> Bright Magenta
                let r = (255.0 * (0.2 + 0.8 * t * t)) as u8;
                let g = (255.0 * (0.8 * (1.0 - (t - 0.5).abs() * 2.0).clamp(0.0, 1.0))) as u8;
                let b = (255.0 * (0.4 + 0.6 * (1.0 - t))) as u8;
                (r, g, b)
            }
        }
    }
}

/// Complete loudness and dynamics metrics for mastering analysis.
#[derive(Debug, Clone)]
pub struct DynamicsAnalysisResult {
    pub integrated_lufs: f32,
    pub short_term_max_lufs: f32,
    pub momentary_max_lufs: f32,
    pub loudness_range_lu: f32,
    pub true_peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub crest_factor_db: f32,
    pub dynamic_range_db: f32,
    pub clipped_samples: usize,
}

/// Computes full loudness, True Peak, RMS, Crest Factor, Dynamic Range, and clipping count.
pub fn analyze_full_dynamics(
    interleaved_samples: &[f32],
    channels: u32,
    sample_rate: u32,
) -> Result<DynamicsAnalysisResult, AnalysisError> {
    let loudness = analyze_loudness(interleaved_samples, channels, sample_rate)?;
    let ch_count = channels.max(1) as usize;

    let mut sum_sq = 0.0f64;
    let mut clipped_samples = 0usize;
    let mut peak_abs = 0.0f32;
    let mut max_true_peak = -120.0f32;

    for ch in 0..ch_count {
        let mut ch_samples = Vec::with_capacity(interleaved_samples.len() / ch_count + 1);
        for &s in interleaved_samples.iter().skip(ch).step_by(ch_count) {
            sum_sq += (s as f64) * (s as f64);
            let abs = s.abs();
            if abs > peak_abs {
                peak_abs = abs;
            }
            if abs >= 0.9999 {
                clipped_samples += 1;
            }
            ch_samples.push(s);
        }
        let tp = compute_true_peak_dbfs(&ch_samples);
        if tp > max_true_peak {
            max_true_peak = tp;
        }
    }

    let n = interleaved_samples.len().max(1) as f64;
    let rms = (sum_sq / n).sqrt() as f32;
    let rms_dbfs = if rms > 1e-6 {
        20.0 * rms.log10()
    } else {
        -120.0
    };
    let sample_peak_dbfs = if peak_abs > 1e-6 {
        20.0 * peak_abs.log10()
    } else {
        -120.0
    };
    let crest_factor_db = (sample_peak_dbfs - rms_dbfs).max(0.0);
    let dynamic_range_db = (max_true_peak - loudness.integrated_lufs).max(0.0);

    Ok(DynamicsAnalysisResult {
        integrated_lufs: loudness.integrated_lufs,
        short_term_max_lufs: loudness.short_term_max_lufs,
        momentary_max_lufs: loudness.momentary_max_lufs,
        loudness_range_lu: loudness.loudness_range_lu,
        true_peak_dbfs: max_true_peak,
        rms_dbfs,
        crest_factor_db,
        dynamic_range_db,
        clipped_samples,
    })
}

/// Tonal balance energy distribution across Low (<250 Hz), Mid (250-4000 Hz), and High (>4000 Hz).
#[derive(Debug, Clone, Copy, Default)]
pub struct TonalBalance {
    pub low_pct: f32,
    pub mid_pct: f32,
    pub high_pct: f32,
}

/// Actionable mastering report generated by `MasteringAssistant`.
#[derive(Debug, Clone)]
pub struct MasteringReport {
    pub dynamics: DynamicsAnalysisResult,
    pub tonal_balance: TonalBalance,
    pub target_lufs: f32,
    pub suggested_gain_db: f32,
    pub warnings: Vec<String>,
}

/// Automated mix analyzer and mastering recommendation generator.
pub struct MasteringAssistant;

impl MasteringAssistant {
    pub fn analyze_mix(
        interleaved_samples: &[f32],
        channels: u32,
        sample_rate: u32,
        target_lufs: f32,
    ) -> Result<MasteringReport, AnalysisError> {
        let dynamics = analyze_full_dynamics(interleaved_samples, channels, sample_rate)?;
        let ch_count = channels.max(1) as usize;
        let mono: Vec<f32> = interleaved_samples
            .chunks_exact(ch_count)
            .map(|f| f.iter().sum::<f32>() / ch_count as f32)
            .collect();

        // Compute tonal balance using FFT over representative center window
        let fft_size = 4096usize;
        let mut analyzer = SpectrumAnalyzer::new(fft_size);
        let start = mono.len().saturating_sub(fft_size) / 2;
        let mut slice = vec![0.0f32; fft_size];
        let copy_len = (mono.len().saturating_sub(start)).min(fft_size);
        slice[..copy_len].copy_from_slice(&mono[start..start + copy_len]);
        let mags_db = analyzer.compute_magnitude_db(&slice);

        let bin_hz = sample_rate.max(1) as f32 / fft_size as f32;
        let mut low_e = 0.0f64;
        let mut mid_e = 0.0f64;
        let mut high_e = 0.0f64;
        for (i, &db) in mags_db.iter().enumerate().skip(1) {
            let freq = i as f32 * bin_hz;
            let e = 10.0f64.powf(db as f64 / 10.0);
            if freq < 250.0 {
                low_e += e;
            } else if freq <= 4000.0 {
                mid_e += e;
            } else {
                high_e += e;
            }
        }
        let total_e = (low_e + mid_e + high_e).max(1e-12);
        let tonal_balance = TonalBalance {
            low_pct: (low_e / total_e * 100.0) as f32,
            mid_pct: (mid_e / total_e * 100.0) as f32,
            high_pct: (high_e / total_e * 100.0) as f32,
        };

        let raw_gain = if dynamics.integrated_lufs.is_finite() && dynamics.integrated_lufs > -70.0 {
            (target_lufs - dynamics.integrated_lufs).clamp(-24.0, 24.0)
        } else {
            0.0
        };
        // Keep True Peak safely at or below -1.0 dBTP
        let max_safe_gain = -1.0 - dynamics.true_peak_dbfs;
        let suggested_gain_db = raw_gain.min(max_safe_gain);

        let mut warnings = Vec::new();
        if dynamics.clipped_samples > 0 {
            warnings.push(format!(
                "Clipping detected: {} sample(s) at or above 0 dBFS",
                dynamics.clipped_samples
            ));
        }
        if dynamics.true_peak_dbfs > -1.0 {
            warnings.push(format!(
                "True Peak ({:.2} dBTP) exceeds -1.0 dBTP streaming ceiling",
                dynamics.true_peak_dbfs
            ));
        }
        if raw_gain > max_safe_gain {
            warnings.push(format!(
                "Limiting recommended: +{:.1} dB needed for {:.1} LUFS, safe headroom allows {:+.1} dB",
                raw_gain, target_lufs, max_safe_gain
            ));
        }

        Ok(MasteringReport {
            dynamics,
            tonal_balance,
            target_lufs,
            suggested_gain_db,
            warnings,
        })
    }

    /// Compares a mix against a reference audio track for loudness, dynamics,
    /// and 3-band frequency spectrum balance.
    pub fn compare_with_reference(
        mix_interleaved: &[f32],
        ref_interleaved: &[f32],
        channels: u32,
        sample_rate: u32,
        target_lufs: f32,
    ) -> Result<ReferenceComparison, AnalysisError> {
        let mix = Self::analyze_mix(mix_interleaved, channels, sample_rate, target_lufs)?;
        let reference = Self::analyze_mix(ref_interleaved, channels, sample_rate, target_lufs)?;
        Ok(ReferenceComparison {
            lufs_delta_db: mix.dynamics.integrated_lufs - reference.dynamics.integrated_lufs,
            true_peak_delta_db: mix.dynamics.true_peak_dbfs - reference.dynamics.true_peak_dbfs,
            crest_factor_delta_db: mix.dynamics.crest_factor_db
                - reference.dynamics.crest_factor_db,
            low_delta_pct: mix.tonal_balance.low_pct - reference.tonal_balance.low_pct,
            mid_delta_pct: mix.tonal_balance.mid_pct - reference.tonal_balance.mid_pct,
            high_delta_pct: mix.tonal_balance.high_pct - reference.tonal_balance.high_pct,
            mix,
            reference,
        })
    }
}

/// Comparison result between a project mix and an imported reference track.
#[derive(Debug, Clone)]
pub struct ReferenceComparison {
    pub mix: MasteringReport,
    pub reference: MasteringReport,
    pub lufs_delta_db: f32,
    pub true_peak_delta_db: f32,
    pub crest_factor_delta_db: f32,
    pub low_delta_pct: f32,
    pub mid_delta_pct: f32,
    pub high_delta_pct: f32,
}
