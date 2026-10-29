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
