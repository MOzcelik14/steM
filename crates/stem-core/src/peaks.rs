use serde::{Deserialize, Serialize};

/// A single peak bucket representing the minimum and maximum sample amplitude.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PeakBucket {
    pub min: f32,
    pub max: f32,
}

impl PeakBucket {
    #[inline]
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn merge(&mut self, other: &PeakBucket) {
        if other.min < self.min {
            self.min = other.min;
        }
        if other.max > self.max {
            self.max = other.max;
        }
    }
}

/// Multi-resolution peak cache level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakLevel {
    /// Number of audio frames grouped into each bucket (e.g. 64, 256, 1024, 4096, 16384).
    pub block_size: usize,
    /// Buckets per audio channel: channels[ch][bucket_idx].
    pub channels: Vec<Vec<PeakBucket>>,
}

/// Complete multi-resolution peak pyramid for fast O(1) waveform visualization at any zoom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakPyramid {
    pub levels: Vec<PeakLevel>,
    pub total_frames: u64,
    pub num_channels: usize,
}

impl PeakPyramid {
    pub const BLOCK_SIZES: &'static [usize] = &[64, 256, 1024, 4096, 16384];

    pub fn new(num_channels: usize, total_frames: u64) -> Self {
        let levels = Self::BLOCK_SIZES
            .iter()
            .map(|&block_size| {
                let num_buckets = (total_frames as usize).div_ceil(block_size);
                let channels = vec![vec![PeakBucket::default(); num_buckets]; num_channels];
                PeakLevel {
                    block_size,
                    channels,
                }
            })
            .collect();

        Self {
            levels,
            total_frames,
            num_channels,
        }
    }

    /// Query peak data for a given channel and time range, optimized for the display pixel width.
    /// Returns a slice of min/max pairs scaled to the target pixel resolution.
    pub fn query_channel_peaks(
        &self,
        channel: usize,
        start_frame: u64,
        end_frame: u64,
        target_points: usize,
    ) -> Vec<PeakBucket> {
        if self.levels.is_empty() || channel >= self.num_channels || target_points == 0 {
            return Vec::new();
        }

        let clamped_start = start_frame.min(self.total_frames);
        let clamped_end = end_frame.min(self.total_frames).max(clamped_start);
        let range_frames = (clamped_end - clamped_start) as usize;
        if range_frames == 0 {
            return Vec::new();
        }

        let frames_per_point = range_frames.div_ceil(target_points).max(1);

        // Find the most detailed pyramid level whose block_size is <= frames_per_point
        let chosen_level = self
            .levels
            .iter()
            .rev()
            .find(|l| l.block_size <= frames_per_point)
            .unwrap_or(&self.levels[0]);

        let block_size = chosen_level.block_size;
        let buckets = &chosen_level.channels[channel];

        let mut result = Vec::with_capacity(target_points);

        for pt in 0..target_points {
            let pt_frame_start = clamped_start as usize + (pt * range_frames) / target_points;
            let pt_frame_end = (clamped_start as usize + ((pt + 1) * range_frames) / target_points)
                .min(clamped_end as usize);

            let bucket_start = pt_frame_start / block_size;
            let bucket_end = pt_frame_end.div_ceil(block_size).min(buckets.len());

            if bucket_start < buckets.len() && bucket_start < bucket_end {
                let mut merged = buckets[bucket_start];
                for b in &buckets[bucket_start + 1..bucket_end] {
                    merged.merge(b);
                }
                result.push(merged);
            } else if bucket_start < buckets.len() {
                result.push(buckets[bucket_start]);
            } else {
                result.push(PeakBucket::default());
            }
        }

        result
    }
}
