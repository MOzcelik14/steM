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
            sample_rate: sample_rate.max(1.0),
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
        let sr = self.sample_rate.max(1.0);
        let attack_s = (self.attack_ms * 0.001).max(0.0001);
        let release_s = (self.release_ms * 0.001).max(0.001);
        self.attack_coeff = (-1.0 / (sr * attack_s)).exp();
        self.release_coeff = (-1.0 / (sr * release_s)).exp();
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

    /// Process a single audio sample using an external sidechain key signal,
    /// returning `(output_sample, gain_reduction_db)`.
    #[inline]
    pub fn process_sample_sidechain(&mut self, input: f32, key: f32) -> (f32, f32) {
        let abs_key = key.abs();

        // Envelope detection on sidechain key
        if abs_key > self.envelope {
            self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_key;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_key;
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

    /// Process a single audio sample and return (output_sample, gain_reduction_db).
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> (f32, f32) {
        self.process_sample_sidechain(input, input)
    }

    /// Process an interleaved stereo or mono buffer.
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let (out, _) = self.process_sample(*sample);
            *sample = out;
        }
    }

    /// Process a buffer using an external sidechain key buffer of matching length.
    pub fn process_buffer_sidechain(&mut self, buffer: &mut [f32], sidechain_key: &[f32]) {
        for (sample, &key) in buffer.iter_mut().zip(sidechain_key.iter()) {
            let (out, _) = self.process_sample_sidechain(*sample, key);
            *sample = out;
        }
    }
}

/// Brickwall Peak Limiter with short lookahead to prevent clipping.
#[derive(Debug, Clone)]
pub struct Limiter {
    _sample_rate: f32,
    threshold_linear: f32,
    release_coeff: f32,
    gain: f32,
    lookahead_buf: Vec<f32>,
    lookahead_pos: usize,
}

impl Limiter {
    pub fn new(sample_rate: f32, threshold_db: f32, release_ms: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let release_s = (release_ms * 0.001).max(0.001);
        let lookahead_samples = ((sr * 0.002).round() as usize).clamp(1, 256);
        Self {
            _sample_rate: sr,
            threshold_linear: 10.0f32.powf(threshold_db / 20.0),
            release_coeff: (-1.0 / (sr * release_s)).exp(),
            gain: 1.0,
            lookahead_buf: vec![0.0; lookahead_samples],
            lookahead_pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.lookahead_buf.fill(0.0);
        self.lookahead_pos = 0;
    }

    pub fn set_params(&mut self, threshold_db: f32, release_ms: f32) {
        let release_s = (release_ms * 0.001).max(0.001);
        self.threshold_linear = 10.0f32.powf(threshold_db / 20.0);
        self.release_coeff = (-1.0 / (self._sample_rate * release_s)).exp();
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delayed = self.lookahead_buf[self.lookahead_pos];
        self.lookahead_buf[self.lookahead_pos] = input;
        self.lookahead_pos = (self.lookahead_pos + 1) % self.lookahead_buf.len();

        let peak = input.abs().max(delayed.abs());
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

        (delayed * self.gain).clamp(-self.threshold_linear, self.threshold_linear)
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

/// Real-time Noise Gate with attack, hold, release, and floor range control.
#[derive(Debug, Clone)]
pub struct NoiseGate {
    sample_rate: f32,
    threshold_linear: f32,
    attack_ms: f32,
    hold_ms: f32,
    release_ms: f32,
    attack_coeff: f32,
    release_coeff: f32,
    hold_samples: u32,
    range_linear: f32,
    envelope: f32,
    hold_counter: u32,
    gate_gain: f32,
}

impl NoiseGate {
    pub fn new(
        sample_rate: f32,
        threshold_db: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
        range_db: f32,
    ) -> Self {
        let mut gate = Self {
            sample_rate: sample_rate.max(1.0),
            threshold_linear: 10.0f32.powf(threshold_db / 20.0),
            attack_ms,
            hold_ms,
            release_ms,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            hold_samples: 0,
            range_linear: 10.0f32.powf(range_db.clamp(-96.0, 0.0) / 20.0),
            envelope: 0.0,
            hold_counter: 0,
            gate_gain: 1.0,
        };
        gate.set_params(threshold_db, attack_ms, hold_ms, release_ms, range_db);
        gate
    }

    pub fn set_params(
        &mut self,
        threshold_db: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
        range_db: f32,
    ) {
        self.threshold_linear = 10.0f32.powf(threshold_db / 20.0);
        self.attack_ms = attack_ms;
        self.hold_ms = hold_ms;
        self.release_ms = release_ms;
        self.range_linear = 10.0f32.powf(range_db.clamp(-96.0, 0.0) / 20.0);
        self.update_coefficients();
    }

    fn update_coefficients(&mut self) {
        let attack_s = (self.attack_ms * 0.001).max(0.0001);
        let release_s = (self.release_ms * 0.001).max(0.001);
        self.attack_coeff = (-1.0 / (self.sample_rate * attack_s)).exp();
        self.release_coeff = (-1.0 / (self.sample_rate * release_s)).exp();
        self.hold_samples = ((self.hold_ms.max(0.0) * 0.001) * self.sample_rate) as u32;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        self.update_coefficients();
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.hold_counter = 0;
        self.gate_gain = 1.0;
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let abs_in = input.abs();
        if abs_in > self.envelope {
            self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_in;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_in;
        }

        let target_gain = if self.envelope >= self.threshold_linear {
            self.hold_counter = self.hold_samples;
            1.0
        } else if self.hold_counter > 0 {
            self.hold_counter -= 1;
            1.0
        } else {
            self.range_linear
        };

        if target_gain > self.gate_gain {
            self.gate_gain =
                self.attack_coeff * self.gate_gain + (1.0 - self.attack_coeff) * target_gain;
        } else {
            self.gate_gain =
                self.release_coeff * self.gate_gain + (1.0 - self.release_coeff) * target_gain;
        }

        input * self.gate_gain
    }
}

/// Split-band De-Esser targeting high-frequency sibilance (e.g. 4-9 kHz).
#[derive(Clone)]
pub struct DeEsser {
    sample_rate: f32,
    frequency_hz: f32,
    threshold_db: f32,
    ratio: f32,
    range_db: f32,
    bandpass: crate::biquad::BiquadFilter,
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl DeEsser {
    pub fn new(
        sample_rate: f32,
        frequency_hz: f32,
        threshold_db: f32,
        ratio: f32,
        range_db: f32,
    ) -> Self {
        let sr = sample_rate.max(1.0);
        let coeffs = crate::biquad::BiquadCoeffs::design(
            stem_core::project::FilterType::BandPass,
            sr,
            frequency_hz.clamp(2000.0, (sr * 0.45).max(2500.0)),
            2.0,
            0.0,
        );
        Self {
            sample_rate: sr,
            frequency_hz,
            threshold_db,
            ratio: ratio.max(1.0),
            range_db: range_db.clamp(-30.0, 0.0),
            bandpass: crate::biquad::BiquadFilter::new(coeffs),
            envelope: 0.0,
            attack_coeff: (-1.0 / (sr * 0.001)).exp(),
            release_coeff: (-1.0 / (sr * 0.040)).exp(),
        }
    }

    pub fn set_params(&mut self, frequency_hz: f32, threshold_db: f32, ratio: f32, range_db: f32) {
        self.frequency_hz = frequency_hz;
        self.threshold_db = threshold_db;
        self.ratio = ratio.max(1.0);
        self.range_db = range_db.clamp(-30.0, 0.0);
        let coeffs = crate::biquad::BiquadCoeffs::design(
            stem_core::project::FilterType::BandPass,
            self.sample_rate,
            frequency_hz.clamp(2000.0, (self.sample_rate * 0.45).max(2500.0)),
            2.0,
            0.0,
        );
        self.bandpass.coeffs = coeffs;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        self.attack_coeff = (-1.0 / (self.sample_rate * 0.001)).exp();
        self.release_coeff = (-1.0 / (self.sample_rate * 0.040)).exp();
        self.set_params(
            self.frequency_hz,
            self.threshold_db,
            self.ratio,
            self.range_db,
        );
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.bandpass.reset();
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let sib = self.bandpass.process_sample(input);
        let abs_sib = sib.abs();
        if abs_sib > self.envelope {
            self.envelope = self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_sib;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_sib;
        }

        let env_db = if self.envelope > 1e-6 {
            20.0 * self.envelope.log10()
        } else {
            -120.0
        };

        let gr_db = if env_db > self.threshold_db {
            let over = env_db - self.threshold_db;
            let raw_reduction = (over / self.ratio) - over;
            raw_reduction.max(self.range_db)
        } else {
            0.0
        };

        if gr_db >= -0.001 {
            return input;
        }

        let gr_linear = 10.0f32.powf(gr_db / 20.0);
        let rest = input - sib;
        rest + sib * gr_linear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_gate_attenuates_quiet_and_passes_loud() {
        let mut gate = NoiseGate::new(48_000.0, -40.0, 1.0, 10.0, 50.0, -60.0);
        // Feed very quiet signal (-80 dBFS ~ 0.0001)
        let mut quiet_out = 0.0f32;
        for _ in 0..24_000 {
            quiet_out = gate.process_sample(0.0001).abs();
        }
        assert!(
            quiet_out < 0.000001,
            "expected gate to attenuate quiet signal, got {quiet_out}"
        );

        // Feed loud signal (0.5 ~ -6 dBFS)
        let mut loud_out = 0.0f32;
        for _ in 0..480 {
            loud_out = gate.process_sample(0.5).abs();
        }
        assert!(
            loud_out > 0.45,
            "expected gate to open for loud signal, got {loud_out}"
        );
    }

    #[test]
    fn test_deesser_attenuates_sibilance_more_than_low_tone() {
        let sr = 48_000.0f32;
        let mut deesser = DeEsser::new(sr, 6500.0, -30.0, 6.0, -18.0);

        // Low 200 Hz tone should pass nearly untouched
        let mut low_peak = 0.0f32;
        for i in 0..2400 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 200.0 * (i as f32) / sr).sin();
            let y = deesser.process_sample(x);
            if i > 1200 {
                low_peak = low_peak.max(y.abs());
            }
        }
        assert!(low_peak > 0.45, "low tone attenuated too much: {low_peak}");

        deesser.reset();
        // High 6500 Hz sibilance at 0.5 amplitude should be reduced
        let mut high_peak = 0.0f32;
        for i in 0..2400 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 6500.0 * (i as f32) / sr).sin();
            let y = deesser.process_sample(x);
            if i > 1200 {
                high_peak = high_peak.max(y.abs());
            }
        }
        assert!(
            high_peak < 0.35,
            "sibilance not attenuated enough: {high_peak}"
        );
    }
}
