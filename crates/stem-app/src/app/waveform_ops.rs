use super::StemApp;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use stem_audio::SharedAudioTimeline;
use stem_core::waveform::WaveformDocument;

use crate::state::AppViewMode;

impl StemApp {
    /// Re-renders active waveform document into a SharedAudioTimeline buffer and sends it to the real-time engine.
    pub fn sync_waveform_to_audio_engine(&mut self) {
        if let Some(ref planar) = self.waveform_state.cached_planar {
            let total_frames = self
                .waveform_state
                .document
                .as_ref()
                .map(|d| d.total_frames())
                .unwrap_or(0);
            let timeline = Arc::new(SharedAudioTimeline {
                channels: planar.clone(),
                track_channels: Vec::new(),
                total_frames,
            });
            self.active_timeline = Some(timeline.clone());
            if let Some(ref mut engine) = self.audio_engine {
                engine.update_timeline(timeline);
                self.sync_waveform_playback_loop();
            }
        }
    }

    pub fn sync_waveform_playback_loop(&mut self) {
        if let Some(ref mut engine) = self.audio_engine {
            if self.waveform_state.loop_selection {
                if let Some((start, end)) = self.waveform_state.selection {
                    if end > start {
                        engine.set_loop(true, start, end);
                        return;
                    }
                }
            }
            let total = self
                .waveform_state
                .document
                .as_ref()
                .map(|d| d.total_frames())
                .unwrap_or(0);
            engine.set_loop(false, 0, total);
        }
    }

    pub fn recompute_waveform_cache(&mut self) {
        if let Some(ref doc) = self.waveform_state.document {
            let planar = doc.render_planar(&self.source_buffers);
            let channel_refs: Vec<&[f32]> = planar.iter().map(|ch| ch.as_slice()).collect();
            let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
            self.waveform_state.cached_planar = Some(planar);
            self.waveform_state.pyramid = Some(pyramid);
            if self.view_mode == AppViewMode::Waveform {
                self.sync_waveform_to_audio_engine();
            }
        } else {
            self.waveform_state.cached_planar = None;
            self.waveform_state.pyramid = None;
        }
    }

    pub fn load_source_into_waveform_editor(
        &mut self,
        source_id: Uuid,
        file_name: &str,
        clip_info: Option<(Uuid, Uuid)>,
    ) {
        let source = self.project.sources.get(&source_id).cloned();
        if let Some(source) = source {
            let doc = WaveformDocument::from_source(&source);
            self.waveform_state.document = Some(doc);
            self.waveform_state.active_source_id = Some(source_id);
            self.waveform_state.file_name = file_name.to_string();
            self.waveform_state.active_clip_info = clip_info;
            self.waveform_state.selection = None;
            self.waveform_state.history = stem_core::waveform::WaveformHistory::new();
            self.waveform_state.playhead_frame = 0;
            self.recompute_waveform_cache();
            self.view_mode = AppViewMode::Waveform;
            self.action_seek_waveform(0);
        }
    }

    pub fn action_open_clip_in_waveform_editor(&mut self, clip_id: Uuid) {
        for track in &self.project.tracks {
            if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
                let track_id = track.id;
                let clip_name = clip.name.clone();
                let source_id = clip.source_id;
                self.load_source_into_waveform_editor(
                    source_id,
                    &clip_name,
                    Some((clip_id, track_id)),
                );
                break;
            }
        }
    }

    /// Ensures the currently selected Multitrack clip (or clip under `time_selection`)
    /// is loaded into `waveform_state.document` without switching away from Multitrack view,
    /// mapping any `time_selection` into `waveform_state.selection`.
    pub fn ensure_selected_clip_loaded_in_place(&mut self) -> bool {
        if self.view_mode == AppViewMode::Waveform {
            return self.waveform_state.document.is_some();
        }
        let sel = self.timeline_state.time_selection;
        let target_clip_id = self.timeline_state.selected_clip_id;
        let mut found: Option<(Uuid, Uuid, Uuid, String, u64, u64)> = None;
        for track in &self.project.tracks {
            for clip in &track.clips {
                let matches_id = target_clip_id == Some(clip.id);
                let matches_sel = target_clip_id.is_none()
                    && sel.is_some_and(|(s, e)| {
                        e > clip.track_offset_frames && s < clip.timeline_end_frame()
                    });
                if matches_id || matches_sel {
                    found = Some((
                        clip.id,
                        track.id,
                        clip.source_id,
                        clip.name.clone(),
                        clip.track_offset_frames,
                        clip.source_start_frame,
                    ));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let Some((clip_id, track_id, source_id, clip_name, track_off, src_start)) = found else {
            return self.waveform_state.document.is_some();
        };
        let needs_load = self.waveform_state.active_clip_info != Some((clip_id, track_id))
            || self.waveform_state.document.is_none();
        if needs_load {
            if let Some(source) = self.project.sources.get(&source_id).cloned() {
                let doc = WaveformDocument::from_source(&source);
                self.waveform_state.document = Some(doc);
                self.waveform_state.active_source_id = Some(source_id);
                self.waveform_state.file_name = clip_name;
                self.waveform_state.active_clip_info = Some((clip_id, track_id));
                self.waveform_state.history = stem_core::waveform::WaveformHistory::new();
                self.recompute_waveform_cache();
            }
        }
        if let (Some((s, e)), Some(ref doc)) = (sel, &self.waveform_state.document) {
            let total = doc.total_frames();
            let rel_s = s
                .saturating_sub(track_off)
                .saturating_add(src_start)
                .min(total);
            let rel_e = e
                .saturating_sub(track_off)
                .saturating_add(src_start)
                .min(total);
            if rel_s < rel_e {
                self.waveform_state.selection = Some((rel_s, rel_e));
            }
        }
        self.waveform_state.document.is_some()
    }

    fn finish_waveform_or_timeline_edit(&mut self) {
        self.recompute_waveform_cache();
        if self.view_mode == AppViewMode::Multitrack
            && self.waveform_state.active_clip_info.is_some()
        {
            self.action_waveform_apply_to_project();
        }
    }

    pub fn action_open_file_in_waveform_editor(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Audio Files",
                &[
                    "wav", "WAV", "flac", "FLAC", "mp3", "MP3", "ogg", "OGG", "aac", "AAC", "m4a",
                    "M4A", "aiff", "AIFF",
                ],
            )
            .pick_file()
        {
            self.status_text = self.i18n.t("status-loading");
            match stem_audio::decode_file(&path) {
                Ok(decoded) => {
                    let source = decoded.source.clone();
                    let source_id = source.id;
                    let filename = source.filename.clone();
                    self.project.add_source(source);
                    self.source_buffers.insert(source_id, decoded.channels);

                    self.load_source_into_waveform_editor(source_id, &filename, None);
                    self.status_text =
                        self.i18n
                            .t_arg("status-imported-file", "filename", filename);
                }
                Err(e) => {
                    self.status_text = format!("Failed to open file: {e}");
                }
            }
        }
    }

    pub fn action_switch_to_multitrack(&mut self) {
        if self.is_playing {
            self.action_stop();
        }
        self.view_mode = AppViewMode::Multitrack;
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_switch_to_waveform(&mut self) {
        if self.is_playing {
            self.action_stop();
        }
        self.view_mode = AppViewMode::Waveform;
        if self.waveform_state.document.is_some() {
            self.sync_waveform_to_audio_engine();
        }
    }

    pub fn action_seek_waveform(&mut self, frame: u64) {
        self.waveform_state.playhead_frame = frame;
        if let Some(ref mut engine) = self.audio_engine {
            engine.seek(frame);
        }
        self.telemetry.playhead_frame = frame;
    }

    pub fn action_waveform_cut(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            if let Some((s, e)) = self.waveform_state.selection {
                if e > s {
                    self.waveform_state.history.push_state(doc.clone());
                    self.clipboard = doc.cut_range(s, e, &self.source_buffers);
                    self.waveform_state.selection = None;
                    self.status_text = self.i18n.t("waveform-status-cut");
                    self.recompute_waveform_cache();
                }
            }
        }
    }

    pub fn action_waveform_copy(&mut self) {
        if let Some(ref doc) = self.waveform_state.document {
            if let Some((s, e)) = self.waveform_state.selection {
                if e > s {
                    self.clipboard = doc.copy_range(s, e, &self.source_buffers);
                    self.status_text = self.i18n.t("waveform-status-copied");
                }
            }
        }
    }

    pub fn action_waveform_paste(&mut self) {
        if let Some(ref clip) = self.clipboard.clone() {
            if let Some(ref mut doc) = self.waveform_state.document {
                self.waveform_state.history.push_state(doc.clone());
                let paste_at = if let Some((s, e)) = self.waveform_state.selection {
                    if e > s {
                        doc.delete_range(s, e);
                    }
                    s
                } else {
                    self.waveform_state.playhead_frame
                };
                doc.paste_clipboard(paste_at, clip, &mut self.source_buffers);
                let dur = clip.duration_frames;
                self.waveform_state.selection = Some((paste_at, paste_at + dur));
                self.status_text = self.i18n.t("waveform-status-pasted");
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_delete(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            if let Some((s, e)) = self.waveform_state.selection {
                if e > s {
                    self.waveform_state.history.push_state(doc.clone());
                    doc.delete_range(s, e);
                    self.waveform_state.selection = None;
                    self.status_text = self.i18n.t("waveform-status-deleted");
                    self.recompute_waveform_cache();
                }
            }
        }
    }

    pub fn action_waveform_trim(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            if let Some((s, e)) = self.waveform_state.selection {
                if e > s {
                    self.waveform_state.history.push_state(doc.clone());
                    doc.trim_to_range(s, e);
                    self.waveform_state.selection = None;
                    self.status_text = self.i18n.t("waveform-status-trimmed");
                    self.recompute_waveform_cache();
                }
            }
        }
    }

    pub fn action_waveform_insert_silence(&mut self) {
        let sample_rate = self.project.sample_rate;
        if let Some(ref mut doc) = self.waveform_state.document {
            self.waveform_state.history.push_state(doc.clone());
            let at = if let Some((s, _)) = self.waveform_state.selection {
                s
            } else {
                self.waveform_state.playhead_frame
            };
            doc.insert_silence(at, sample_rate as u64);
            self.status_text = self.i18n.t("waveform-status-silence-inserted");
            self.recompute_waveform_cache();
        }
    }

    pub fn action_waveform_reverse(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            self.waveform_state.history.push_state(doc.clone());
            let (s, e) = self
                .waveform_state
                .selection
                .unwrap_or((0, doc.total_frames()));
            doc.reverse_range(s, e);
            self.status_text = self.i18n.t("waveform-status-reversed");
            self.recompute_waveform_cache();
        }
    }

    pub fn action_waveform_invert_phase(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            self.waveform_state.history.push_state(doc.clone());
            let (s, e) = self
                .waveform_state
                .selection
                .unwrap_or((0, doc.total_frames()));
            doc.invert_phase_range(s, e);
            self.status_text = self.i18n.t("waveform-status-phase-inverted");
            self.recompute_waveform_cache();
        }
    }

    pub fn action_waveform_fade_in(&mut self, curve: stem_core::waveform::FadeCurve) {
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
            if s < e {
                self.waveform_state.history.push_state(doc.clone());
                doc.apply_fade_in_range(s, e, curve, &mut self.source_buffers);
                self.status_text = self.i18n.t("status-fade-applied");
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_fade_out(&mut self, curve: stem_core::waveform::FadeCurve) {
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
            if s < e {
                self.waveform_state.history.push_state(doc.clone());
                doc.apply_fade_out_range(s, e, curve, &mut self.source_buffers);
                self.status_text = self.i18n.t("status-fade-applied");
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_normalize(&mut self, target_db: f32) {
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
            if s < e {
                self.waveform_state.history.push_state(doc.clone());
                doc.normalize_range(s, e, target_db, &mut self.source_buffers);
                self.status_text = self.i18n.t("status-normalized");
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_apply_active_effects(&mut self) {
        let sample_rate = self.project.sample_rate as f32;
        let mut dsp_chain = self.effects_state.build_dsp_chain(sample_rate);
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
            if s < e {
                self.waveform_state.history.push_state(doc.clone());
                doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                    if channels.len() >= 2 {
                        let (first, rest) = channels.split_at_mut(1);
                        dsp_chain.process_buffer(&mut first[0], &mut rest[0]);
                    } else if !channels.is_empty() {
                        for s in &mut channels[0] {
                            let (l, _) = dsp_chain.process_stereo(*s, *s);
                            *s = l;
                        }
                    }
                });
                self.status_text = self.i18n.t("status-effect-applied");
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_capture_noise_profile(&mut self) {
        self.ensure_selected_clip_loaded_in_place();
        if let Some(ref doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
            if s < e {
                let rendered = doc.render_range(s, e, &self.source_buffers);
                if let Some(channels) = rendered {
                    if let Some(mono) = channels.first() {
                        let fft_size = 1024;
                        let profile = stem_dsp::NoiseProfile::capture(
                            mono,
                            self.project.sample_rate,
                            fft_size,
                        );
                        self.noise_reduction_state.noise_profile = Some(profile);
                        let dur_sec = (e - s) as f32 / self.project.sample_rate as f32;
                        let msg =
                            format!("Captured noise profile ({dur_sec:.2}s, {fft_size} bins)");
                        self.noise_reduction_state.status_message = Some(msg);
                        self.status_text = self.i18n.t_arg(
                            "cleanup-profile-captured",
                            "bins",
                            (fft_size / 2 + 1).to_string(),
                        );
                    }
                }
            }
        }
    }

    pub fn action_apply_noise_reduction(&mut self, whole_file: bool) {
        self.ensure_selected_clip_loaded_in_place();
        if self.noise_reduction_state.noise_profile.is_none() {
            self.action_capture_noise_profile();
        }
        if let Some(profile) = self.noise_reduction_state.noise_profile.clone() {
            let red_db = self.noise_reduction_state.reduction_db;
            let sens = self.noise_reduction_state.sensitivity;
            let gate = stem_dsp::SpectralGate::new(profile, red_db, sens);

            let mut applied = false;
            if let Some(ref mut doc) = self.waveform_state.document {
                let total = doc.total_frames();
                let (s, e) = if whole_file {
                    (0, total)
                } else {
                    self.waveform_state.selection.unwrap_or((0, total))
                };

                if s < e {
                    self.waveform_state.history.push_state(doc.clone());
                    doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                        gate.process_planar(channels);
                    });
                    self.status_text = self.i18n.t("cleanup-status-applied");
                    applied = true;
                }
            }
            if applied {
                self.finish_waveform_or_timeline_edit();
            }
        }
    }

    pub fn action_apply_click_remover(&mut self) {
        self.ensure_selected_clip_loaded_in_place();
        let sens = self.click_remover_state.sensitivity;
        let max_w = self.click_remover_state.max_width;
        let declicker = stem_dsp::ClickRemover::new(sens, max_w);

        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            let (s, e) = self.waveform_state.selection.unwrap_or((0, total));

            if s < e {
                self.waveform_state.history.push_state(doc.clone());
                let mut repaired_count = 0usize;
                doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                    repaired_count = declicker.process_planar(channels);
                });

                let msg = if repaired_count > 0 {
                    self.i18n
                        .t_arg("cleanup-click-result", "count", repaired_count.to_string())
                } else {
                    self.i18n.t("cleanup-click-none")
                };
                self.click_remover_state.status_message = Some(msg.clone());
                self.status_text = msg;
                applied = true;
            }
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    fn collect_active_interleaved(&self) -> (Vec<f32>, u32, u32) {
        let mut interleaved = Vec::new();
        let sample_rate = self.project.sample_rate;
        let mut channels = 2u32;

        if self.view_mode == AppViewMode::Waveform {
            if let Some(ref doc) = self.waveform_state.document {
                channels = (doc.channels.max(1)) as u32;
                let total = doc.total_frames();
                let (s, e) = self.waveform_state.selection.unwrap_or((0, total));
                if let Some(rendered) = doc.render_range(s, e, &self.source_buffers) {
                    let num_frames = rendered[0].len();
                    interleaved.reserve(num_frames * channels as usize);
                    for f in 0..num_frames {
                        for ch in 0..channels as usize {
                            let val = rendered
                                .get(ch)
                                .and_then(|c| c.get(f))
                                .copied()
                                .unwrap_or(0.0);
                            interleaved.push(val);
                        }
                    }
                }
            }
        } else if let Some(ref timeline) = self.active_timeline {
            let num_frames = timeline.channels[0].len();
            channels = 2;
            interleaved.reserve(num_frames * 2);
            for f in 0..num_frames {
                let l = timeline.channels[0].get(f).copied().unwrap_or(0.0);
                let r = timeline
                    .channels
                    .get(1)
                    .and_then(|c| c.get(f))
                    .copied()
                    .unwrap_or(l);
                interleaved.push(l);
                interleaved.push(r);
            }
        }
        (interleaved, channels, sample_rate)
    }

    pub fn action_open_loudness_report(&mut self) {
        let (interleaved, channels, sample_rate) = self.collect_active_interleaved();

        if !interleaved.is_empty() {
            let target_lufs = self.loudness_report_state.standard.target_lufs();
            match stem_dsp::MasteringAssistant::analyze_mix(
                &interleaved,
                channels,
                sample_rate,
                target_lufs,
            ) {
                Ok(report) => {
                    self.loudness_report_state.result = Some(stem_dsp::LoudnessResult {
                        integrated_lufs: report.dynamics.integrated_lufs,
                        momentary_max_lufs: report.dynamics.momentary_max_lufs,
                        short_term_max_lufs: report.dynamics.short_term_max_lufs,
                        loudness_range_lu: report.dynamics.loudness_range_lu,
                    });
                    self.loudness_report_state.true_peak_dbfs = report.dynamics.true_peak_dbfs;
                    self.loudness_report_state.mastering_report = Some(report);
                    self.loudness_report_state.show = true;
                }
                Err(e) => {
                    self.status_text = format!("Loudness analysis error: {e:?}");
                }
            }
        } else {
            self.status_text = "No audio data to analyze".to_string();
        }
    }

    /// Imports a reference audio file and compares its loudness and frequency spectrum
    /// against the active mix/waveform.
    pub fn action_compare_reference_audio(&mut self, path: &std::path::Path) {
        let (mix_interleaved, channels, sample_rate) = self.collect_active_interleaved();
        if mix_interleaved.is_empty() {
            self.status_text = "No active mix to compare against reference".to_string();
            return;
        }
        match stem_audio::decode_file(path) {
            Ok(decoded) => {
                let ref_frames = decoded.channels.first().map(|c| c.len()).unwrap_or(0);
                let mut ref_interleaved = Vec::with_capacity(ref_frames * channels as usize);
                for f in 0..ref_frames {
                    for ch in 0..channels as usize {
                        let ch_idx = ch.min(decoded.channels.len().saturating_sub(1));
                        let val = decoded
                            .channels
                            .get(ch_idx)
                            .and_then(|c| c.get(f))
                            .copied()
                            .unwrap_or(0.0);
                        ref_interleaved.push(val);
                    }
                }
                let target_lufs = self.loudness_report_state.standard.target_lufs();
                match stem_dsp::MasteringAssistant::compare_with_reference(
                    &mix_interleaved,
                    &ref_interleaved,
                    channels,
                    sample_rate,
                    target_lufs,
                ) {
                    Ok(cmp) => {
                        self.loudness_report_state.reference_audio_path = Some(path.to_path_buf());
                        self.loudness_report_state.reference_comparison = Some(cmp);
                    }
                    Err(e) => {
                        self.status_text = format!("Reference comparison error: {e:?}");
                    }
                }
            }
            Err(e) => {
                self.status_text = format!("Failed to decode reference audio: {e}");
            }
        }
    }

    pub fn action_waveform_undo(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            if self.waveform_state.history.undo(doc) {
                self.status_text = "Waveform edit undone".to_string();
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_redo(&mut self) {
        if let Some(ref mut doc) = self.waveform_state.document {
            if self.waveform_state.history.redo(doc) {
                self.status_text = "Waveform edit redone".to_string();
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_waveform_apply_to_project(&mut self) {
        if let Some((clip_id, track_id)) = self.waveform_state.active_clip_info {
            if let Some(ref doc) = self.waveform_state.document {
                let planar = doc.render_planar(&self.source_buffers);
                let new_source_id = Uuid::new_v4();
                let dur_frames = doc.total_frames();
                let source_name = format!("{}_edited", self.waveform_state.file_name);

                let source = stem_core::project::AudioSource {
                    id: new_source_id,
                    path: PathBuf::from(&source_name),
                    filename: source_name.clone(),
                    sample_rate: self.project.sample_rate,
                    channels: doc.channels,
                    duration_frames: dur_frames,
                    file_hash: None,
                };
                self.project.add_source(source);

                let channel_refs: Vec<&[f32]> = planar.iter().map(|ch| ch.as_slice()).collect();
                let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
                self.peak_pyramids.insert(new_source_id, pyramid);
                self.source_buffers.insert(new_source_id, planar);

                if let Some(track) = self.project.get_track_mut(track_id) {
                    if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                        clip.source_id = new_source_id;
                        clip.source_start_frame = 0;
                        clip.length_frames = dur_frames;
                    }
                }

                self.status_text = self.i18n.t("waveform-status-applied");
                self.sync_timeline_to_audio_engine();
            }
        }
    }

    /// Non-destructive STFT attenuation of the selected time-frequency region in the waveform editor.
    pub fn action_spectral_attenuate(&mut self, min_freq_hz: f32, max_freq_hz: f32, atten_db: f32) {
        self.ensure_selected_clip_loaded_in_place();
        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames() as usize;
            if total == 0 {
                return;
            }
            let (s, e) = self
                .waveform_state
                .selection
                .map(|(a, b)| (a.min(b) as usize, a.max(b) as usize))
                .unwrap_or((0, total));
            let sr = doc.sample_rate as f32;
            self.waveform_state.history.push_state(doc.clone());
            doc.apply_processor_to_range(0, total as u64, &mut self.source_buffers, |channels| {
                let region = stem_dsp::SpectralRegion {
                    start_frame: s,
                    end_frame: e,
                    min_freq_hz,
                    max_freq_hz,
                };
                for ch in channels.iter_mut() {
                    stem_dsp::SpectralRepair::attenuate_region(ch, sr, &region, atten_db);
                }
            });
            self.status_text = format!(
                "Spectral attenuate ({:.0}-{:.0} Hz, {:.1} dB)",
                min_freq_hz, max_freq_hz, atten_db
            );
            applied = true;
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    /// Non-destructive STFT spectral interpolation (healing) of the selected time-frequency region.
    pub fn action_spectral_heal(&mut self, min_freq_hz: f32, max_freq_hz: f32) {
        self.ensure_selected_clip_loaded_in_place();
        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames() as usize;
            if total == 0 {
                return;
            }
            let (s, e) = self
                .waveform_state
                .selection
                .map(|(a, b)| (a.min(b) as usize, a.max(b) as usize))
                .unwrap_or((0, total));
            let sr = doc.sample_rate as f32;
            self.waveform_state.history.push_state(doc.clone());
            doc.apply_processor_to_range(0, total as u64, &mut self.source_buffers, |channels| {
                let region = stem_dsp::SpectralRegion {
                    start_frame: s,
                    end_frame: e,
                    min_freq_hz,
                    max_freq_hz,
                };
                for ch in channels.iter_mut() {
                    stem_dsp::SpectralRepair::interpolate_region(ch, sr, &region);
                }
            });
            self.status_text = format!("Spectral heal ({:.0}-{:.0} Hz)", min_freq_hz, max_freq_hz);
            applied = true;
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    /// Non-destructive 50/60 Hz mains hum and harmonic notch removal.
    pub fn action_apply_dehum(&mut self, base_freq_hz: f32) {
        self.ensure_selected_clip_loaded_in_place();
        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            if total == 0 {
                return;
            }
            let (s, e) = self
                .waveform_state
                .selection
                .map(|(a, b)| (a.min(b), a.max(b)))
                .unwrap_or((0, total));
            let sr = doc.sample_rate as f32;
            self.waveform_state.history.push_state(doc.clone());
            doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                for ch in channels.iter_mut() {
                    let mut hum = stem_dsp::HumRemover::new(sr, base_freq_hz, 8.0);
                    hum.process_channel(ch);
                }
            });
            self.status_text = format!("De-Hum ({:.0} Hz) applied", base_freq_hz);
            applied = true;
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    /// Non-destructive peak de-clipping restoration.
    pub fn action_apply_declip(&mut self) {
        self.ensure_selected_clip_loaded_in_place();
        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            if total == 0 {
                return;
            }
            let (s, e) = self
                .waveform_state
                .selection
                .map(|(a, b)| (a.min(b), a.max(b)))
                .unwrap_or((0, total));
            self.waveform_state.history.push_state(doc.clone());
            let mut count = 0usize;
            doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                let declip = stem_dsp::DeClipper::new(0.97);
                for ch in channels.iter_mut() {
                    count += declip.process_channel(ch);
                }
            });
            self.status_text = format!("De-Clip repaired {} peak(s)", count);
            applied = true;
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    /// Non-destructive broadband de-reverberation tail suppression.
    pub fn action_apply_dereverb(&mut self) {
        self.ensure_selected_clip_loaded_in_place();
        let mut applied = false;
        if let Some(ref mut doc) = self.waveform_state.document {
            let total = doc.total_frames();
            if total == 0 {
                return;
            }
            let (s, e) = self
                .waveform_state
                .selection
                .map(|(a, b)| (a.min(b), a.max(b)))
                .unwrap_or((0, total));
            let sr = doc.sample_rate as f32;
            self.waveform_state.history.push_state(doc.clone());
            doc.apply_processor_to_range(s, e, &mut self.source_buffers, |channels| {
                let dereverb = stem_dsp::DeReverberator::new(sr, 0.55);
                for ch in channels.iter_mut() {
                    dereverb.process_channel(ch);
                }
            });
            self.status_text = "De-Reverb applied".to_string();
            applied = true;
        }
        if applied {
            self.finish_waveform_or_timeline_edit();
        }
    }

    // -------------------------------------------------------------------------
    // Project Actions
    // -------------------------------------------------------------------------
}
