use stem_core::project::FilterType;
use stem_dsp::analysis::analyze_loudness;
use stem_dsp::biquad::{BiquadCoeffs, BiquadFilter};
use stem_dsp::dither::TpdfDither;
use stem_dsp::dynamics::{Compressor, Limiter};
use stem_dsp::peaks::build_peak_pyramid;

#[test]
fn test_tpdf_dither_bounds() {
    let mut dither = TpdfDither::new();
    for _ in 0..10_000 {
        let val = dither.next_tpdf();
        assert!(
            (-1.0..=1.0).contains(&val),
            "TPDF sample out of bounds [-1.0, 1.0]: {val}"
        );
    }

    // Quantization bounds (within 1 LSB of full scale due to dither)
    let q_pos = dither.quantize_i16(1.0);
    assert!(q_pos >= 32766);
    let q_neg = dither.quantize_i16(-1.0);
    assert!(q_neg <= -32766);
}

#[test]
fn test_biquad_lowpass_attenuation() {
    let sample_rate = 48000.0;
    let cutoff = 1000.0;
    let coeffs = BiquadCoeffs::design(FilterType::LowPass, sample_rate, cutoff, 0.707, 0.0);
    let mut filter = BiquadFilter::new(coeffs);

    // High frequency (10 kHz) sine wave should be heavily attenuated
    let freq = 10000.0;
    let mut out_max = 0.0f32;
    for i in 0..1000 {
        let t = i as f32 / sample_rate;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin();
        let out = filter.process_sample(s);
        if i > 200 && out.abs() > out_max {
            out_max = out.abs();
        }
    }

    // 10 kHz is well into the stopband (>3 octaves above 1 kHz cutoff, >36 dB attenuation)
    assert!(
        out_max < 0.15,
        "High frequency was not sufficiently attenuated: peak={out_max}"
    );
}

#[test]
fn test_compressor_gain_reduction() {
    let mut comp = Compressor::new(48000.0, -20.0, 4.0, 10.0, 100.0, 0.0);

    // Feed a loud signal at 0 dBFS (amplitude 1.0)
    let mut max_gr = 0.0f32;
    for _ in 0..1000 {
        let (_, gr) = comp.process_sample(1.0);
        if gr > max_gr {
            max_gr = gr;
        }
    }

    // Input is 0 dBFS, threshold is -20 dBFS. Overhang is 20 dB.
    // Ratio 4:1 gives compressed overhang of 5 dB.
    // Gain reduction should approach 15 dB.
    assert!(
        max_gr > 12.0 && max_gr <= 16.0,
        "Expected ~15 dB gain reduction, got {max_gr} dB"
    );
}

#[test]
fn test_limiter_brickwall() {
    let mut limiter = Limiter::new(48000.0, -1.0, 50.0); // -1.0 dBFS limit

    // Feed 6 dB over (amplitude 2.0)
    for _ in 0..1000 {
        let out = limiter.process_sample(2.0);
        assert!(
            out.abs() <= 1.01,
            "Limiter allowed signal over 0 dBFS: {out}"
        );
    }
}

#[test]
fn test_peak_pyramid_hierarchy() {
    // Generate 10,000 samples with a known peak at frame 5000
    let mut ch = vec![0.0f32; 10000];
    ch[5000] = 0.85;
    ch[5001] = -0.92;

    let pyramid = build_peak_pyramid(&[&ch]);
    assert_eq!(pyramid.total_frames, 10000);
    assert_eq!(pyramid.num_channels, 1);
    assert_eq!(pyramid.levels.len(), 5);

    // Query 100 display points
    let points = pyramid.query_channel_peaks(0, 0, 10000, 100);
    assert_eq!(points.len(), 100);

    // At least one point must capture the max 0.85 and min -0.92
    let overall_max = points.iter().map(|p| p.max).fold(0.0f32, f32::max);
    let overall_min = points.iter().map(|p| p.min).fold(0.0f32, f32::min);

    assert!(
        (overall_max - 0.85).abs() < 1e-4,
        "Peak max not preserved: {overall_max}"
    );
    assert!(
        (overall_min - (-0.92)).abs() < 1e-4,
        "Peak min not preserved: {overall_min}"
    );
}

#[test]
fn test_ebur128_loudness_analysis() {
    // 1 second of 1 kHz sine wave at -18 dBFS stereo
    let sample_rate = 48000;
    let amplitude = 10.0f32.powf(-18.0 / 20.0);
    let mut interleaved = Vec::with_capacity(sample_rate as usize * 2);

    for i in 0..sample_rate {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amplitude;
        interleaved.push(s);
        interleaved.push(s);
    }

    let result = analyze_loudness(&interleaved, 2, sample_rate).expect("EBU R128 analysis");
    // Standard 1 kHz sine at -18 dBFS reads approximately -21 LUFS
    assert!(
        result.integrated_lufs > -25.0 && result.integrated_lufs < -17.0,
        "Unexpected integrated LUFS: {}",
        result.integrated_lufs
    );
}

#[test]
fn test_itu_r_bs1770_reference_signal() {
    // ITU-R BS.1770-4 calibration reference:
    // 1. A 1000 Hz full-scale stereo sine wave of amplitude 1.0 in BOTH channels (in phase)
    // has power = 0.5 + 0.5 = 1.0 (0.0 dBFS), yielding integrated loudness of 0.0 LUFS +/- 0.1 LUFS.
    let sample_rate = 48000;
    let duration_secs = 2.0;
    let num_frames = (sample_rate as f64 * duration_secs) as usize;
    let mut stereo_full = Vec::with_capacity(num_frames * 2);

    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        stereo_full.push(s);
        stereo_full.push(s);
    }

    let res_full =
        analyze_loudness(&stereo_full, 2, sample_rate).expect("Loudness full-scale stereo test");
    assert!(
        (res_full.integrated_lufs - 0.0).abs() < 0.15,
        "Expected stereo 0 dBFS peak to yield ~0.0 LUFS, got {:.2} LUFS",
        res_full.integrated_lufs
    );

    // 2. A 1000 Hz sine wave of amplitude 1.0 in LEFT channel only (right silent)
    // has power = 0.5 (-3.01 dBFS), yielding integrated loudness of -3.01 LUFS +/- 0.1 LUFS.
    let mut mono_full = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        mono_full.push(s);
        mono_full.push(0.0);
    }
    let res_mono =
        analyze_loudness(&mono_full, 2, sample_rate).expect("Loudness mono reference test");
    assert!(
        (res_mono.integrated_lufs - (-3.01)).abs() < 0.15,
        "Expected single-channel 0 dBFS peak to yield ~ -3.01 LUFS, got {:.2} LUFS",
        res_mono.integrated_lufs
    );

    // 3. Calibration signal targeting broadcast -23 LUFS
    let amp_23 = 10.0f32.powf(-23.0 / 20.0);
    let mut interleaved_23 = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amp_23;
        interleaved_23.push(s);
        interleaved_23.push(s);
    }
    let res_23 = analyze_loudness(&interleaved_23, 2, sample_rate).expect("Loudness -23 LUFS test");
    assert!(
        (res_23.integrated_lufs - (-23.0)).abs() < 0.15,
        "Expected -23.0 LUFS target, got {:.2} LUFS",
        res_23.integrated_lufs
    );
}

#[test]
fn test_tpdf_dither_statistics() {
    let mut dither = TpdfDither::new();
    let n = 100_000;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut min = 1.0f32;
    let mut max = -1.0f32;

    for _ in 0..n {
        let s = dither.next_tpdf();
        assert!((-1.0..=1.0).contains(&s), "TPDF sample out of range: {s}");
        if s < min {
            min = s;
        }
        if s > max {
            max = s;
        }
        sum += s as f64;
        sum_sq += (s as f64) * (s as f64);
    }

    let mean = sum / (n as f64);
    let variance = (sum_sq / (n as f64)) - (mean * mean);

    // TPDF mean must be near 0
    assert!(mean.abs() < 0.008, "TPDF mean should be ~0.0, got {mean}");

    // Theoretical variance of triangular distribution with width 2 (-1 to +1) is 1/6 ~= 0.16667
    assert!(
        (variance - (1.0 / 6.0)).abs() < 0.015,
        "TPDF variance should be ~0.1667, got {variance}"
    );

    // Range should span close to -1.0 and +1.0
    assert!(min < -0.95, "TPDF minimum did not approach -1.0: {min}");
    assert!(max > 0.95, "TPDF maximum did not approach +1.0: {max}");
}

#[test]
fn test_resampling_round_trip() {
    let src_rate = 44100;
    let dst_rate = 48000;
    let duration_frames = src_rate as usize; // 1 second

    let mut left = Vec::with_capacity(duration_frames);
    let mut right = Vec::with_capacity(duration_frames);

    for i in 0..duration_frames {
        let t = i as f32 / src_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.75;
        left.push(s);
        right.push(s);
    }

    let input = vec![left, right];

    // Resample 44.1 kHz -> 48.0 kHz
    let upsampled = stem_dsp::resample_audio(&input, src_rate, dst_rate)
        .expect("Upsampling 44.1k -> 48k should succeed");
    let expected_up_frames =
        (duration_frames as f64 * dst_rate as f64 / src_rate as f64).round() as usize;
    assert!(
        (upsampled[0].len() as i64 - expected_up_frames as i64).abs() < 50,
        "Upsampled frame count mismatch"
    );

    // Resample back 48.0 kHz -> 44.1 kHz
    let roundtripped = stem_dsp::resample_audio(&upsampled, dst_rate, src_rate)
        .expect("Downsampling 48k -> 44.1k should succeed");

    // Check roundtrip duration within sinc filter boundary margins
    assert!(
        (roundtripped[0].len() as i64 - duration_frames as i64).abs() < 100,
        "Roundtripped frame count mismatch"
    );

    // Anti-aliasing FIR resamplers introduce a small linear phase group delay (latency).
    // Calculate peak cross-correlation across lags -30..=30 to align filter latency.
    let check_start = 500;
    let check_end = 2000;
    let mut max_corr = 0.0f64;

    for lag in -30..=30 {
        let mut dot = 0.0f64;
        let mut norm_in = 0.0f64;
        let mut norm_out = 0.0f64;

        for (i, &in_val) in input[0]
            .iter()
            .enumerate()
            .take(check_end)
            .skip(check_start)
        {
            let out_idx = (i as isize + lag) as usize;
            if out_idx < roundtripped[0].len() {
                let a = in_val as f64;
                let b = roundtripped[0][out_idx] as f64;
                dot += a * b;
                norm_in += a * a;
                norm_out += b * b;
            }
        }

        let corr = dot / (norm_in.sqrt() * norm_out.sqrt());
        if corr > max_corr {
            max_corr = corr;
        }
    }

    assert!(
        max_corr > 0.999,
        "Resampling round trip aligned correlation too low: {max_corr}"
    );

    // Check amplitude preservation on central steady portion
    let mut orig_peak = 0.0f32;
    let mut rt_peak = 0.0f32;
    for i in 1000..5000 {
        orig_peak = orig_peak.max(input[0][i].abs());
        rt_peak = rt_peak.max(roundtripped[0][i].abs());
    }
    assert!(
        (orig_peak - rt_peak).abs() < 0.01,
        "Resampling amplitude loss: orig={orig_peak}, rt={rt_peak}"
    );
}

#[test]
fn test_peak_cache_exact_correctness() {
    let num_frames = 16384;
    let mut buf = vec![0.0f32; num_frames];

    // Place spikes in distinct buckets
    buf[42] = 0.77;
    buf[43] = -0.66;
    buf[200] = 0.95;
    buf[201] = -0.88;
    buf[5000] = 0.45;
    buf[5001] = -0.32;
    buf[16000] = 0.99;
    buf[16001] = -0.99;

    let pyramid = build_peak_pyramid(&[&buf]);

    // Check every pyramid level against exact slice min/max
    let bucket_sizes = [64, 256, 1024, 4096, 16384];
    for (lvl_idx, &bucket_size) in bucket_sizes.iter().enumerate() {
        let level = &pyramid.levels[lvl_idx];
        let num_buckets = num_frames.div_ceil(bucket_size);

        assert_eq!(level.channels[0].len(), num_buckets);

        for b in 0..num_buckets {
            let start = b * bucket_size;
            let end = (start + bucket_size).min(num_frames);
            let slice = &buf[start..end];

            let mut expected_min = 0.0f32;
            let mut expected_max = 0.0f32;
            for &s in slice {
                if s < expected_min {
                    expected_min = s;
                }
                if s > expected_max {
                    expected_max = s;
                }
            }

            let peak = level.channels[0][b];
            assert!(
                (peak.min - expected_min).abs() < 1e-6,
                "Lvl {lvl_idx} bucket {b} min mismatch: expected {expected_min}, got {}",
                peak.min
            );
            assert!(
                (peak.max - expected_max).abs() < 1e-6,
                "Lvl {lvl_idx} bucket {b} max mismatch: expected {expected_max}, got {}",
                peak.max
            );
        }
    }
}

#[test]
fn test_biquad_eq_math() {
    let sample_rate = 48000.0;

    // 1. Peaking filter: 1 kHz, Q=1.0, +6 dB boost
    let peak_coeffs = BiquadCoeffs::design(FilterType::Peaking, sample_rate, 1000.0, 1.0, 6.0);
    let gain_at_center = peak_coeffs.magnitude_response_db(1000.0, sample_rate);
    let gain_at_dc = peak_coeffs.magnitude_response_db(20.0, sample_rate);
    let gain_at_hf = peak_coeffs.magnitude_response_db(20000.0, sample_rate);

    assert!(
        (gain_at_center - 6.0).abs() < 0.1,
        "Peaking center gain should be +6 dB: got {gain_at_center}"
    );
    assert!(
        gain_at_dc.abs() < 0.2,
        "Peaking DC gain should be ~0 dB: got {gain_at_dc}"
    );
    assert!(
        gain_at_hf.abs() < 0.2,
        "Peaking HF gain should be ~0 dB: got {gain_at_hf}"
    );

    // 2. Low Shelf filter: 200 Hz, Q=0.707, +6 dB boost
    let ls_coeffs = BiquadCoeffs::design(FilterType::LowShelf, sample_rate, 200.0, 0.707, 6.0);
    let gain_low = ls_coeffs.magnitude_response_db(40.0, sample_rate);
    let gain_high = ls_coeffs.magnitude_response_db(5000.0, sample_rate);

    assert!(
        (gain_low - 6.0).abs() < 0.2,
        "Low shelf low gain should be +6 dB: got {gain_low}"
    );
    assert!(
        gain_high.abs() < 0.2,
        "Low shelf high gain should be ~0 dB: got {gain_high}"
    );

    // 3. High Shelf filter: 5000 Hz, Q=0.707, -6 dB cut
    let hs_coeffs = BiquadCoeffs::design(FilterType::HighShelf, sample_rate, 5000.0, 0.707, -6.0);
    let gain_hs_low = hs_coeffs.magnitude_response_db(100.0, sample_rate);
    let gain_hs_high = hs_coeffs.magnitude_response_db(15000.0, sample_rate);

    assert!(
        gain_hs_low.abs() < 0.2,
        "High shelf low gain should be ~0 dB: got {gain_hs_low}"
    );
    assert!(
        (gain_hs_high - (-6.0)).abs() < 0.2,
        "High shelf high gain should be -6 dB: got {gain_hs_high}"
    );
}

#[test]
fn test_parameter_smoothing_continuity() {
    let sample_rate = 48000.0;
    // 20ms smoothing constant
    let mut param = stem_dsp::chain::SmoothedParam::new(0.0, sample_rate, 20.0);

    // Sudden jump target from 0.0 to 1.0
    param.set_target(1.0);

    let mut prev = 0.0f32;
    let mut max_delta = 0.0f32;

    for _ in 0..1000 {
        let current = param.next();
        let delta = (current - prev).abs();
        if delta > max_delta {
            max_delta = delta;
        }
        // No discontinuous jump allowed!
        assert!(
            delta < 0.01,
            "Discontinuous parameter jump detected: delta={delta}"
        );
        prev = current;
    }

    // After 1000 samples (~20.8 ms = ~1 time constant), param should have reached ~63.2% of target
    assert!(
        (param.current() - 1.0).abs() < 0.40,
        "Param did not reach expected target: current={}",
        param.current()
    );

    // Run 3000 more samples (total 4000 samples ≈ 83.3 ms = ~4.16 time constants)
    for _ in 0..3000 {
        param.next();
    }
    assert!(
        (param.current() - 1.0).abs() < 0.02,
        "Param should be within 0.02 of 1.0: got {}",
        param.current()
    );

    // After 8000 samples (>7 time constants), param should be virtually at 1.0
    for _ in 0..4000 {
        param.next();
    }
    assert!(
        (param.current() - 1.0).abs() < 0.001,
        "Param should be virtually at 1.0: got {}",
        param.current()
    );
}

#[test]
fn test_effect_chain_bypass_bit_exact_passthrough() {
    let sample_rate = 48000.0;
    let mut chain = stem_dsp::chain::EffectChain::new();

    let mut eq = stem_dsp::chain::EqEffect::new(sample_rate);
    eq.bypassed = true;
    chain.add_effect(stem_dsp::chain::InsertEffect::Equalizer(eq));

    let mut comp =
        stem_dsp::chain::CompressorEffect::new(sample_rate, -20.0, 4.0, 10.0, 100.0, 6.0);
    comp.bypassed = true;
    chain.add_effect(stem_dsp::chain::InsertEffect::Compressor(comp));

    let mut lim = stem_dsp::chain::LimiterEffect::new(sample_rate, -1.0, 50.0);
    lim.bypassed = true;
    chain.add_effect(stem_dsp::chain::InsertEffect::Limiter(lim));

    let mut delay = stem_dsp::chain::DelayEffect::new(sample_rate, 250.0, 0.5, 0.5);
    delay.bypassed = true;
    chain.add_effect(stem_dsp::chain::InsertEffect::Delay(delay));

    let mut reverb = stem_dsp::chain::ReverbEffect::new(sample_rate, 0.8, 0.5, 0.3);
    reverb.bypassed = true;
    chain.add_effect(stem_dsp::chain::InsertEffect::Reverb(reverb));

    // Feed a non-trivial audio signal
    for i in 0..5000 {
        let t = i as f32 / sample_rate;
        let in_l = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.75;
        let in_r = (2.0 * std::f32::consts::PI * 880.0 * t).cos() * 0.65;

        let (out_l, out_r) = chain.process_stereo(in_l, in_r);

        // Bit-exact identity check
        assert_eq!(
            in_l.to_bits(),
            out_l.to_bits(),
            "Left channel bit mismatch on bypassed chain at sample {i}"
        );
        assert_eq!(
            in_r.to_bits(),
            out_r.to_bits(),
            "Right channel bit mismatch on bypassed chain at sample {i}"
        );
    }
}

#[test]
fn test_effect_chain_order_strictly_respected() {
    let sample_rate = 48000.0;

    // Chain 1: [EQ +18 dB boost at 1 kHz] THEN [Limiter at -20 dB = 0.1 linear]
    let mut chain_eq_then_lim = stem_dsp::chain::EffectChain::new();
    let mut eq1 = stem_dsp::chain::EqEffect::new(sample_rate);
    eq1.set_band(
        1,
        stem_core::project::EqBandConfig {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain_db: 18.0,
            q: 2.0,
        },
    );
    chain_eq_then_lim.add_effect(stem_dsp::chain::InsertEffect::Equalizer(eq1));
    chain_eq_then_lim.add_effect(stem_dsp::chain::InsertEffect::Limiter(
        stem_dsp::chain::LimiterEffect::new(sample_rate, -20.0, 10.0),
    ));

    // Chain 2: [Limiter at -20 dB = 0.1 linear] THEN [EQ +18 dB boost at 1 kHz]
    let mut chain_lim_then_eq = stem_dsp::chain::EffectChain::new();
    chain_lim_then_eq.add_effect(stem_dsp::chain::InsertEffect::Limiter(
        stem_dsp::chain::LimiterEffect::new(sample_rate, -20.0, 10.0),
    ));
    let mut eq2 = stem_dsp::chain::EqEffect::new(sample_rate);
    eq2.set_band(
        1,
        stem_core::project::EqBandConfig {
            enabled: true,
            filter_type: FilterType::Peaking,
            frequency: 1000.0,
            gain_db: 18.0,
            q: 2.0,
        },
    );
    chain_lim_then_eq.add_effect(stem_dsp::chain::InsertEffect::Equalizer(eq2));

    let mut peak_chain1 = 0.0f32;
    let mut peak_chain2 = 0.0f32;

    for i in 0..4000 {
        let t = i as f32 / sample_rate;
        let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;

        let (out1, _) = chain_eq_then_lim.process_stereo(s, s);
        let (out2, _) = chain_lim_then_eq.process_stereo(s, s);

        if i > 1000 {
            if out1.abs() > peak_chain1 {
                peak_chain1 = out1.abs();
            }
            if out2.abs() > peak_chain2 {
                peak_chain2 = out2.abs();
            }
        }
    }

    // In Chain 1 (EQ -> Limiter), the limiter clamps the post-EQ signal to ~0.1
    assert!(
        peak_chain1 < 0.15,
        "Chain 1 (EQ -> Limiter) failed to clamp to limiter ceiling: peak={peak_chain1}"
    );

    // In Chain 2 (Limiter -> EQ), limiter clamps to 0.1, THEN EQ boosts +18 dB (~8x) to ~0.8
    assert!(
        peak_chain2 > 0.6,
        "Chain 2 (Limiter -> EQ) failed to boost post-limiter signal: peak={peak_chain2}"
    );

    // Peak 2 must be significantly greater than Peak 1 (> 4x difference)
    assert!(
        peak_chain2 > peak_chain1 * 4.0,
        "Effect chain order was not respected: peak1={peak_chain1}, peak2={peak_chain2}"
    );
}

#[test]
fn test_spectral_noise_reduction_measurably_attenuates_noise() {
    let sample_rate = 44100;
    let fft_size = 1024;
    let num_samples = 44100; // 1 second

    // Synthesize signal:
    // First 0.5s: pure Gaussian/white noise floor (std dev 0.05, ~ -26 dBFS)
    // Second 0.5s: 1 kHz sine wave (amp 0.5) + noise floor
    let mut signal = vec![0.0f32; num_samples];
    let mut rng_state: u64 = 123456789;

    for (i, s) in signal.iter_mut().enumerate() {
        // Simple LCG PRNG for reproducible white noise
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let n1 = (rng_state >> 32) as f32 / u32::MAX as f32 - 0.5;
        let noise = n1 * 0.1; // peak approx 0.05

        let t = i as f32 / sample_rate as f32;
        let tone = if i >= num_samples / 2 {
            (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5
        } else {
            0.0
        };

        *s = tone + noise;
    }

    // 1. Capture noise profile from the first 0.4s (noise only)
    let noise_selection = &signal[0..17640];
    let profile = stem_dsp::NoiseProfile::capture(noise_selection, sample_rate, fft_size);

    // 2. Build spectral gate with 24 dB reduction
    let gate = stem_dsp::SpectralGate::new(profile, 24.0, 1.2);

    let processed = gate.process_channel(&signal);
    assert_eq!(processed.len(), signal.len(), "Processed length mismatch");

    // Measure RMS power of noise-only segment before and after
    let noise_eval_range = 2000..16000;
    let mut orig_noise_energy = 0.0f64;
    let mut proc_noise_energy = 0.0f64;

    for i in noise_eval_range.clone() {
        orig_noise_energy += (signal[i] as f64) * (signal[i] as f64);
        proc_noise_energy += (processed[i] as f64) * (processed[i] as f64);
    }

    let orig_rms = (orig_noise_energy / noise_eval_range.len() as f64).sqrt();
    let proc_rms = (proc_noise_energy / noise_eval_range.len() as f64).sqrt();

    let reduction_achieved_db = 20.0 * (orig_rms / proc_rms.max(1e-9)).log10();

    assert!(
        reduction_achieved_db >= 14.0,
        "Noise reduction should achieve at least 14 dB attenuation, got {reduction_achieved_db:.2} dB"
    );

    // Verify tone section (second half) remains strong
    let tone_eval_range = 25000..40000;
    let mut proc_tone_peak = 0.0f32;
    for i in tone_eval_range {
        proc_tone_peak = proc_tone_peak.max(processed[i].abs());
    }
    assert!(
        proc_tone_peak > 0.4,
        "Desired audio tone was excessively degraded: peak={proc_tone_peak}"
    );
}

#[test]
fn test_click_and_pop_remover_restores_transient_impulses() {
    let sample_rate = 44100;
    let num_samples = 44100;
    let mut audio = vec![0.0f32; num_samples];

    // Clean 220 Hz sine tone
    for (i, sample) in audio.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        *sample = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.4;
    }

    // Inject 5 artificial clicks (sharp high-derivative spikes)
    let click_positions = [3000, 8500, 15000, 24000, 36000];
    for &pos in &click_positions {
        for offset in 0..3 {
            audio[pos + offset] = 0.95; // spike
        }
    }

    let declicker = stem_dsp::ClickRemover::new(4.0, 16);
    let mut restored = audio.clone();
    let clicks_repaired = declicker.process_channel(&mut restored);

    assert!(
        clicks_repaired >= click_positions.len(),
        "Expected at least {} clicks repaired, got {clicks_repaired}",
        click_positions.len()
    );

    // Verify that the sharp spikes at click positions have been interpolated smoothly
    for &pos in &click_positions {
        let max_spike = restored[pos..pos + 3]
            .iter()
            .fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(
            max_spike < 0.65,
            "Click at sample {pos} was not suppressed: spike amplitude = {max_spike}"
        );

        // Verify continuity with neighbor
        let step = (restored[pos] - restored[pos - 1]).abs();
        assert!(
            step < 0.15,
            "Discontinuity at repaired click boundary {pos}: step={step}"
        );
    }
}

#[test]
fn test_spectrogram_color_palette_and_true_peak() {
    let palette = stem_dsp::SpectrogramPalette::Viridis;
    let (r0, g0, b0) = palette.color_rgb(0.0);
    let (r1, g1, b1) = palette.color_rgb(1.0);
    // Low vs High should be distinct colors
    assert_ne!((r0, g0, b0), (r1, g1, b1));

    // True peak test: high inter-sample peaks detected
    let samples = vec![0.0, 0.707, -0.707, 0.0];
    let tp = stem_dsp::compute_true_peak_dbfs(&samples);
    assert!(
        tp > -5.0 && tp <= 0.0,
        "True peak calculation returned invalid value: {tp}"
    );
}

#[test]
fn test_spatial_delay_and_reverb_stereo_decorrelation() {
    let mut delay = stem_dsp::Delay::new(48000.0, 500.0);
    delay.set_params(10.0, 0.5, 0.5);
    let mut impulse_echo_peak = 0.0f32;
    for i in 0..1000 {
        let inp = if i == 0 { 1.0 } else { 0.0 };
        let out = delay.process_sample(inp);
        if i > 400 {
            impulse_echo_peak = impulse_echo_peak.max(out.abs());
        }
    }
    assert!(
        impulse_echo_peak > 0.1,
        "Delay did not produce expected echo"
    );

    let mut rev_l = stem_dsp::Reverb::new(48000.0);
    let mut rev_r = stem_dsp::Reverb::new_with_spread(48000.0, 23);
    rev_l.set_params(0.7, 0.3, 1.0);
    rev_r.set_params(0.7, 0.3, 1.0);
    let mut diff_energy = 0.0f32;
    for i in 0..4000 {
        let inp = if i == 0 { 1.0 } else { 0.0 };
        let ol = rev_l.process_sample(inp);
        let or = rev_r.process_sample(inp);
        diff_energy += (ol - or).abs();
    }
    assert!(
        diff_energy > 0.01,
        "Stereo spread reverb should decorrelate L/R channels"
    );
}

#[test]
fn test_ebur128_momentary_max_captures_peak_before_silence() {
    let sr = 48000u32;
    // 1 second loud 1 kHz tone followed by 2 seconds of silence
    let mut samples = Vec::with_capacity(sr as usize * 3 * 2);
    for i in 0..(sr as usize * 3) {
        let val = if i < sr as usize {
            (2.0 * std::f32::consts::PI * 1000.0 * (i as f32 / sr as f32)).sin() * 0.5
        } else {
            0.0
        };
        samples.push(val);
        samples.push(val);
    }

    let res = stem_dsp::analyze_loudness(&samples, 2, sr).unwrap();
    assert!(
        res.momentary_max_lufs > -20.0,
        "Expected momentary max > -20 LUFS even with trailing silence, got {}",
        res.momentary_max_lufs
    );
}

#[test]
fn test_effect_chain_from_configs_applies_effects() {
    let configs = vec![stem_core::EffectConfig::Limiter {
        threshold_db: -12.0,
        release_ms: 50.0,
    }];
    let mut chain = stem_dsp::chain::EffectChain::from_configs(&configs, 48000.0);
    assert_eq!(chain.effects.len(), 1);
    let (l, r) = chain.process_stereo(0.95, -0.95);
    assert!(
        l.abs() < 0.3 && r.abs() < 0.3,
        "Limiter built via from_configs should attenuate 0.95 input below -12 dBFS: l={l}, r={r}"
    );
}

#[test]
fn test_spectral_repair_and_restoration_tools() {
    let sr = 48000.0f32;
    let n = 8192usize;

    // 1. SpectralRepair::attenuate_region should suppress 2500 Hz while leaving 200 Hz intact
    let mut sig = vec![0.0f32; n];
    for (i, s) in sig.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.4
            + (2.0 * std::f32::consts::PI * 2500.0 * t).sin() * 0.4;
    }
    let region = stem_dsp::SpectralRegion {
        start_frame: 2048,
        end_frame: 6144,
        min_freq_hz: 2200.0,
        max_freq_hz: 2800.0,
    };
    let mut attenuated = sig.clone();
    stem_dsp::SpectralRepair::attenuate_region(&mut attenuated, sr, &region, -40.0);
    let rms_before: f32 = sig[3000..5000].iter().map(|x| x * x).sum();
    let rms_after: f32 = attenuated[3000..5000].iter().map(|x| x * x).sum();
    assert!(
        rms_after < rms_before * 0.65,
        "Spectral attenuation should reduce 2.5 kHz component energy: before={rms_before}, after={rms_after}"
    );

    // 2. SpectralRepair::interpolate_region should heal a burst in the middle
    let mut burst = sig.clone();
    for s in &mut burst[3500..4500] {
        *s += 0.5;
    }
    stem_dsp::SpectralRepair::interpolate_region(&mut burst, sr, &region);
    assert!(burst[3500..4500].iter().all(|x| x.is_finite()));

    // 3. HumRemover should attenuate 50 Hz hum
    let mut hum_sig: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * 50.0 * (i as f32 / sr)).sin() * 0.5)
        .collect();
    let mut hum = stem_dsp::HumRemover::new(sr, 50.0, 6.0);
    hum.process_channel(&mut hum_sig);
    let tail_peak = hum_sig[n - 1024..]
        .iter()
        .fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!(
        tail_peak < 0.1,
        "HumRemover should attenuate 50 Hz mains hum, got tail_peak={tail_peak}"
    );

    // 4. DeClipper should repair flat-topped clipped samples
    let mut clipped: Vec<f32> = (0..1024)
        .map(|i| {
            ((2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr)).sin() * 1.25).clamp(-0.98, 0.98)
        })
        .collect();
    let declip = stem_dsp::DeClipper::new(0.97);
    let fixed = declip.process_channel(&mut clipped);
    assert!(
        fixed > 0,
        "DeClipper should detect and repair clipped peaks"
    );

    // 5. DeReverberator should attenuate decaying tail
    let mut reverb_sig = vec![0.0f32; sr as usize];
    for (i, s) in reverb_sig.iter_mut().enumerate() {
        let t = i as f32 / sr;
        *s = if i < 2000 {
            0.8
        } else {
            (-t * 6.0).exp() * 0.4
        };
    }
    let dereverb = stem_dsp::DeReverberator::new(sr, 0.7);
    let mut out = reverb_sig.clone();
    dereverb.process_channel(&mut out);
    assert!(out[8000].abs() <= reverb_sig[8000].abs());
}

#[test]
fn test_full_dynamics_and_mastering_assistant() {
    let sr = 48000u32;
    let mut interleaved = Vec::with_capacity(sr as usize * 2);
    for i in 0..sr as usize {
        let t = i as f32 / sr as f32;
        let s = (2.0 * std::f32::consts::PI * 500.0 * t).sin() * 0.5;
        interleaved.push(s);
        interleaved.push(s);
    }

    let report = stem_dsp::MasteringAssistant::analyze_mix(&interleaved, 2, sr, -14.0)
        .expect("Mastering analysis should succeed");
    assert!(report.dynamics.rms_dbfs < 0.0 && report.dynamics.rms_dbfs > -20.0);
    assert!(report.dynamics.crest_factor_db > 2.0 && report.dynamics.crest_factor_db < 5.0);
    assert_eq!(report.dynamics.clipped_samples, 0);
    assert!(report.tonal_balance.mid_pct > 70.0);
}

#[test]
fn test_sidechain_stereo_width_and_surround_51() {
    // 1. Sidechain compression ducks input when key exceeds threshold
    let mut comp = stem_dsp::Compressor::new(48000.0, -20.0, 4.0, 1.0, 50.0, 0.0);
    let mut ducked = 0.0f32;
    for _ in 0..500 {
        let (out, gr) = comp.process_sample_sidechain(0.2, 0.9);
        ducked = out;
        assert!(gr >= 0.0);
    }
    assert!(ducked < 0.15, "Sidechain key should duck signal: {ducked}");

    // 2. StereoProcessor width=0.0 collapses stereo to mono
    let mono_proc = stem_dsp::StereoProcessor::new(0.0, 0.0, 0.0);
    let (ml, mr) = mono_proc.process_stereo(0.8, 0.2);
    assert!((ml - 0.5).abs() < 1e-5 && (mr - 0.5).abs() < 1e-5);

    // StereoProcessor width=2.0 widens stereo difference
    let wide_proc = stem_dsp::StereoProcessor::new(2.0, 0.0, 0.0);
    let (wl, wr) = wide_proc.process_stereo(0.8, 0.2);
    assert!((wl - 1.1).abs() < 1e-5 && (wr - (-0.1)).abs() < 1e-5);

    // 3. Surround 5.1 encode + ITU downmix
    let pan51 = stem_dsp::Surround51Pan {
        rear_mix: 0.5,
        center_mix: 0.5,
        lfe_mix: 0.25,
    };
    let ch = pan51.encode_stereo(0.6, 0.4);
    assert!(ch[2] > 0.0 && ch[3] > 0.0 && ch[4] > 0.0 && ch[5] > 0.0);
    let (dl, dr) = stem_dsp::Surround51Pan::downmix_to_stereo(ch);
    assert!(dl > 0.5 && dr > 0.3);
}

#[test]
fn test_reference_loudness_and_spectrum_comparison() {
    let sr = 48000u32;
    let mut mix = Vec::with_capacity(sr as usize * 2);
    let mut reference = Vec::with_capacity(sr as usize * 2);
    for i in 0..sr as usize {
        let t = i as f32 / sr as f32;
        // Mix: quiet 100 Hz bass
        let m = (2.0 * std::f32::consts::PI * 100.0 * t).sin() * 0.2;
        mix.push(m);
        mix.push(m);
        // Reference: louder 6000 Hz bright tone
        let r = (2.0 * std::f32::consts::PI * 6000.0 * t).sin() * 0.5;
        reference.push(r);
        reference.push(r);
    }

    let cmp = stem_dsp::MasteringAssistant::compare_with_reference(&mix, &reference, 2, sr, -14.0)
        .expect("Reference comparison should succeed");
    assert!(
        cmp.lufs_delta_db < 0.0,
        "Mix should be quieter than reference"
    );
    assert!(cmp.low_delta_pct > 50.0, "Mix should have more low energy");
    assert!(
        cmp.high_delta_pct < -50.0,
        "Reference should have more high energy"
    );
}

#[test]
fn test_dsp_regression_and_throughput_benchmark() {
    let sr = 48000.0f32;
    let mut chain = stem_dsp::EffectChain::default_chain(sr);
    let num_samples = 48000usize; // 1 second stereo
    let mut left = vec![0.0f32; num_samples];
    let mut right = vec![0.0f32; num_samples];
    for i in 0..num_samples {
        let t = i as f32 / sr;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        left[i] = s;
        right[i] = s;
    }

    let t0 = std::time::Instant::now();
    for i in 0..num_samples {
        let (ol, or) = chain.process_stereo(left[i], right[i]);
        assert!(ol.is_finite() && or.is_finite());
        left[i] = ol;
        right[i] = or;
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_secs_f32() < 1.0,
        "DSP regression: 1s stereo default_chain took {elapsed:?}"
    );

    // Multi-rate resample regression (44100 -> 48000 -> 96000)
    let in_ch = vec![left[..4410].to_vec()];
    let resampled_48 = stem_dsp::resample_audio(&in_ch, 44100, 48000).expect("44.1k->48k resample");
    let resampled_96 =
        stem_dsp::resample_audio(&resampled_48, 48000, 96000).expect("48k->96k resample");
    assert!(!resampled_96.is_empty() && !resampled_96[0].is_empty());
    assert!(resampled_96[0].iter().all(|s| s.is_finite()));
}
