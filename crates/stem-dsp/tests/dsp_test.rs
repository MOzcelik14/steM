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
