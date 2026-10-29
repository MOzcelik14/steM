//! Dynamics processing: Compressor and Brickwall Limiter

#[derive(Debug, Clone)]
pub struct Compressor {
    sample_rate: f32,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_gain_linear: f32,

    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl Compressor {
    pub fn new(
        sample_rate: f32,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    ) -> Self {
        let mut comp = Self {
            sample_rate,
            threshold_db,
            ratio: ratio.max(1.0),
            attack_ms,
            release_ms,
            makeup_gain_linear: 10.0f32.powf(makeup_gain_db / 20.0),
            envelope: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
        };
        comp.update_coefficients();
        comp
    }

    fn update_coefficients(&mut self) {
        let attack_s = (self.attack_ms * 0.001).max(0.0001);
        let release_s = (self.release_ms * 0.001).max(0.001);
        self.attack_coeff = (-1.0 / (self.sample_rate * attack_s)).exp();
        self.release_coeff = (-1.0 / (self.sample_rate * release_s)).exp();
    }

    pub fn set_params(
        &mut self,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    ) {
        self.threshold_db = threshold_db;
        self.ratio = ratio.max(1.0);
        self.attack_ms = attack_ms;
        self.release_ms = release_ms;
        self.makeup_gain_linear = 10.0f32.powf(makeup_gain_db / 20.0);
        self.update_coefficients();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    /// Evaluates the static compression curve: maps input dB to output dB (without makeup gain).
    pub fn compute_static_transfer_db(&self, in_db: f32) -> f32 {
        if in_db > self.threshold_db {
            let over_db = in_db - self.threshold_db;
            let compressed_over = over_db / self.ratio;
            self.threshold_db + compressed_over
        } else {
            in_db
        }
    }

    /// Process a single audio sample and return (output_sample, gain_reduction_db).
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> (f32, f32) {
        let abs_in = input.abs();

        // Envelope detection
        if abs_in > self.envelope {
            self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_in;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_in;
        }

        // Convert envelope to dBFS (clamped at -120 dB)
        let env_db = if self.envelope > 1e-6 {
            20.0 * self.envelope.log10()
        } else {
            -120.0
        };

        // Static compression curve
        let gain_reduction_db = if env_db > self.threshold_db {
            let over_db = env_db - self.threshold_db;
            let compressed_over_db = over_db / self.ratio;
            compressed_over_db - over_db
        } else {
            0.0
        };

        let gain_linear = 10.0f32.powf(gain_reduction_db / 20.0) * self.makeup_gain_linear;
        (input * gain_linear, -gain_reduction_db)
    }

    /// Process an interleaved stereo or mono buffer.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let (out, _) = self.process_sample(*sample);
            *sample = out;
        }
    }
}

/// Brickwall Peak Limiter to prevent clipping.
#[derive(Debug, Clone)]
pub struct Limiter {
    _sample_rate: f32,
    threshold_linear: f32,
    release_coeff: f32,
    gain: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32, threshold_db: f32, release_ms: f32) -> Self {
        let release_s = (release_ms * 0.001).max(0.001);
        Self {
            _sample_rate: sample_rate,
            threshold_linear: 10.0f32.powf(threshold_db / 20.0),
            release_coeff: (-1.0 / (sample_rate * release_s)).exp(),
            gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
    }

    pub fn set_params(&mut self, threshold_db: f32, release_ms: f32) {
        let release_s = (release_ms * 0.001).max(0.001);
        self.threshold_linear = 10.0f32.powf(threshold_db / 20.0);
        self.release_coeff = (-1.0 / (self._sample_rate * release_s)).exp();
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let peak = input.abs();
        let target_gain = if peak > self.threshold_linear && peak > 1e-6 {
            self.threshold_linear / peak
        } else {
            1.0
        };

        // Instant attack to catch peaks, smooth release
        if target_gain < self.gain {
            self.gain = target_gain;
        } else {
            self.gain = self.release_coeff * self.gain + (1.0 - self.release_coeff) * target_gain;
        }

        input * self.gain
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}
