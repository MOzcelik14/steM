use super::StemApp;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use stem_audio::{export_project_to_file, ExportSettings};
use stem_core::history::UndoRedoStack;
use stem_core::project::Project;

use crate::state::WorkerMessage;

impl StemApp {
    pub fn action_new_project(&mut self) {
        self.action_stop();
        let sr = self.active_sample_rate();
        self.project = Project::new("Untitled Project", sr);
        self.project_file_path = None;
        self.source_buffers.clear();
        self.peak_pyramids.clear();
        self.history = UndoRedoStack::new();
        self.active_timeline = None;
        self.sync_timeline_to_audio_engine();
        self.status_text = self.i18n.t("status-ready");
    }

    pub fn action_new_project_from_template(
        &mut self,
        template: stem_core::project::ProjectTemplate,
    ) {
        self.action_stop();
        let sr = self.active_sample_rate();
        self.project = Project::from_template(template, sr);
        self.project_file_path = None;
        self.source_buffers.clear();
        self.peak_pyramids.clear();
        self.history = UndoRedoStack::new();
        self.active_timeline = None;
        self.sync_timeline_to_audio_engine();
        self.status_text = self.i18n.t("status-ready");
    }

    pub fn action_load_demo(&mut self) {
        self.action_stop();
        let sr = self.active_sample_rate();
        let (demo_proj, demo_buffers) = stem_core::generate_demo_project(sr);
        self.project = demo_proj;
        self.project_file_path = None;
        self.source_buffers = demo_buffers;
        self.peak_pyramids.clear();

        // Build multi-resolution peak pyramids for all demo tracks
        for (src_id, buf) in &self.source_buffers {
            let channel_refs: Vec<&[f32]> = buf.iter().map(|ch| ch.as_slice()).collect();
            let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
            self.peak_pyramids.insert(*src_id, pyramid);
        }

        self.history = UndoRedoStack::new();
        self.status_text = self.i18n.t("status-demo-loaded");
        self.sync_timeline_to_audio_engine();
    }

    fn spawn_reload_project_sources(&self) {
        let target_sr = self.project.sample_rate;
        for (&source_id, source) in &self.project.sources {
            let tx = self.worker_tx.clone();
            let path = source.path.clone();
            std::thread::spawn(move || {
                if let Ok(decoded) = stem_audio::decode_file(&path) {
                    let channels = if decoded.source.sample_rate != target_sr && target_sr > 0 {
                        stem_dsp::resample_audio(
                            &decoded.channels,
                            decoded.source.sample_rate,
                            target_sr,
                        )
                        .unwrap_or(decoded.channels)
                    } else {
                        decoded.channels
                    };
                    let channel_refs: Vec<&[f32]> =
                        channels.iter().map(|ch| ch.as_slice()).collect();
                    let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
                    let _ = tx.send(WorkerMessage::SourceReloaded {
                        source_id,
                        channels,
                        pyramid: Box::new(pyramid),
                    });
                }
            });
        }
    }

    fn write_atomic_file(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
        stem_core::atomic_write_bytes(path, contents.as_bytes())
    }

    pub fn action_open_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("steM Project", &["stem", "STEM", "ron", "RON"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(ron_str) => match Project::from_ron(&ron_str) {
                    Ok(proj) => {
                        self.action_stop();
                        self.project = proj;
                        self.project_file_path = Some(path);
                        self.source_buffers.clear();
                        self.peak_pyramids.clear();
                        self.history = Default::default();
                        self.spawn_reload_project_sources();
                        self.sync_timeline_to_audio_engine();
                        self.status_text = self.i18n.t("status-ready");
                    }
                    Err(e) => {
                        self.status_text = format!("Open error: {e}");
                    }
                },
                Err(e) => {
                    self.status_text = format!("Read error: {e}");
                }
            }
        }
    }

    pub fn action_save_project(&mut self) {
        if let Some(ref path) = self.project_file_path {
            match self.project.to_ron() {
                Ok(ron_str) => match Self::write_atomic_file(path, &ron_str) {
                    Ok(()) => {
                        self.status_text = self.i18n.t("status-project-saved");
                    }
                    Err(e) => {
                        self.status_text = format!("Save error: {e}");
                    }
                },
                Err(e) => {
                    self.status_text = format!("Save error: {e}");
                }
            }
        } else {
            self.action_save_project_as();
        }
    }

    pub fn action_save_project_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("steM Project", &["stem", "STEM", "ron", "RON"])
            .save_file()
        {
            match self.project.to_ron() {
                Ok(ron_str) => match Self::write_atomic_file(&path, &ron_str) {
                    Ok(()) => {
                        self.project_file_path = Some(path);
                        self.status_text = self.i18n.t("status-project-saved");
                    }
                    Err(e) => {
                        self.status_text = format!("Save error: {e}");
                    }
                },
                Err(e) => {
                    self.status_text = format!("Save error: {e}");
                }
            }
        }
    }

    pub fn import_audio_path(
        &mut self,
        path: PathBuf,
        target_track_idx: Option<usize>,
        target_offset_frames: Option<u64>,
    ) {
        self.status_text = self.i18n.t("status-loading");
        let tx = self.worker_tx.clone();
        let target_sr = self.project.sample_rate;

        std::thread::spawn(move || match stem_audio::decode_file(&path) {
            Ok(mut decoded) => {
                if decoded.source.sample_rate != target_sr {
                    if let Ok(resampled) = stem_dsp::resample_audio(
                        &decoded.channels,
                        decoded.source.sample_rate,
                        target_sr,
                    ) {
                        decoded.source.duration_frames =
                            resampled.first().map(|c| c.len() as u64).unwrap_or(0);
                        decoded.channels = resampled;
                        decoded.source.sample_rate = target_sr;
                    }
                }
                let channel_refs: Vec<&[f32]> =
                    decoded.channels.iter().map(|ch| ch.as_slice()).collect();
                let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
                let _ = tx.send(WorkerMessage::ImportComplete {
                    decoded: Box::new(decoded),
                    pyramid: Box::new(pyramid),
                    target_track_idx,
                    target_offset_frames,
                });
            }
            Err(e) => {
                let _ = tx.send(WorkerMessage::ImportFailed(e.to_string()));
            }
        });
    }

    pub fn action_import_audio(&mut self) {
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
            self.import_audio_path(path, None, None);
        }
    }

    pub fn action_set_metronome(&mut self, enabled: bool) {
        self.metronome_enabled = enabled;
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_metronome(
                enabled,
                self.project.tempo_bpm,
                self.project.time_signature.0,
                self.project.sample_rate,
            );
        }
    }

    pub fn action_ripple_delete_selection(&mut self) {
        if let Some((start, end)) = self.timeline_state.time_selection {
            let target_track = self.timeline_state.selected_track_id;
            self.project.ripple_delete_range(start, end, target_track);
            self.timeline_state.time_selection = None;
            self.sync_timeline_to_audio_engine();
        }
    }

    pub fn action_silence_selection(&mut self) {
        if let Some((start, end)) = self.timeline_state.time_selection {
            let target_track = self.timeline_state.selected_track_id;
            self.project.silence_range(start, end, target_track);
            self.sync_timeline_to_audio_engine();
        }
    }

    pub fn action_split_at_selection(&mut self) {
        if let Some((start, end)) = self.timeline_state.time_selection {
            let target_track = self.timeline_state.selected_track_id;
            self.project.split_all_at_frame(start, target_track);
            self.project.split_all_at_frame(end, target_track);
            self.sync_timeline_to_audio_engine();
        }
    }

    pub fn action_export_chapter_markers(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("chapters.txt")
            .add_filter("Text", &["txt"])
            .save_file()
        {
            let sr = self.project.sample_rate.max(1) as f64;
            let mut markers = self.project.markers.clone();
            markers.sort_by_key(|m| m.frame);
            let mut out = String::new();
            for m in markers {
                let sec = m.frame as f64 / sr;
                let mins = (sec / 60.0).floor() as u64;
                let rem = sec - (mins as f64 * 60.0);
                out.push_str(&format!("{mins:02}:{rem:06.3} {}\n", m.name));
            }
            match std::fs::write(&path, out) {
                Ok(_) => {
                    self.status_text = self.i18n.t("export-chapters-completed");
                }
                Err(e) => {
                    self.status_text = format!("Export error: {e}");
                }
            }
        }
    }

    pub fn action_start_export(&mut self) {
        let dest = match self.export_destination_path {
            Some(ref p) => PathBuf::from(p),
            None => return,
        };

        let proj = self.project.clone();
        let buffers = self.source_buffers.clone();
        let settings = self.export_settings.clone();
        let tx = self.worker_tx.clone();

        let cancel_token = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.export_cancel_token = Some(cancel_token.clone());

        self.export_progress = Some(0.0);
        self.status_text = self.i18n.t("export-status-in-progress");

        std::thread::spawn(move || {
            let (prog_tx, prog_rx) = crossbeam_channel::unbounded();
            let tx_clone = tx.clone();

            // Progress monitor thread
            std::thread::spawn(move || {
                while let Ok(progress) = prog_rx.recv() {
                    let _ = tx_clone.send(WorkerMessage::ExportProgress(progress));
                }
            });

            match export_project_to_file(
                &proj,
                &buffers,
                &dest,
                &settings,
                Some(prog_tx),
                Some(cancel_token),
            ) {
                Ok(_) => {
                    let _ = tx.send(WorkerMessage::ExportFinished(Ok(dest
                        .to_string_lossy()
                        .to_string())));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMessage::ExportFinished(Err(e.to_string())));
                }
            }
        });
    }

    pub fn action_cancel_export(&mut self) {
        if let Some(ref token) = self.export_cancel_token {
            token.store(true, std::sync::atomic::Ordering::SeqCst);
            self.status_text = self.i18n.t("export-status-cancelled");
        }
        self.export_progress = None;
        self.export_cancel_token = None;
    }

    // -------------------------------------------------------------------------
    // Playback & Transport Actions
    // -------------------------------------------------------------------------

    pub fn save_recovery_journal(&self) {
        if let Ok(ron_data) = self.project.to_ron() {
            let _ = stem_core::atomic_write_bytes(&self.recovery_file_path, ron_data.as_bytes());
        }
    }

    pub fn restore_recovery_journal(&mut self) -> bool {
        if let Ok(ron_data) = std::fs::read_to_string(&self.recovery_file_path) {
            if let Ok(restored_project) = Project::from_ron(&ron_data) {
                self.project = restored_project;
                self.source_buffers.clear();
                self.peak_pyramids.clear();
                self.spawn_reload_project_sources();
                self.sync_timeline_to_audio_engine();
                self.recovery_state.show = false;
                self.status_text = self.i18n.t("recovery-restored");
                return true;
            }
        }
        false
    }

    pub fn discard_recovery_journal(&mut self) {
        if self.recovery_file_path.exists() {
            let _ = std::fs::remove_file(&self.recovery_file_path);
        }
        self.recovery_state.show = false;
        self.status_text = self.i18n.t("recovery-discarded");
    }

    pub fn run_batch_export(&mut self) {
        if self.batch_export_state.in_progress {
            return;
        }

        let output_dir = self.batch_export_state.output_dir.clone();
        let project = self.project.clone();
        let source_buffers = self.source_buffers.clone();
        let pattern = self.batch_export_state.filename_pattern.clone();
        let format = self.batch_export_state.format;
        let bit_depth = self.batch_export_state.bit_depth;
        let export_stems = self.batch_export_state.export_stems;
        let export_marker_regions = self.batch_export_state.export_marker_regions;
        let overwrite_existing = self.batch_export_state.overwrite_existing;

        let cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_clone = cancel_token.clone();
        let tx = self.worker_tx.clone();

        self.batch_export_state.in_progress = true;
        self.batch_export_state.progress = 0.0;
        self.batch_export_state.cancel_token = Some(cancel_token);
        self.batch_export_state.status_msg = None;
        self.batch_export_state.summary = None;

        std::thread::spawn(move || {
            use crate::state::{BatchExportItemOutcome, BatchExportSummary, BatchItemStatus};
            use std::sync::atomic::Ordering;

            let mut summary = BatchExportSummary::default();
            if !output_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&output_dir) {
                    summary.failed = 1;
                    summary.items.push(BatchExportItemOutcome {
                        filename: output_dir.display().to_string(),
                        status: BatchItemStatus::Failed(e.to_string()),
                    });
                    let _ = tx.send(WorkerMessage::BatchExportFinished(summary));
                    return;
                }
            }

            let ext = match format {
                stem_audio::ExportFormat::Wav => "wav",
                stem_audio::ExportFormat::Flac => "flac",
                stem_audio::ExportFormat::Mp3 => "mp3",
                stem_audio::ExportFormat::Ogg => "ogg",
            };

            let stem_count = if export_stems {
                project.tracks.len()
            } else {
                0
            };
            let region_count = if export_marker_regions && project.markers.len() >= 2 {
                project.markers.len() - 1
            } else {
                0
            };
            let total_jobs = (stem_count + region_count).max(1);
            let mut completed_jobs = 0usize;

            if export_stems {
                for (idx, track) in project.tracks.iter().enumerate() {
                    if cancel_clone.load(Ordering::Relaxed) {
                        summary.cancelled = true;
                        break;
                    }

                    let fname = stem_audio::format_export_filename(
                        &pattern,
                        &project.name,
                        &track.name,
                        idx,
                        "",
                    );
                    let display_name = format!("{fname}.{ext}");
                    let out_file = output_dir.join(&display_name);

                    if out_file.exists() && !overwrite_existing {
                        summary.skipped += 1;
                        summary.items.push(BatchExportItemOutcome {
                            filename: display_name,
                            status: BatchItemStatus::Skipped(
                                "File already exists (overwrite disabled)".to_string(),
                            ),
                        });
                    } else {
                        let settings = ExportSettings {
                            format,
                            bit_depth,
                            sample_rate: project.sample_rate,
                            range: stem_audio::ExportRange::EntireProject,
                            apply_dither: bit_depth != stem_audio::BitDepth::Float32,
                            normalize_target_lufs: None,
                            ..ExportSettings::default()
                        };
                        let res = stem_audio::render_track_stem(
                            &project,
                            &source_buffers,
                            idx,
                            stem_audio::ExportRange::EntireProject,
                        )
                        .and_then(|stem_buf| {
                            stem_audio::export_buffer_to_file(&stem_buf, &out_file, &settings)
                        });
                        match res {
                            Ok(()) => {
                                summary.succeeded += 1;
                                summary.items.push(BatchExportItemOutcome {
                                    filename: display_name,
                                    status: BatchItemStatus::Succeeded,
                                });
                            }
                            Err(e) => {
                                summary.failed += 1;
                                summary.items.push(BatchExportItemOutcome {
                                    filename: display_name,
                                    status: BatchItemStatus::Failed(e.to_string()),
                                });
                            }
                        }
                    }

                    completed_jobs += 1;
                    let _ = tx.send(WorkerMessage::BatchExportProgress(
                        completed_jobs as f32 / total_jobs as f32,
                    ));
                }
            }

            if !summary.cancelled && export_marker_regions && project.markers.len() >= 2 {
                for i in 0..(project.markers.len() - 1) {
                    if cancel_clone.load(Ordering::Relaxed) {
                        summary.cancelled = true;
                        break;
                    }

                    let m_start = &project.markers[i];
                    let m_end = &project.markers[i + 1];
                    let fname = stem_audio::format_export_filename(
                        &pattern,
                        &project.name,
                        "Master",
                        i,
                        &m_start.name,
                    );
                    let display_name = format!("{fname}.{ext}");
                    let out_file = output_dir.join(&display_name);

                    if out_file.exists() && !overwrite_existing {
                        summary.skipped += 1;
                        summary.items.push(BatchExportItemOutcome {
                            filename: display_name,
                            status: BatchItemStatus::Skipped(
                                "File already exists (overwrite disabled)".to_string(),
                            ),
                        });
                    } else {
                        let range = stem_audio::ExportRange::TimeRange {
                            start_frame: m_start.frame,
                            end_frame: m_end.frame,
                        };
                        let settings = ExportSettings {
                            format,
                            bit_depth,
                            sample_rate: project.sample_rate,
                            range,
                            apply_dither: bit_depth != stem_audio::BitDepth::Float32,
                            normalize_target_lufs: None,
                            ..ExportSettings::default()
                        };
                        match stem_audio::export_project_to_file(
                            &project,
                            &source_buffers,
                            &out_file,
                            &settings,
                            None,
                            Some(cancel_clone.clone()),
                        ) {
                            Ok(()) => {
                                summary.succeeded += 1;
                                summary.items.push(BatchExportItemOutcome {
                                    filename: display_name,
                                    status: BatchItemStatus::Succeeded,
                                });
                            }
                            Err(e) => {
                                if e.to_string().to_lowercase().contains("cancel") {
                                    summary.cancelled = true;
                                }
                                summary.failed += 1;
                                summary.items.push(BatchExportItemOutcome {
                                    filename: display_name,
                                    status: BatchItemStatus::Failed(e.to_string()),
                                });
                            }
                        }
                    }

                    completed_jobs += 1;
                    let _ = tx.send(WorkerMessage::BatchExportProgress(
                        completed_jobs as f32 / total_jobs as f32,
                    ));
                }
            }

            let _ = tx.send(WorkerMessage::BatchExportFinished(summary));
        });
    }

    pub fn action_start_stem_separation(&mut self) {
        if self.stem_separation_state.in_progress {
            return;
        }
        let sources: Vec<_> = self.project.sources.values().cloned().collect();
        let Some(src) = sources.get(self.stem_separation_state.source_idx).cloned() else {
            self.stem_separation_state.status_msg = Some("No audio source available".to_string());
            return;
        };

        // Find original clip offset so separated stems remain synchronized on the timeline
        let offset_frames = self
            .project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.source_id == src.id)
            .map(|c| c.track_offset_frames)
            .unwrap_or(0);

        let model = self.stem_separation_state.model;
        let quality = self.stem_separation_state.quality;
        let prefer_gpu = self.stem_separation_state.prefer_gpu;
        let target_sr = self.project.sample_rate;
        let cancel_token = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_clone = cancel_token.clone();
        let tx = self.worker_tx.clone();

        // If source path doesn't exist on disk (e.g. demo or in-memory recording), export a temp WAV first
        let mut temp_input_file: Option<PathBuf> = None;
        let input_path = if src.path.is_file() {
            src.path.clone()
        } else if let Some(buf) = self.source_buffers.get(&src.id) {
            let tmp = std::env::temp_dir().join(format!("stem_demucs_in_{}.wav", src.id));
            let settings = ExportSettings {
                format: stem_audio::ExportFormat::Wav,
                bit_depth: stem_audio::BitDepth::Float32,
                sample_rate: target_sr,
                apply_dither: false,
                ..ExportSettings::default()
            };
            if let Err(e) = stem_audio::export_buffer_to_file(buf, &tmp, &settings) {
                self.stem_separation_state.status_msg = Some(format!("Prep error: {e}"));
                return;
            }
            temp_input_file = Some(tmp.clone());
            tmp
        } else {
            self.stem_separation_state.status_msg = Some("Source audio buffer missing".to_string());
            return;
        };

        let out_dir = std::env::temp_dir().join(format!("stem_demucs_out_{}", Uuid::new_v4()));

        self.stem_separation_state.in_progress = true;
        self.stem_separation_state.progress = 0.0;
        self.stem_separation_state.cancel_token = Some(cancel_token);
        self.stem_separation_state.status_msg = Some("Running Demucs...".to_string());

        std::thread::spawn(move || {
            let req = stem_audio::StemSeparationRequest {
                input_path,
                output_dir: out_dir.clone(),
                model,
                quality,
                prefer_gpu,
                target_sample_rate: target_sr,
            };
            let res = stem_audio::DemucsRunner::run_separation(
                &req,
                |pct| {
                    let _ = tx.send(WorkerMessage::StemSeparationProgress(pct));
                },
                Some(cancel_clone),
            )
            .map(|stems| {
                stems
                    .into_iter()
                    .map(|(kind, decoded)| {
                        let ch_refs: Vec<&[f32]> =
                            decoded.channels.iter().map(|c| c.as_slice()).collect();
                        let pyramid = stem_dsp::build_peak_pyramid(&ch_refs);
                        (kind, decoded, pyramid)
                    })
                    .collect()
            })
            .map_err(|e| e.to_string());

            if let Some(tmp) = temp_input_file {
                let _ = std::fs::remove_file(tmp);
            }
            let _ = std::fs::remove_dir_all(&out_dir);

            let _ = tx.send(WorkerMessage::StemSeparationFinished {
                offset_frames,
                result: res,
            });
        });
    }
}
