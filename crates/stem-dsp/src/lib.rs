//! Pure Digital Signal Processing algorithms for steM.
//!
//! Contains filters, dynamics processing, spatial effects, resampling,
//! TPDF dither, loudness measurement, FFT spectral analysis, and peak cache building.
//! Strictly pure DSP with zero file I/O or UI dependencies.

pub mod analysis;
pub mod biquad;
pub mod chain;
pub mod dither;
pub mod dynamics;
pub mod peaks;
pub mod resample;
pub mod restoration;
pub mod spatial;

pub use analysis::{
    analyze_loudness, compute_true_peak_dbfs, LoudnessResult, SpectrogramPalette, SpectrumAnalyzer,
};
pub use biquad::{BiquadCoeffs, BiquadFilter};
pub use chain::{
    CompressorEffect, DelayEffect, EffectChain, EqEffect, InsertEffect, LimiterEffect,
    ReverbEffect, SmoothedParam,
};
pub use dither::TpdfDither;
pub use dynamics::{Compressor, Limiter};
pub use peaks::build_peak_pyramid;
pub use resample::resample_audio;
pub use restoration::{ClickRemover, NoiseProfile, SpectralGate};
pub use spatial::{Delay, Reverb};
