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
