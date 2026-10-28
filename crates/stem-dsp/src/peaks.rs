//! Peak cache generation for instantaneous multi-resolution waveform visualization

use stem_core::peaks::{PeakBucket, PeakPyramid};

/// Build a complete multi-resolution peak pyramid from multi-channel audio data.
/// Hierarchically aggregates min/max values across pyramid levels for maximum speed.
pub fn build_peak_pyramid(channels: &[&[f32]]) -> PeakPyramid {
    let num_channels = channels.len();
    if num_channels == 0 {
        return PeakPyramid::new(0, 0);
    }
    let total_frames = channels[0].len() as u64;

    let mut pyramid = PeakPyramid::new(num_channels, total_frames);
    if total_frames == 0 {
        return pyramid;
    }

    // Level 0: 64 samples per bucket, computed directly from raw PCM
    let level_0_block_size = PeakPyramid::BLOCK_SIZES[0];
    for (ch_idx, channel_data) in channels.iter().enumerate() {
        let buckets = &mut pyramid.levels[0].channels[ch_idx];
        for (b_idx, chunk) in channel_data.chunks(level_0_block_size).enumerate() {
            let mut min_val = chunk[0];
            let mut max_val = chunk[0];
            for &s in chunk.iter().skip(1) {
                if s < min_val {
                    min_val = s;
                }
                if s > max_val {
                    max_val = s;
                }
            }
            if b_idx < buckets.len() {
                buckets[b_idx] = PeakBucket::new(min_val, max_val);
            }
        }
    }

    // Higher levels: 256, 1024, 4096, 16384
    // Each level's block size is 4x the previous level's, so 4 buckets from level L-1 merge into 1 bucket of level L!
    for lvl in 1..PeakPyramid::BLOCK_SIZES.len() {
        let (prev_level, curr_level) = {
            let (left, right) = pyramid.levels.split_at_mut(lvl);
            (&left[lvl - 1], &mut right[0])
        };

        let ratio = curr_level.block_size / prev_level.block_size;

        for ch_idx in 0..num_channels {
            let src_buckets = &prev_level.channels[ch_idx];
            let dst_buckets = &mut curr_level.channels[ch_idx];

            for (dst_idx, chunk) in src_buckets.chunks(ratio).enumerate() {
                if chunk.is_empty() {
                    continue;
                }
                let mut merged = chunk[0];
                for b in chunk.iter().skip(1) {
                    merged.merge(b);
                }
                if dst_idx < dst_buckets.len() {
                    dst_buckets[dst_idx] = merged;
                }
            }
        }
    }

    pyramid
}
