//! Real-time Effect Insert Chain and Parameter Smoothing
//!
//! Provides sample-accurate, zero-allocation per-sample effects processing
//! for Track and Master insert chains with per-effect bypass, reset, and reordering.

use stem_core::project::{EqBandConfig, FilterType};

use crate::biquad::{BiquadCoeffs, BiquadFilter};
use crate::dynamics::{Compressor, Limiter};
use crate::spatial::{Delay, Reverb};

/// Exponential one-pole smoothing filter for continuous parameters (gain, mix, frequency).
/// Prevents zipper noise and clicks when sliders or automation points change.
#[derive(Debug, Clone, Copy)]
pub struct SmoothedParam {
    current: f32,
    target: f32,
    coeff: f32,
}

impl SmoothedParam {
    pub fn new(initial: f32, sample_rate: f32, time_constant_ms: f32) -> Self {
        let tc_sec = (time_constant_ms * 0.001).max(0.0001);
        let coeff = (-1.0 / (sample_rate.max(1.0) * tc_sec)).exp();
        Self {
            current: initial,
            target: initial,
            coeff,
        }
    }

    #[inline]
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    #[inline]
    pub fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    #[inline]
    pub fn next_value(&mut self) -> f32 {
        self.current = self.coeff * self.current + (1.0 - self.coeff) * self.target;
        self.current
    }

    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        self.next_value()
    }

    #[inline]
    pub fn current(&self) -> f32 {
        self.current
    }

    #[inline]
    pub fn target(&self) -> f32 {
        self.target
    }

    #[inline]
    pub fn is_smoothing(&self) -> bool {
        (self.current - self.target).abs() > 1e-5
    }
}

// -----------------------------------------------------------------------------
// Stereo Insert Effects
// -----------------------------------------------------------------------------

/// 4-Band Parametric Stereo Equalizer.
#[derive(Clone)]
pub struct EqEffect {
    pub bypassed: bool,
    sample_rate: f32,
    pub bands: [EqBandConfig; 4],
    filters_l: [BiquadFilter; 4],
    filters_r: [BiquadFilter; 4],
}

impl EqEffect {
    pub fn new(sample_rate: f32) -> Self {
        let default_bands = [
            EqBandConfig {
                enabled: true,
                filter_type: FilterType::LowShelf,
                frequency: 100.0,
                gain_db: 0.0,
                q: 0.707,
            },
            EqBandConfig {
                enabled: true,
                filter_type: FilterType::Peaking,
                frequency: 800.0,
                gain_db: 0.0,
                q: 1.0,
            },
            EqBandConfig {
                enabled: true,
                filter_type: FilterType::Peaking,
                frequency: 3000.0,
                gain_db: 0.0,
                q: 1.0,
            },
            EqBandConfig {
                enabled: true,
                filter_type: FilterType::HighShelf,
                frequency: 10000.0,
                gain_db: 0.0,
                q: 0.707,
            },
        ];

        let mut eq = Self {
            bypassed: false,
            sample_rate,
            bands: default_bands,
            filters_l: Default::default(),
            filters_r: Default::default(),
        };
        eq.recompute_all_bands();
        eq
    }

    pub fn set_band(&mut self, band_idx: usize, config: EqBandConfig) {
        if band_idx < 4 {
            self.bands[band_idx] = config;
            self.recompute_band(band_idx);
        }
    }

    pub fn recompute_band(&mut self, band_idx: usize) {
        let b = &self.bands[band_idx];
        let coeffs = if b.enabled {
            BiquadCoeffs::design(b.filter_type, self.sample_rate, b.frequency, b.q, b.gain_db)
        } else {
            BiquadCoeffs::default()
        };
        self.filters_l[band_idx].coeffs = coeffs;
        self.filters_r[band_idx].coeffs = coeffs;
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match param_id {
            // 0..3: Band 0..3 Gain dB (-24.0 to +24.0)
            0..=3 => {
                let idx = param_id as usize;
                self.bands[idx].gain_db = value.clamp(-24.0, 24.0);
                self.recompute_band(idx);
            }
            // 4..7: Band 0..3 Frequency Hz (20.0 to 20000.0)
            4..=7 => {
                let idx = (param_id - 4) as usize;
                self.bands[idx].frequency = value.clamp(20.0, 20000.0);
                self.recompute_band(idx);
            }
            // 8..11: Band 0..3 Q (0.1 to 10.0)
            8..=11 => {
                let idx = (param_id - 8) as usize;
                self.bands[idx].q = value.clamp(0.1, 10.0);
                self.recompute_band(idx);
            }
            // 12..15: Band 0..3 Enabled (value > 0.5)
            12..=15 => {
                let idx = (param_id - 12) as usize;
                self.bands[idx].enabled = value > 0.5;
                self.recompute_band(idx);
            }
            _ => {}
        }
    }

    pub fn recompute_all_bands(&mut self) {
        for i in 0..4 {
            self.recompute_band(i);
        }
    }

    pub fn reset(&mut self) {
        for f in &mut self.filters_l {
            f.reset();
        }
        for f in &mut self.filters_r {
            f.reset();
        }
    }

    #[inline]
    pub fn process_stereo(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        for i in 0..4 {
            if self.bands[i].enabled {
                left = self.filters_l[i].process_sample(left);
                right = self.filters_r[i].process_sample(right);
            }
        }
        (left, right)
    }
}

/// Stereo Dynamics Compressor with peak detection and smoothed makeup gain.
#[derive(Clone)]
pub struct CompressorEffect {
    pub bypassed: bool,
    pub sample_rate: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_gain_db: f32,
    pub inner: Compressor,
    pub makeup_linear: SmoothedParam,
}

impl CompressorEffect {
    pub fn new(
        sample_rate: f32,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    ) -> Self {
        let inner = Compressor::new(
            sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            0.0, // Makeup gain handled by SmoothedParam
        );
        let linear_gain = 10.0f32.powf(makeup_gain_db / 20.0);
        let makeup_linear = SmoothedParam::new(linear_gain, sample_rate, 15.0);

        Self {
            bypassed: false,
            sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_gain_db,
            inner,
            makeup_linear,
        }
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
        self.ratio = ratio;
        self.attack_ms = attack_ms;
        self.release_ms = release_ms;
        self.makeup_gain_db = makeup_gain_db;

        self.inner
            .set_params(threshold_db, ratio, attack_ms, release_ms, 0.0);
        self.makeup_linear
            .set_target(10.0f32.powf(makeup_gain_db / 20.0));
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match param_id {
            0 => {
                self.threshold_db = value.clamp(-60.0, 0.0);
                self.inner.set_params(
                    self.threshold_db,
                    self.ratio,
                    self.attack_ms,
                    self.release_ms,
                    0.0,
                );
            }
            1 => {
                self.ratio = value.clamp(1.0, 30.0);
                self.inner.set_params(
                    self.threshold_db,
                    self.ratio,
                    self.attack_ms,
                    self.release_ms,
                    0.0,
                );
            }
            2 => {
                self.attack_ms = value.clamp(0.1, 500.0);
                self.inner.set_params(
                    self.threshold_db,
                    self.ratio,
                    self.attack_ms,
                    self.release_ms,
                    0.0,
                );
            }
            3 => {
                self.release_ms = value.clamp(1.0, 2000.0);
                self.inner.set_params(
                    self.threshold_db,
                    self.ratio,
                    self.attack_ms,
                    self.release_ms,
                    0.0,
                );
            }
            4 => {
                self.makeup_gain_db = value.clamp(0.0, 36.0);
                self.makeup_linear
                    .set_target(10.0f32.powf(self.makeup_gain_db / 20.0));
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
        self.makeup_linear
            .reset(10.0f32.powf(self.makeup_gain_db / 20.0));
    }

    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        // Stereo-linked sidechain: detect peak of both channels to prevent image shifting
        let sidechain = left.abs().max(right.abs());
        let (_, gain_reduction_db) = self.inner.process_sample(sidechain);
        let gain = 10.0f32.powf(-gain_reduction_db / 20.0);
        let makeup = self.makeup_linear.next();
        let total_gain = gain * makeup;
        (left * total_gain, right * total_gain)
    }
}

/// Stereo Brickwall Limiter.
#[derive(Clone)]
pub struct LimiterEffect {
    pub bypassed: bool,
    pub sample_rate: f32,
    pub threshold_db: f32,
    pub release_ms: f32,
    pub inner: Limiter,
}

impl LimiterEffect {
    pub fn new(sample_rate: f32, threshold_db: f32, release_ms: f32) -> Self {
        Self {
            bypassed: false,
            sample_rate,
            threshold_db,
            release_ms,
            inner: Limiter::new(sample_rate, threshold_db, release_ms),
        }
    }

    pub fn set_params(&mut self, threshold_db: f32, release_ms: f32) {
        self.threshold_db = threshold_db;
        self.release_ms = release_ms;
        self.inner.set_params(threshold_db, release_ms);
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match param_id {
            0 => {
                self.threshold_db = value.clamp(-30.0, 0.0);
                self.inner.set_params(self.threshold_db, self.release_ms);
            }
            1 => {
                self.release_ms = value.clamp(1.0, 1000.0);
                self.inner.set_params(self.threshold_db, self.release_ms);
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        // Stereo linked
        let max_peak = left.abs().max(right.abs());
        let limited_peak = self.inner.process_sample(max_peak);
        let gain = if max_peak > 1e-6 {
            limited_peak / max_peak
        } else {
            1.0
        };
        (left * gain, right * gain)
    }
}

/// Stereo Delay Effect with smoothed feedback and dry/wet mix.
pub struct DelayEffect {
    pub bypassed: bool,
    pub sample_rate: f32,
    pub time_ms: f32,
    pub feedback: f32,
    delay_l: Delay,
    delay_r: Delay,
    pub mix: SmoothedParam,
}

impl Clone for DelayEffect {
    fn clone(&self) -> Self {
        let mut d = Self::new(
            self.sample_rate,
            self.time_ms,
            self.feedback,
            self.mix.target(),
        );
        d.bypassed = self.bypassed;
        d
    }
}

impl DelayEffect {
    pub fn new(sample_rate: f32, time_ms: f32, feedback: f32, mix: f32) -> Self {
        let mut dl = Delay::new(sample_rate, 2000.0);
        let mut dr = Delay::new(sample_rate, 2000.0);
        dl.set_params(time_ms, feedback, 1.0); // We manage wet/dry externally with SmoothedParam
        dr.set_params(time_ms * 1.05, feedback, 1.0); // Slight stereo offset

        Self {
            bypassed: false,
            sample_rate,
            time_ms,
            feedback,
            delay_l: dl,
            delay_r: dr,
            mix: SmoothedParam::new(mix.clamp(0.0, 1.0), sample_rate, 20.0),
        }
    }

    pub fn set_params(&mut self, time_ms: f32, feedback: f32, mix: f32) {
        self.time_ms = time_ms.clamp(1.0, 2000.0);
        self.feedback = feedback.clamp(0.0, 0.95);
        self.delay_l.set_params(self.time_ms, self.feedback, 1.0);
        self.delay_r
            .set_params(self.time_ms * 1.05, self.feedback, 1.0);
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match param_id {
            0 => {
                self.time_ms = value.clamp(1.0, 2000.0);
                self.delay_l.set_params(self.time_ms, self.feedback, 1.0);
                self.delay_r
                    .set_params(self.time_ms * 1.05, self.feedback, 1.0);
            }
            1 => {
                self.feedback = value.clamp(0.0, 0.95);
                self.delay_l.set_params(self.time_ms, self.feedback, 1.0);
                self.delay_r
                    .set_params(self.time_ms * 1.05, self.feedback, 1.0);
            }
            2 => {
                self.mix.set_target(value.clamp(0.0, 1.0));
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.delay_l.reset();
        self.delay_r.reset();
    }

    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        let wet_l = self.delay_l.process_sample(left);
        let wet_r = self.delay_r.process_sample(right);
        let m = self.mix.next();
        let dry = 1.0 - m;
        (left * dry + wet_l * m, right * dry + wet_r * m)
    }
}

/// Stereo Algorithmic Reverb with smoothed dry/wet mix.
pub struct ReverbEffect {
    pub bypassed: bool,
    pub sample_rate: f32,
    pub room_size: f32,
    pub damping: f32,
    reverb_l: Reverb,
    reverb_r: Reverb,
    pub mix: SmoothedParam,
}

impl Clone for ReverbEffect {
    fn clone(&self) -> Self {
        let mut r = Self::new(
            self.sample_rate,
            self.room_size,
            self.damping,
            self.mix.target(),
        );
        r.bypassed = self.bypassed;
        r
    }
}

impl ReverbEffect {
    pub fn new(sample_rate: f32, room_size: f32, damping: f32, mix: f32) -> Self {
        let mut rl = Reverb::new(sample_rate);
        let mut rr = Reverb::new(sample_rate);
        rl.set_params(room_size, damping, 1.0);
        rr.set_params(room_size, damping, 1.0);

        Self {
            bypassed: false,
            sample_rate,
            room_size,
            damping,
            reverb_l: rl,
            reverb_r: rr,
            mix: SmoothedParam::new(mix.clamp(0.0, 1.0), sample_rate, 25.0),
        }
    }

    pub fn set_params(&mut self, room_size: f32, damping: f32, mix: f32) {
        self.room_size = room_size.clamp(0.0, 0.98);
        self.damping = damping.clamp(0.0, 1.0);
        self.reverb_l.set_params(self.room_size, self.damping, 1.0);
        self.reverb_r.set_params(self.room_size, self.damping, 1.0);
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match param_id {
            0 => {
                self.room_size = value.clamp(0.0, 0.98);
                self.reverb_l.set_params(self.room_size, self.damping, 1.0);
                self.reverb_r.set_params(self.room_size, self.damping, 1.0);
            }
            1 => {
                self.damping = value.clamp(0.0, 1.0);
                self.reverb_l.set_params(self.room_size, self.damping, 1.0);
                self.reverb_r.set_params(self.room_size, self.damping, 1.0);
            }
            2 => {
                self.mix.set_target(value.clamp(0.0, 1.0));
            }
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        self.reverb_l.reset();
        self.reverb_r.reset();
    }

    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        let wet_l = self.reverb_l.process_sample(left);
        let wet_r = self.reverb_r.process_sample(right);
        let m = self.mix.next();
        let dry = 1.0 - m;
        (left * dry + wet_l * m, right * dry + wet_r * m)
    }
}

// -----------------------------------------------------------------------------
// Unified Insert Effect Enum & Chain
// -----------------------------------------------------------------------------

#[derive(Clone)]
pub enum InsertEffect {
    Equalizer(EqEffect),
    Compressor(CompressorEffect),
    Limiter(LimiterEffect),
    Delay(DelayEffect),
    Reverb(ReverbEffect),
}

impl InsertEffect {
    pub fn name(&self) -> &'static str {
        match self {
            InsertEffect::Equalizer(_) => "Equalizer",
            InsertEffect::Compressor(_) => "Compressor",
            InsertEffect::Limiter(_) => "Limiter",
            InsertEffect::Delay(_) => "Delay",
            InsertEffect::Reverb(_) => "Reverb",
        }
    }

    pub fn is_bypassed(&self) -> bool {
        match self {
            InsertEffect::Equalizer(e) => e.bypassed,
            InsertEffect::Compressor(e) => e.bypassed,
            InsertEffect::Limiter(e) => e.bypassed,
            InsertEffect::Delay(e) => e.bypassed,
            InsertEffect::Reverb(e) => e.bypassed,
        }
    }

    pub fn set_bypassed(&mut self, bypassed: bool) {
        match self {
            InsertEffect::Equalizer(e) => e.bypassed = bypassed,
            InsertEffect::Compressor(e) => e.bypassed = bypassed,
            InsertEffect::Limiter(e) => e.bypassed = bypassed,
            InsertEffect::Delay(e) => e.bypassed = bypassed,
            InsertEffect::Reverb(e) => e.bypassed = bypassed,
        }
    }

    pub fn set_param(&mut self, param_id: u8, value: f32) {
        match self {
            InsertEffect::Equalizer(e) => e.set_param(param_id, value),
            InsertEffect::Compressor(e) => e.set_param(param_id, value),
            InsertEffect::Limiter(e) => e.set_param(param_id, value),
            InsertEffect::Delay(e) => e.set_param(param_id, value),
            InsertEffect::Reverb(e) => e.set_param(param_id, value),
        }
    }

    pub fn reset(&mut self) {
        match self {
            InsertEffect::Equalizer(e) => e.reset(),
            InsertEffect::Compressor(e) => e.reset(),
            InsertEffect::Limiter(e) => e.reset(),
            InsertEffect::Delay(e) => e.reset(),
            InsertEffect::Reverb(e) => e.reset(),
        }
    }

    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        match self {
            InsertEffect::Equalizer(e) => e.process_stereo(left, right),
            InsertEffect::Compressor(e) => e.process_stereo(left, right),
            InsertEffect::Limiter(e) => e.process_stereo(left, right),
            InsertEffect::Delay(e) => e.process_stereo(left, right),
            InsertEffect::Reverb(e) => e.process_stereo(left, right),
        }
    }
}

/// Real-time insert effect chain.
/// Supports in-place stereo processing, reordering, and per-effect bypass/reset.
#[derive(Clone, Default)]
pub struct EffectChain {
    pub effects: Vec<InsertEffect>,
}

impl EffectChain {
    pub fn new() -> Self {
        Self {
            effects: Vec::with_capacity(8),
        }
    }

    /// Creates a default insert chain with all 5 core effects (EQ, Compressor, Limiter, Delay, Reverb).
    pub fn default_chain(sample_rate: f32) -> Self {
        let mut chain = Self::new();
        // EQ is active by default with flat bands
        chain.add_effect(InsertEffect::Equalizer(EqEffect::new(sample_rate)));

        // Compressor bypassed by default
        let mut comp = CompressorEffect::new(sample_rate, -18.0, 3.0, 15.0, 100.0, 0.0);
        comp.bypassed = true;
        chain.add_effect(InsertEffect::Compressor(comp));

        // Limiter bypassed by default
        let mut limiter = LimiterEffect::new(sample_rate, -0.5, 50.0);
        limiter.bypassed = true;
        chain.add_effect(InsertEffect::Limiter(limiter));

        // Delay bypassed by default
        let mut delay = DelayEffect::new(sample_rate, 350.0, 0.3, 0.25);
        delay.bypassed = true;
        chain.add_effect(InsertEffect::Delay(delay));

        // Reverb bypassed by default
        let mut reverb = ReverbEffect::new(sample_rate, 0.65, 0.3, 0.2);
        reverb.bypassed = true;
        chain.add_effect(InsertEffect::Reverb(reverb));

        chain
    }

    pub fn add_effect(&mut self, effect: InsertEffect) {
        self.effects.push(effect);
    }

    pub fn remove_effect(&mut self, index: usize) -> Option<InsertEffect> {
        if index < self.effects.len() {
            Some(self.effects.remove(index))
        } else {
            None
        }
    }

    pub fn reorder(&mut self, from: usize, to: usize) {
        let len = self.effects.len();
        if from < len && to < len && from != to {
            let effect = self.effects.remove(from);
            self.effects.insert(to, effect);
        }
    }

    pub fn set_bypassed(&mut self, index: usize, bypassed: bool) {
        if let Some(e) = self.effects.get_mut(index) {
            e.set_bypassed(bypassed);
        }
    }

    pub fn is_bypassed(&self, index: usize) -> bool {
        self.effects
            .get(index)
            .map(|e| e.is_bypassed())
            .unwrap_or(true)
    }

    pub fn set_param(&mut self, effect_idx: usize, param_id: u8, value: f32) {
        if let Some(e) = self.effects.get_mut(effect_idx) {
            e.set_param(param_id, value);
        }
    }

    pub fn reset(&mut self, index: usize) {
        if let Some(e) = self.effects.get_mut(index) {
            e.reset();
        }
    }

    pub fn reset_all(&mut self) {
        for e in &mut self.effects {
            e.reset();
        }
    }

    #[inline]
    pub fn process_stereo(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        for e in &mut self.effects {
            (left, right) = e.process_stereo(left, right);
        }
        (left, right)
    }

    pub fn process_buffer(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_stereo(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }
}
