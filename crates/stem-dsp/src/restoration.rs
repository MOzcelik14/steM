//! Audio restoration: spectral gating noise reduction and click/pop removal.

use realfft::RealFftPlanner;
use std::f32::consts::PI;

/// Captured frequency spectrum representing background noise floor.
#[derive(Debug, Clone)]
pub struct NoiseProfile {
    pub fft_size: usize,
    pub sample_rate: u32,
    /// Average linear magnitude per FFT frequency bin (length: fft_size / 2 + 1).
    pub magnitudes: Vec<f32>,
}

impl NoiseProfile {
    /// Captures a noise profile from a representative audio selection (e.g. room tone).
    pub fn capture(audio: &[f32], sample_rate: u32, fft_size: usize) -> Self {
        assert!(fft_size.is_power_of_two());
        let num_bins = fft_size / 2 + 1;
        let mut avg_mags = vec![0.0f32; num_bins];

        if audio.is_empty() {
            return Self {
                fft_size,
                sample_rate,
                magnitudes: vec![1e-4; num_bins],
            };
        }

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(fft_size);

        // Precompute Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (fft_size as f32 - 1.0)).cos()))
            .collect();

        let hop = fft_size / 2;
        let mut num_frames = 0usize;
        let mut frame_buf = vec![0.0f32; fft_size];
        let mut output_spectrum = r2c.make_output_vec();
        let mut sum_mags = vec![0.0f32; num_bins];
        let mut sum_sq_mags = vec![0.0f32; num_bins];

        let mut offset = 0;
        while offset + fft_size <= audio.len() {
            for i in 0..fft_size {
                frame_buf[i] = audio[offset + i] * window[i];
            }

            if r2c.process(&mut frame_buf, &mut output_spectrum).is_ok() {
                let norm = 1.0 / fft_size as f32;
                for (bin, c) in output_spectrum.iter().enumerate() {
                    let m = c.norm() * norm;
                    sum_mags[bin] += m;
                    sum_sq_mags[bin] += m * m;
                }
                num_frames += 1;
            }

            offset += hop;
        }

        // If audio was shorter than fft_size, do a single zero-padded frame
        if num_frames == 0 {
            let copy_len = audio.len().min(fft_size);
            for i in 0..copy_len {
                frame_buf[i] = audio[i] * window[i];
            }
            frame_buf[copy_len..fft_size].fill(0.0);
            if r2c.process(&mut frame_buf, &mut output_spectrum).is_ok() {
                let norm = 1.0 / fft_size as f32;
                for (bin, c) in output_spectrum.iter().enumerate() {
                    let m = c.norm() * norm;
                    sum_mags[bin] += m;
                    sum_sq_mags[bin] += m * m;
                }
                num_frames = 1;
            }
        }

        let inv_frames = 1.0 / (num_frames.max(1) as f32);
        for bin in 0..num_bins {
            let mean = sum_mags[bin] * inv_frames;
            let mean_sq = sum_sq_mags[bin] * inv_frames;
            let variance = (mean_sq - mean * mean).max(0.0);
            let std_dev = variance.sqrt();
            // 95th percentile upper envelope threshold: mean + 1.8 * std_dev
            avg_mags[bin] = (mean + 1.8 * std_dev).max(1e-6);
        }

        Self {
            fft_size,
            sample_rate,
            magnitudes: avg_mags,
        }
    }
}

/// Spectral Gating Noise Reducer using Short-Time Fourier Transform (STFT) with Overlap-Add.
pub struct SpectralGate {
    profile: NoiseProfile,
    reduction_db: f32,
    sensitivity: f32,
}

impl SpectralGate {
    pub fn new(profile: NoiseProfile, reduction_db: f32, sensitivity: f32) -> Self {
        Self {
            profile,
            reduction_db: reduction_db.max(0.0),
            sensitivity: sensitivity.max(0.01),
        }
    }

    pub fn set_reduction_db(&mut self, db: f32) {
        self.reduction_db = db.max(0.0);
    }

    pub fn set_sensitivity(&mut self, s: f32) {
        self.sensitivity = s.max(0.01);
    }

    /// Process a mono audio slice using STFT overlap-add.
    pub fn process_channel(&self, input: &[f32]) -> Vec<f32> {
        let n = self.profile.fft_size;
        let hop = n / 2;

        if input.is_empty() {
            return Vec::new();
        }

        // Pad input with hop zeros at front and back for seamless COLA Hann reconstruction
        let mut padded = vec![0.0f32; input.len() + n];
        padded[hop..hop + input.len()].copy_from_slice(input);

        let mut output = vec![0.0f32; padded.len()];

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(n);
        let c2r = planner.plan_fft_inverse(n);

        // Analysis Hann window
        let window: Vec<f32> = (0..n)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (n as f32 - 1.0)).cos()))
            .collect();

        let mut time_buf = vec![0.0f32; n];
        let mut freq_buf = r2c.make_output_vec();
        let mut reconstructed = vec![0.0f32; n];

        let num_bins = n / 2 + 1;
        let inv_n = 1.0 / n as f32;
        let min_gain = 10.0f32.powf(-self.reduction_db / 20.0);

        let mut offset = 0;
        while offset + n <= padded.len() {
            // Apply analysis window
            for i in 0..n {
                time_buf[i] = padded[offset + i] * window[i];
            }

            if r2c.process(&mut time_buf, &mut freq_buf).is_ok() {
                // Spectral attenuation
                for (k, c) in freq_buf.iter_mut().enumerate().take(num_bins) {
                    let bin_mag = c.norm() * inv_n;
                    let noise_thresh = self.profile.magnitudes[k] * self.sensitivity;

                    let gain_lin = if bin_mag <= noise_thresh {
                        min_gain
                    } else {
                        let snr_inv = noise_thresh / bin_mag;
                        let wiener = (1.0 - snr_inv * snr_inv).max(0.0);
                        wiener.max(min_gain)
                    };

                    *c *= gain_lin;
                }

                if c2r.process(&mut freq_buf, &mut reconstructed).is_ok() {
                    // Overlap-add with rectangular synthesis window (COLA sum = 1.0 with 50% Hann)
                    for i in 0..n {
                        output[offset + i] += reconstructed[i] * inv_n;
                    }
                }
            }

            offset += hop;
        }

        // Extract central unpadded region matching exact input length
        output[hop..hop + input.len()].to_vec()
    }

    /// Process planar multi-channel audio in place.
    pub fn process_planar(&self, channels: &mut [Vec<f32>]) {
        for ch in channels.iter_mut() {
            let processed = self.process_channel(ch);
            *ch = processed;
        }
    }
}

/// Transient Click and Pop detector and interpolator.
pub struct ClickRemover {
    /// Detection threshold multiplier on local signal deviation (default: 3.0 - 5.0).
    pub sensitivity: f32,
    /// Maximum sample duration of an isolated click/pop to repair (default: 32).
    pub max_width: usize,
}

impl Default for ClickRemover {
    fn default() -> Self {
        Self {
            sensitivity: 3.5,
            max_width: 32,
        }
    }
}

impl ClickRemover {
    pub fn new(sensitivity: f32, max_width: usize) -> Self {
        Self {
            sensitivity: sensitivity.clamp(1.0, 10.0),
            max_width: max_width.clamp(2, 64),
        }
    }

    /// Processes a single audio channel in place, repairing detected clicks and returning count repaired.
    pub fn process_channel(&self, samples: &mut [f32]) -> usize {
        let len = samples.len();
        if len < 16 {
            return 0;
        }

        // 1. Calculate running local average absolute difference for adaptive threshold
        let mut local_diff = vec![0.0f32; len];
        for i in 1..len {
            local_diff[i] = (samples[i] - samples[i - 1]).abs();
        }

        // Compute local noise floor using 128-sample moving average
        let mut noise_floor = vec![0.0f32; len];
        let mut running_sum = 0.0f32;
        let win = 128.min(len);
        for &diff_val in local_diff.iter().take(win) {
            running_sum += diff_val;
        }
        for i in 0..len {
            if i >= win {
                running_sum += local_diff[i] - local_diff[i - win];
            }
            noise_floor[i] = (running_sum / win as f32).max(1e-4);
        }

        // 2. Multi-point moving median filter to isolate transient impulse clicks
        let mut clicks_fixed = 0usize;
        let mut i = 4;
        while i < len.saturating_sub(4) {
            // 9-point median around sample i
            let mut window = [
                samples[i - 4],
                samples[i - 3],
                samples[i - 2],
                samples[i - 1],
                samples[i],
                samples[i + 1],
                samples[i + 2],
                samples[i + 3],
                samples[i + 4],
            ];
            window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = window[4];

            let dev = (samples[i] - median).abs();
            let threshold = (self.sensitivity * noise_floor[i]).max(0.08 / self.sensitivity);

            if dev > threshold {
                // Find extent of click
                let click_start = i;
                let mut click_end = i;
                while click_end + 1 < len.saturating_sub(4)
                    && (click_end - click_start) < self.max_width
                {
                    let next_val = samples[click_end + 1];
                    let mut next_win = [
                        samples[click_end.saturating_sub(3)],
                        samples[click_end.saturating_sub(2)],
                        samples[click_end.saturating_sub(1)],
                        samples[click_end],
                        next_val,
                        samples[(click_end + 2).min(len - 1)],
                        samples[(click_end + 3).min(len - 1)],
                    ];
                    next_win.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let next_median = next_win[3];
                    if (next_val - next_median).abs() > threshold * 0.6 {
                        click_end += 1;
                    } else {
                        break;
                    }
                }

                // Interpolate using cubic Hermite spline across [click_start, click_end]
                let p0_idx = click_start.saturating_sub(1);
                let p1_idx = (click_end + 1).min(len - 1);
                let p0 = samples[p0_idx];
                let p1 = samples[p1_idx];

                let prev_p = if p0_idx > 0 { samples[p0_idx - 1] } else { p0 };
                let next_p = if p1_idx + 1 < len {
                    samples[p1_idx + 1]
                } else {
                    p1
                };

                let m0 = 0.5 * (p0 - prev_p);
                let m1 = 0.5 * (next_p - p1);

                let span = (p1_idx - p0_idx) as f32;
                for (offset, sample) in samples[click_start..=click_end].iter_mut().enumerate() {
                    let cur = click_start + offset;
                    let t = (cur - p0_idx) as f32 / span;
                    let t2 = t * t;
                    let t3 = t2 * t;

                    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                    let h10 = t3 - 2.0 * t2 + t;
                    let h01 = -2.0 * t3 + 3.0 * t2;
                    let h11 = t3 - t2;

                    *sample = h00 * p0 + h10 * m0 * span + h01 * p1 + h11 * m1 * span;
                }

                clicks_fixed += 1;
                i = click_end + 2;
            } else {
                i += 1;
            }
        }

        clicks_fixed
    }

    /// Process planar multi-channel audio in place, returning total clicks repaired across all channels.
    pub fn process_planar(&self, channels: &mut [Vec<f32>]) -> usize {
        let mut total = 0;
        for ch in channels.iter_mut() {
            total += self.process_channel(ch);
        }
        total
    }
}

/// Time-frequency bounding region for spectral repair operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectralRegion {
    pub start_frame: usize,
    pub end_frame: usize,
    pub min_freq_hz: f32,
    pub max_freq_hz: f32,
}

/// STFT overlap-add spectral repair processor for time-frequency attenuation and interpolation.
pub struct SpectralRepair;

impl SpectralRepair {
    const FFT_SIZE: usize = 2048;
    const HOP_SIZE: usize = 512;

    /// Attenuates only the frequency bins inside `region` by `attenuation_db` (negative dB reduces energy).
    pub fn attenuate_region(
        samples: &mut [f32],
        sample_rate: f32,
        region: &SpectralRegion,
        attenuation_db: f32,
    ) {
        if samples.is_empty() || region.end_frame <= region.start_frame {
            return;
        }
        let sr = sample_rate.max(1.0);
        let start = region.start_frame.min(samples.len());
        let end = region.end_frame.min(samples.len());
        if end <= start {
            return;
        }

        let f_lo = region.min_freq_hz.min(region.max_freq_hz).max(0.0);
        let f_hi = region.max_freq_hz.max(region.min_freq_hz).min(sr * 0.5);
        let bin_hz = sr / Self::FFT_SIZE as f32;
        let bin_min = ((f_lo / bin_hz).floor() as usize).min(Self::FFT_SIZE / 2);
        let bin_max = ((f_hi / bin_hz).ceil() as usize).min(Self::FFT_SIZE / 2);
        if bin_max < bin_min {
            return;
        }

        let gain = 10.0f32.powf(attenuation_db.clamp(-96.0, 12.0) / 20.0);

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(Self::FFT_SIZE);
        let c2r = planner.plan_fft_inverse(Self::FFT_SIZE);

        let window: Vec<f32> = (0..Self::FFT_SIZE)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / Self::FFT_SIZE as f32).cos())
            })
            .collect();

        let padded_len = samples.len() + Self::FFT_SIZE;
        let mut padded = vec![0.0f32; padded_len];
        padded[..samples.len()].copy_from_slice(samples);

        let mut output = vec![0.0f32; padded_len];
        let mut norm_acc = vec![0.0f32; padded_len];

        let mut time_buf = vec![0.0f32; Self::FFT_SIZE];
        let mut freq_buf = r2c.make_output_vec();
        let mut reconstructed = vec![0.0f32; Self::FFT_SIZE];

        let mut pos = 0usize;
        while pos + Self::FFT_SIZE <= padded_len {
            let frame_center = pos + Self::FFT_SIZE / 2;
            let in_time_region = frame_center >= start && frame_center < end;

            for i in 0..Self::FFT_SIZE {
                time_buf[i] = padded[pos + i] * window[i];
            }
            if r2c.process(&mut time_buf, &mut freq_buf).is_ok() {
                if in_time_region {
                    for bin in freq_buf.iter_mut().take(bin_max + 1).skip(bin_min) {
                        *bin *= gain;
                    }
                }
                if c2r.process(&mut freq_buf, &mut reconstructed).is_ok() {
                    let inv = 1.0 / Self::FFT_SIZE as f32;
                    for i in 0..Self::FFT_SIZE {
                        let w = window[i];
                        output[pos + i] += reconstructed[i] * inv * w;
                        norm_acc[pos + i] += w * w;
                    }
                }
            }
            pos += Self::HOP_SIZE;
        }

        for i in start..end {
            if norm_acc[i] > 1e-6 {
                samples[i] = output[i] / norm_acc[i];
            }
        }
    }

    /// Heals a time-frequency region by interpolating target bin magnitudes from neighboring pre/post frames.
    pub fn interpolate_region(samples: &mut [f32], sample_rate: f32, region: &SpectralRegion) {
        if samples.is_empty() || region.end_frame <= region.start_frame {
            return;
        }
        let sr = sample_rate.max(1.0);
        let start = region.start_frame.min(samples.len());
        let end = region.end_frame.min(samples.len());
        if end <= start {
            return;
        }

        let f_lo = region.min_freq_hz.min(region.max_freq_hz).max(0.0);
        let f_hi = region.max_freq_hz.max(region.min_freq_hz).min(sr * 0.5);
        let bin_hz = sr / Self::FFT_SIZE as f32;
        let bin_min = ((f_lo / bin_hz).floor() as usize).min(Self::FFT_SIZE / 2);
        let bin_max = ((f_hi / bin_hz).ceil() as usize).min(Self::FFT_SIZE / 2);
        if bin_max < bin_min {
            return;
        }

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(Self::FFT_SIZE);
        let c2r = planner.plan_fft_inverse(Self::FFT_SIZE);

        let window: Vec<f32> = (0..Self::FFT_SIZE)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / Self::FFT_SIZE as f32).cos())
            })
            .collect();

        // Capture reference magnitudes right before `start` and right after `end`
        let measure_mags = |center_frame: usize| -> Vec<f32> {
            let mut tb = vec![0.0f32; Self::FFT_SIZE];
            let half = Self::FFT_SIZE / 2;
            let begin = center_frame.saturating_sub(half);
            for (i, &w) in window.iter().enumerate() {
                let s_idx = begin + i;
                if s_idx < samples.len() {
                    tb[i] = samples[s_idx] * w;
                }
            }
            let mut fb = r2c.make_output_vec();
            let _ = r2c.process(&mut tb, &mut fb);
            fb.iter()
                .map(|c| (c.re * c.re + c.im * c.im).sqrt())
                .collect()
        };

        let pre_mags = measure_mags(start.saturating_sub(Self::HOP_SIZE));
        let post_mags = measure_mags((end + Self::HOP_SIZE).min(samples.len().saturating_sub(1)));

        let padded_len = samples.len() + Self::FFT_SIZE;
        let mut padded = vec![0.0f32; padded_len];
        padded[..samples.len()].copy_from_slice(samples);
        let mut output = vec![0.0f32; padded_len];
        let mut norm_acc = vec![0.0f32; padded_len];

        let mut time_buf = vec![0.0f32; Self::FFT_SIZE];
        let mut freq_buf = r2c.make_output_vec();
        let mut reconstructed = vec![0.0f32; Self::FFT_SIZE];

        let span = (end - start).max(1) as f32;
        let mut pos = 0usize;
        while pos + Self::FFT_SIZE <= padded_len {
            let frame_center = pos + Self::FFT_SIZE / 2;
            let in_time_region = frame_center >= start && frame_center < end;

            for i in 0..Self::FFT_SIZE {
                time_buf[i] = padded[pos + i] * window[i];
            }
            if r2c.process(&mut time_buf, &mut freq_buf).is_ok() {
                if in_time_region {
                    let alpha = ((frame_center - start) as f32 / span).clamp(0.0, 1.0);
                    for (k, bin) in freq_buf
                        .iter_mut()
                        .enumerate()
                        .take(bin_max + 1)
                        .skip(bin_min)
                    {
                        let cur_mag = (bin.re * bin.re + bin.im * bin.im).sqrt().max(1e-9);
                        let target_mag = pre_mags.get(k).copied().unwrap_or(0.0) * (1.0 - alpha)
                            + post_mags.get(k).copied().unwrap_or(0.0) * alpha;
                        let scale = (target_mag / cur_mag).clamp(0.0, 4.0);
                        *bin *= scale;
                    }
                }
                if c2r.process(&mut freq_buf, &mut reconstructed).is_ok() {
                    let inv = 1.0 / Self::FFT_SIZE as f32;
                    for i in 0..Self::FFT_SIZE {
                        let w = window[i];
                        output[pos + i] += reconstructed[i] * inv * w;
                        norm_acc[pos + i] += w * w;
                    }
                }
            }
            pos += Self::HOP_SIZE;
        }

        for i in start..end {
            if norm_acc[i] > 1e-6 {
                samples[i] = output[i] / norm_acc[i];
            }
        }
    }
}

/// Removes mains hum (50 Hz or 60 Hz) and its first 3 harmonics using cascaded narrow notch filters.
pub struct HumRemover {
    notches: [crate::biquad::BiquadFilter; 4],
}

impl HumRemover {
    pub fn new(sample_rate: f32, base_freq_hz: f32, q: f32) -> Self {
        let sr = sample_rate.max(100.0);
        let base = base_freq_hz.clamp(20.0, 120.0);
        let q_clamped = q.clamp(1.0, 30.0);
        let make_notch = |freq: f32| {
            crate::biquad::BiquadFilter::new(crate::biquad::BiquadCoeffs::design(
                stem_core::FilterType::Peaking,
                sr,
                freq.min(sr * 0.45),
                q_clamped,
                -36.0,
            ))
        };
        let notches = [
            make_notch(base),
            make_notch(base * 2.0),
            make_notch(base * 3.0),
            make_notch(base * 4.0),
        ];
        Self { notches }
    }

    pub fn process_channel(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            let mut val = *s;
            for f in &mut self.notches {
                val = f.process_sample(val);
            }
            *s = val;
        }
    }
}

/// Reconstructs clipped flat-top peaks using cubic Hermite interpolation from surrounding slopes.
pub struct DeClipper {
    pub threshold: f32,
}

impl DeClipper {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.5, 0.999),
        }
    }

    pub fn process_channel(&self, samples: &mut [f32]) -> usize {
        if samples.len() < 8 {
            return 0;
        }
        let mut repaired = 0usize;
        let mut i = 2usize;
        while i + 2 < samples.len() {
            if samples[i].abs() >= self.threshold {
                let start = i;
                while i < samples.len() - 2 && samples[i].abs() >= self.threshold {
                    i += 1;
                }
                let end = i.saturating_sub(1);
                if end >= start && start >= 2 && end + 2 < samples.len() {
                    let p0_idx = start - 1;
                    let p1_idx = end + 1;
                    let p0 = samples[p0_idx];
                    let p1 = samples[p1_idx];
                    let m0 = p0 - samples[start - 2];
                    let m1 = samples[end + 2] - p1;
                    let span = (p1_idx - p0_idx) as f32;
                    for (off, s) in samples[start..=end].iter_mut().enumerate() {
                        let t = (start + off - p0_idx) as f32 / span;
                        let t2 = t * t;
                        let t3 = t2 * t;
                        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                        let h10 = t3 - 2.0 * t2 + t;
                        let h01 = -2.0 * t3 + 3.0 * t2;
                        let h11 = t3 - t2;
                        *s = h00 * p0 + h10 * m0 * span + h01 * p1 + h11 * m1 * span;
                    }
                    repaired += 1;
                }
            } else {
                i += 1;
            }
        }
        repaired
    }
}

/// Envelope-tracking broadband de-reverberation processor that attenuates decaying room tails.
pub struct DeReverberator {
    sample_rate: f32,
    reduction: f32,
}

impl DeReverberator {
    pub fn new(sample_rate: f32, reduction: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(100.0),
            reduction: reduction.clamp(0.0, 0.95),
        }
    }

    pub fn process_channel(&self, samples: &mut [f32]) {
        let fast_att = (-1.0 / (self.sample_rate * 0.002)).exp();
        let fast_rel = (-1.0 / (self.sample_rate * 0.035)).exp();
        let slow_rel = (-1.0 / (self.sample_rate * 0.250)).exp();

        let mut fast_env = 0.0f32;
        let mut slow_env = 0.0f32;
        for s in samples.iter_mut() {
            let abs = s.abs();
            fast_env = if abs > fast_env {
                fast_att * fast_env + (1.0 - fast_att) * abs
            } else {
                fast_rel * fast_env + (1.0 - fast_rel) * abs
            };
            slow_env = if abs > slow_env {
                fast_att * slow_env + (1.0 - fast_att) * abs
            } else {
                slow_rel * slow_env + (1.0 - slow_rel) * abs
            };

            // When slow tail energy exceeds fast direct energy, gently attenuate tail
            if slow_env > fast_env * 1.15 && slow_env > 1e-5 {
                let ratio = (fast_env / slow_env).clamp(0.0, 1.0);
                let gain = 1.0 - self.reduction * (1.0 - ratio);
                *s *= gain;
            }
        }
    }
}
