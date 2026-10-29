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

    ebu.add_frames_f32(interleaved_samples)?;

    let integrated = ebu.loudness_global().unwrap_or(-70.0) as f32;
    let momentary = ebu.loudness_momentary().unwrap_or(-70.0) as f32;
    let short_term = ebu.loudness_shortterm().unwrap_or(-70.0) as f32;
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
