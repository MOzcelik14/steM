//! Triangular Probability Density Function (TPDF) Dither
//!
//! When reducing audio bit depth (e.g. from internal 32-bit float to 24-bit or 16-bit integer
//! upon export or playback), naive truncation or rounding produces quantization distortion that
//! is correlated with the signal, perceived as unpleasant harmonic distortion and graininess.
//!
//! Adding Triangular Probability Density Function (TPDF) dither—formed by summing two
//! independent uniform random noise sources spanning [-0.5, +0.5] LSB—linearizes the quantization
//! transfer characteristic. This completely eliminates harmonic distortion and signal-dependent
//! noise modulation, converting quantization error into an entirely stationary, benign white noise floor.

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub struct TpdfDither {
    rng: SmallRng,
}

impl Default for TpdfDither {
    fn default() -> Self {
        Self::new()
    }
}

impl TpdfDither {
    pub fn new() -> Self {
        // Fast, non-cryptographic RNG suitable for real-time audio and fast offline bounce
        Self {
            rng: SmallRng::from_entropy(),
        }
    }

    /// Generates one sample of TPDF noise in the range [-1.0, +1.0] LSB.
    /// Resulting distribution is triangular, peaking at 0.0 with zero mean and variance = 1/6 LSB^2.
    #[inline]
    pub fn next_tpdf(&mut self) -> f32 {
        let r1: f32 = self.rng.gen_range(-0.5..0.5);
        let r2: f32 = self.rng.gen_range(-0.5..0.5);
        r1 + r2
    }

    /// Quantize a normalized float [-1.0, +1.0] to a 16-bit signed integer using TPDF dither.
    #[inline]
    pub fn quantize_i16(&mut self, sample: f32) -> i16 {
        const SCALE: f32 = 32767.0;
        let dither = self.next_tpdf();
        let scaled = sample * SCALE + dither;
        scaled.round().clamp(-32768.0, 32767.0) as i16
    }

    /// Quantize a normalized float [-1.0, +1.0] to a 24-bit signed integer (in i32) using TPDF dither.
    #[inline]
    pub fn quantize_i24(&mut self, sample: f32) -> i32 {
        const SCALE: f32 = 8388607.0;
        let dither = self.next_tpdf();
        let scaled = sample * SCALE + dither;
        scaled.round().clamp(-8388608.0, 8388607.0) as i32
    }

    /// Process a slice of 32-bit floats into 16-bit PCM in-place or into target slice.
    pub fn process_to_i16(&mut self, input: &[f32], output: &mut [i16]) {
        assert_eq!(input.len(), output.len());
        for (src, dst) in input.iter().zip(output.iter_mut()) {
            *dst = self.quantize_i16(*src);
        }
    }

    /// Process a slice of 32-bit floats into 24-bit PCM (stored in i32).
    pub fn process_to_i24(&mut self, input: &[f32], output: &mut [i32]) {
        assert_eq!(input.len(), output.len());
        for (src, dst) in input.iter().zip(output.iter_mut()) {
            *dst = self.quantize_i24(*src);
        }
    }
}
