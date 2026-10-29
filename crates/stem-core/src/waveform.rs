//! Non-destructive waveform document, edit list, clipboard, and operations for steM.
//!
//! STRICT INVARIANTS:
//! - Source audio files on disk are NEVER modified.
//! - Edits are stored as an edit list of immutable segments referencing source buffers.
//! - Every operation (cut, copy, paste, delete, trim, silence, reverse, invert) is undoable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::project::AudioSource;

/// A non-destructive slice of audio in a waveform document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AudioSegment {
    /// Slices an immutable source buffer.
    SourceSlice {
        source_id: Uuid,
        start_frame: u64,
        length_frames: u64,
        reversed: bool,
        inverted: bool,
    },
    /// Digital silence.
    Silence { length_frames: u64 },
}

impl AudioSegment {
    #[inline]
    pub fn length_frames(&self) -> u64 {
        match self {
            AudioSegment::SourceSlice { length_frames, .. } => *length_frames,
            AudioSegment::Silence { length_frames } => *length_frames,
        }
    }
}

/// Global audio clipboard for copying/cutting/pasting across open files.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClipboard {
    pub channels: Vec<Vec<f32>>,
    pub sample_rate: u32,
    pub duration_frames: u64,
}

impl AudioClipboard {
    pub fn new(channels: Vec<Vec<f32>>, sample_rate: u32) -> Self {
        let duration_frames = channels.first().map(|ch| ch.len() as u64).unwrap_or(0);
        Self {
            channels,
            sample_rate,
            duration_frames,
        }
    }
}

pub use crate::project::FadeCurve;

/// A non-destructive single-file waveform document with an edit list of segments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaveformDocument {
    pub id: Uuid,
    pub name: String,
    pub original_source_id: Uuid,
    pub sample_rate: u32,
    pub channels: usize,
    pub segments: Vec<AudioSegment>,
}

impl WaveformDocument {
    /// Create a new document referencing an entire audio source.
    pub fn from_source(source: &AudioSource) -> Self {
        let segment = AudioSegment::SourceSlice {
            source_id: source.id,
            start_frame: 0,
            length_frames: source.duration_frames,
            reversed: false,
            inverted: false,
        };
        Self {
            id: Uuid::new_v4(),
            name: source.filename.clone(),
            original_source_id: source.id,
            sample_rate: source.sample_rate,
            channels: source.channels,
            segments: vec![segment],
        }
    }

    /// Total duration in frames across all segments.
    pub fn total_frames(&self) -> u64 {
        self.segments.iter().map(|s| s.length_frames()).sum()
    }

    /// Total duration in seconds.
    pub fn total_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.total_frames() as f64 / self.sample_rate as f64
        }
    }

    /// Renders the document into a planar stereo/multi-channel buffer.
    pub fn render_planar(&self, source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>) -> Vec<Vec<f32>> {
        let total = self.total_frames() as usize;
        let mut output = vec![vec![0.0f32; total]; self.channels];

        let mut offset = 0usize;
        for segment in &self.segments {
            let seg_len = segment.length_frames() as usize;
            match segment {
                AudioSegment::Silence { .. } => {
                    // Already zeroes in output
                    offset += seg_len;
                }
                AudioSegment::SourceSlice {
                    source_id,
                    start_frame,
                    length_frames,
                    reversed,
                    inverted,
                } => {
                    let len = *length_frames as usize;
                    let start = *start_frame as usize;
                    let sign = if *inverted { -1.0f32 } else { 1.0f32 };

                    if let Some(buf) = source_buffers.get(source_id) {
                        for (ch_idx, out_ch) in output.iter_mut().enumerate() {
                            let src_ch = if ch_idx < buf.len() {
                                &buf[ch_idx]
                            } else if !buf.is_empty() {
                                &buf[0]
                            } else {
                                continue;
                            };

                            for i in 0..len {
                                let src_idx = if *reversed {
                                    start + (len - 1 - i)
                                } else {
                                    start + i
                                };
                                let val = src_ch.get(src_idx).copied().unwrap_or(0.0) * sign;
                                if offset + i < out_ch.len() {
                                    out_ch[offset + i] = val;
                                }
                            }
                        }
                    }
                    offset += seg_len;
                }
            }
        }

        output
    }

    /// Splits segments at a target frame position so that a boundary exists at `target_frame`.
    fn split_at(&mut self, target_frame: u64) {
        let mut acc = 0u64;
        for i in 0..self.segments.len() {
            let seg_len = self.segments[i].length_frames();
            let seg_start = acc;
            let seg_end = acc + seg_len;

            if target_frame > seg_start && target_frame < seg_end {
                let split_offset = target_frame - seg_start;
                let seg = self.segments.remove(i);

                match seg {
                    AudioSegment::Silence { length_frames } => {
                        self.segments.insert(
                            i,
                            AudioSegment::Silence {
                                length_frames: length_frames - split_offset,
                            },
                        );
                        self.segments.insert(
                            i,
                            AudioSegment::Silence {
                                length_frames: split_offset,
                            },
                        );
                    }
                    AudioSegment::SourceSlice {
                        source_id,
                        start_frame,
                        length_frames,
                        reversed,
                        inverted,
                    } => {
                        if reversed {
                            let first_len = split_offset;
                            let second_len = length_frames - split_offset;
                            // First part comes from the tail of the source range
                            let part1 = AudioSegment::SourceSlice {
                                source_id,
                                start_frame: start_frame + second_len,
                                length_frames: first_len,
                                reversed: true,
                                inverted,
                            };
                            let part2 = AudioSegment::SourceSlice {
                                source_id,
                                start_frame,
                                length_frames: second_len,
                                reversed: true,
                                inverted,
                            };
                            self.segments.insert(i, part2);
                            self.segments.insert(i, part1);
                        } else {
                            let part1 = AudioSegment::SourceSlice {
                                source_id,
                                start_frame,
                                length_frames: split_offset,
                                reversed: false,
                                inverted,
                            };
                            let part2 = AudioSegment::SourceSlice {
                                source_id,
                                start_frame: start_frame + split_offset,
                                length_frames: length_frames - split_offset,
                                reversed: false,
                                inverted,
                            };
                            self.segments.insert(i, part2);
                            self.segments.insert(i, part1);
                        }
                    }
                }
                return;
            }
            acc += seg_len;
        }
    }

    /// Split document segments at boundaries `start` and `end`.
    fn split_range(&mut self, start: u64, end: u64) {
        let total = self.total_frames();
        let s = start.min(total);
        let e = end.min(total).max(s);
        self.split_at(s);
        self.split_at(e);
    }

    /// Copy audio in range [start_frame, end_frame) into a new AudioClipboard.
    pub fn copy_range(
        &self,
        start_frame: u64,
        end_frame: u64,
        source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    ) -> Option<AudioClipboard> {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            return None;
        }

        let full_render = self.render_planar(source_buffers);
        let len = (e - s) as usize;
        let mut clip_channels = Vec::with_capacity(full_render.len());

        for ch in &full_render {
            let slice = ch[s as usize..e as usize].to_vec();
            clip_channels.push(slice);
        }

        Some(AudioClipboard {
            channels: clip_channels,
            sample_rate: self.sample_rate,
            duration_frames: len as u64,
        })
    }

    /// Delete audio in range [start_frame, end_frame).
    pub fn delete_range(&mut self, start_frame: u64, end_frame: u64) {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            return;
        }

        self.split_range(s, e);

        let mut acc = 0u64;
        let mut new_segments = Vec::new();
        for seg in self.segments.drain(..) {
            let seg_len = seg.length_frames();
            let seg_start = acc;
            let seg_end = acc + seg_len;

            // Retain segments that are strictly before s or strictly after e
            if seg_end <= s || seg_start >= e {
                new_segments.push(seg);
            }
            acc += seg_len;
        }
        self.segments = new_segments;
    }

    /// Cut audio in range [start_frame, end_frame), copying to clipboard and deleting from document.
    pub fn cut_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    ) -> Option<AudioClipboard> {
        let clip = self.copy_range(start_frame, end_frame, source_buffers);
        self.delete_range(start_frame, end_frame);
        clip
    }

    /// Trim document to only retain range [start_frame, end_frame).
    pub fn trim_to_range(&mut self, start_frame: u64, end_frame: u64) {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            self.segments.clear();
            return;
        }

        self.split_range(s, e);

        let mut acc = 0u64;
        let mut new_segments = Vec::new();
        for seg in self.segments.drain(..) {
            let seg_len = seg.length_frames();
            let seg_start = acc;
            let seg_end = acc + seg_len;

            if seg_start >= s && seg_end <= e {
                new_segments.push(seg);
            }
            acc += seg_len;
        }
        self.segments = new_segments;
    }

    /// Insert digital silence of `length_frames` at `at_frame`.
    pub fn insert_silence(&mut self, at_frame: u64, length_frames: u64) {
        if length_frames == 0 {
            return;
        }
        let total = self.total_frames();
        let at = at_frame.min(total);

        self.split_at(at);

        let mut acc = 0u64;
        let mut insert_idx = self.segments.len();
        for (i, seg) in self.segments.iter().enumerate() {
            if acc == at {
                insert_idx = i;
                break;
            }
            acc += seg.length_frames();
        }

        self.segments
            .insert(insert_idx, AudioSegment::Silence { length_frames });
    }

    /// Paste audio clipboard at `at_frame`. Registers a new virtual AudioSource buffer.
    pub fn paste_clipboard(
        &mut self,
        at_frame: u64,
        clipboard: &AudioClipboard,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
    ) {
        if clipboard.duration_frames == 0 || clipboard.channels.is_empty() {
            return;
        }

        let new_src_id = Uuid::new_v4();
        source_buffers.insert(new_src_id, clipboard.channels.clone());

        let total = self.total_frames();
        let at = at_frame.min(total);

        self.split_at(at);

        let mut acc = 0u64;
        let mut insert_idx = self.segments.len();
        for (i, seg) in self.segments.iter().enumerate() {
            if acc == at {
                insert_idx = i;
                break;
            }
            acc += seg.length_frames();
        }

        self.segments.insert(
            insert_idx,
            AudioSegment::SourceSlice {
                source_id: new_src_id,
                start_frame: 0,
                length_frames: clipboard.duration_frames,
                reversed: false,
                inverted: false,
            },
        );
    }

    /// Reverse audio in range [start_frame, end_frame).
    pub fn reverse_range(&mut self, start_frame: u64, end_frame: u64) {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            return;
        }

        self.split_range(s, e);

        // Find segments strictly inside [s, e]
        let mut acc = 0u64;
        let mut sub_start = 0;
        let mut sub_end = 0;

        for (i, seg) in self.segments.iter().enumerate() {
            let seg_start = acc;
            let seg_end = acc + seg.length_frames();
            if seg_start == s {
                sub_start = i;
            }
            if seg_end == e {
                sub_end = i + 1;
                break;
            }
            acc += seg.length_frames();
        }

        if sub_start < sub_end {
            // Reverse the order of segments
            self.segments[sub_start..sub_end].reverse();

            // Toggle reversed flag on each segment
            for seg in &mut self.segments[sub_start..sub_end] {
                if let AudioSegment::SourceSlice { reversed, .. } = seg {
                    *reversed = !*reversed;
                }
            }
        }
    }

    /// Invert phase in range [start_frame, end_frame) (* -1.0).
    pub fn invert_phase_range(&mut self, start_frame: u64, end_frame: u64) {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            return;
        }

        self.split_range(s, e);

        let mut acc = 0u64;
        for seg in &mut self.segments {
            let seg_len = seg.length_frames();
            let seg_start = acc;
            let seg_end = acc + seg_len;

            if seg_start >= s && seg_end <= e {
                if let AudioSegment::SourceSlice { inverted, .. } = seg {
                    *inverted = !*inverted;
                }
            }
            acc += seg_len;
        }
    }

    /// Render audio in range [start_frame, end_frame) into planar channel buffers.
    pub fn render_range(
        &self,
        start_frame: u64,
        end_frame: u64,
        source_buffers: &HashMap<Uuid, Vec<Vec<f32>>>,
    ) -> Option<Vec<Vec<f32>>> {
        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);
        if s >= e {
            return None;
        }

        let full_render = self.render_planar(source_buffers);
        let mut clip_channels = Vec::with_capacity(full_render.len());
        for ch in &full_render {
            let slice = ch[s as usize..e as usize].to_vec();
            clip_channels.push(slice);
        }
        Some(clip_channels)
    }

    /// Replace range [start_frame, end_frame) with pre-rendered planar audio channels.
    pub fn replace_range_with_rendered(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        rendered_channels: Vec<Vec<f32>>,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
    ) {
        let dur = rendered_channels
            .first()
            .map(|ch| ch.len() as u64)
            .unwrap_or(0);
        if dur == 0 {
            return;
        }

        let total = self.total_frames();
        let s = start_frame.min(total);
        let e = end_frame.min(total).max(s);

        self.delete_range(s, e);

        let new_src_id = Uuid::new_v4();
        source_buffers.insert(new_src_id, rendered_channels);

        self.split_at(s);

        let mut acc = 0u64;
        let mut insert_idx = self.segments.len();
        for (i, seg) in self.segments.iter().enumerate() {
            if acc == s {
                insert_idx = i;
                break;
            }
            acc += seg.length_frames();
        }

        self.segments.insert(
            insert_idx,
            AudioSegment::SourceSlice {
                source_id: new_src_id,
                start_frame: 0,
                length_frames: dur,
                reversed: false,
                inverted: false,
            },
        );
    }

    /// Apply a fade-in curve to range [start_frame, end_frame).
    pub fn apply_fade_in_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        curve: FadeCurve,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
    ) {
        if let Some(mut channels) = self.render_range(start_frame, end_frame, source_buffers) {
            let dur = channels[0].len();
            if dur > 0 {
                let divisor = if dur > 1 { (dur - 1) as f32 } else { 1.0 };
                for i in 0..dur {
                    let t = i as f32 / divisor;
                    let g = curve.evaluate_fade_in(t);
                    for ch in &mut channels {
                        ch[i] *= g;
                    }
                }
                self.replace_range_with_rendered(start_frame, end_frame, channels, source_buffers);
            }
        }
    }

    /// Apply a fade-out curve to range [start_frame, end_frame).
    pub fn apply_fade_out_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        curve: FadeCurve,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
    ) {
        if let Some(mut channels) = self.render_range(start_frame, end_frame, source_buffers) {
            let dur = channels[0].len();
            if dur > 0 {
                let divisor = if dur > 1 { (dur - 1) as f32 } else { 1.0 };
                for i in 0..dur {
                    let t = i as f32 / divisor;
                    let g = curve.evaluate_fade_out(t);
                    for ch in &mut channels {
                        ch[i] *= g;
                    }
                }
                self.replace_range_with_rendered(start_frame, end_frame, channels, source_buffers);
            }
        }
    }

    /// Peak-normalize audio in range [start_frame, end_frame) to `target_db`.
    pub fn normalize_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        target_db: f32,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
    ) {
        if let Some(mut channels) = self.render_range(start_frame, end_frame, source_buffers) {
            let mut peak = 0.0f32;
            for ch in &channels {
                for &s in ch {
                    let abs_s = s.abs();
                    if abs_s > peak {
                        peak = abs_s;
                    }
                }
            }

            if peak > 1e-7 {
                let target_linear = 10.0f32.powf(target_db / 20.0);
                let gain = target_linear / peak;
                for ch in &mut channels {
                    for s in ch.iter_mut() {
                        *s *= gain;
                    }
                }
                self.replace_range_with_rendered(start_frame, end_frame, channels, source_buffers);
            }
        }
    }

    /// Process range [start_frame, end_frame) in-place with an arbitrary audio processor callback.
    pub fn apply_processor_to_range<F>(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        source_buffers: &mut HashMap<Uuid, Vec<Vec<f32>>>,
        processor: F,
    ) where
        F: FnOnce(&mut [Vec<f32>]),
    {
        if let Some(mut channels) = self.render_range(start_frame, end_frame, source_buffers) {
            processor(&mut channels);
            self.replace_range_with_rendered(start_frame, end_frame, channels, source_buffers);
        }
    }
}

/// Finds the nearest zero-crossing index around `target_frame` within `max_search_radius`.
/// A zero-crossing occurs where `s[i] * s[i+1] <= 0`.
pub fn find_nearest_zero_crossing(
    channel: &[f32],
    target_frame: u64,
    max_search_radius: usize,
) -> u64 {
    let len = channel.len();
    if len < 2 {
        return target_frame;
    }

    let target = (target_frame as usize).min(len - 1);
    let start = target.saturating_sub(max_search_radius);
    let end = (target + max_search_radius).min(len - 1);

    let mut best_crossing = None;

    for i in start..end {
        let a = channel[i];
        let b = channel[i + 1];
        if (a * b) <= 0.0 {
            let cand = if a.abs() <= b.abs() { i } else { i + 1 };
            let dist = (cand as isize - target as isize).unsigned_abs();
            let val = channel[cand].abs();

            match best_crossing {
                None => best_crossing = Some((cand, dist, val)),
                Some((_, best_dist, best_val)) => {
                    if dist < best_dist || (dist == best_dist && val < best_val) {
                        best_crossing = Some((cand, dist, val));
                    }
                }
            }
        }
    }

    best_crossing
        .map(|(idx, _, _)| idx as u64)
        .unwrap_or(target_frame)
}

/// Transactional undo/redo stack for waveform document edits.
#[derive(Debug, Clone, Default)]
pub struct WaveformHistory {
    undo_stack: Vec<WaveformDocument>,
    redo_stack: Vec<WaveformDocument>,
    max_entries: usize,
}

impl WaveformHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: 50,
        }
    }

    /// Record a state before mutating. Clears redo stack.
    pub fn push_state(&mut self, state_before: WaveformDocument) {
        if self.undo_stack.len() >= self.max_entries {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(state_before);
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self, current: &mut WaveformDocument) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(current.clone());
            *current = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, current: &mut WaveformDocument) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(current.clone());
            *current = next;
            true
        } else {
            false
        }
    }
}
