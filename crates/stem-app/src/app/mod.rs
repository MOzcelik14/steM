//! Central application state and eframe::App event loop implementation
mod project_ops;
mod shortcuts;
mod transport_ops;
mod waveform_ops;

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use stem_audio::{
    render_project_mixdown, render_project_tracks, AudioEngine, AudioRecorder, AudioTelemetry,
    ExportSettings, SharedAudioTimeline,
};
use stem_core::history::UndoRedoStack;
use stem_core::peaks::PeakPyramid;
use stem_core::project::{Clip, Project};
use stem_core::waveform::AudioClipboard;
use stem_i18n::{I18n, Locale, TimecodeMode};

use crate::state::{
    AppViewMode, BatchExportDialogState, ClickRemoverDialogState, KeyBindingMap,
    KeybindingsDialogState, LoudnessReportDialogState, MarkerListDialogState,
    NamedMarkerDialogState, NoiseReductionDialogState, PluginBrowserDialogState, RecordingState,
    RecoveryDialogState, StemSeparationDialogState, TimelineViewState, WaveformViewState,
    WorkerMessage,
};
use crate::views::{
    render_dialogs, render_effects_dialog, render_menu_bar, render_mixer_panel,
    render_spectrogram_panel, render_timeline_view, render_transport_bar, render_waveform_editor,
    EffectsRackState, SpectrogramState,
};

pub struct StemApp {
    pub project: Project,
    pub project_file_path: Option<PathBuf>,
    pub i18n: I18n,
    pub history: UndoRedoStack,
    pub timeline_state: TimelineViewState,

    // View Mode & Waveform Editor State
    pub view_mode: AppViewMode,
    pub waveform_state: WaveformViewState,
    pub clipboard: Option<AudioClipboard>,

    // Audio Engine & Buffers
    pub audio_engine: Option<AudioEngine>,
    pub audio_recorder: Option<AudioRecorder>,
    pub telemetry: AudioTelemetry,
    pub is_playing: bool,
    pub is_recording: bool,
    pub recording_state: RecordingState,
    pub source_buffers: HashMap<Uuid, Vec<Vec<f32>>>,
    pub peak_pyramids: HashMap<Uuid, PeakPyramid>,
    pub active_timeline: Option<Arc<SharedAudioTimeline>>,

    // Transport & Timecode
    pub timecode_mode: TimecodeMode,

    // Background workers
    pub worker_tx: Sender<WorkerMessage>,
    pub worker_rx: Receiver<WorkerMessage>,

    // UI Panels & Views
    pub show_mixer_panel: bool,
    pub show_spectrogram_panel: bool,
    pub show_history_panel: bool,
    pub show_preferences_dialog: bool,
    pub show_export_dialog: bool,
    pub show_about_dialog: bool,
    pub show_unsaved_close_dialog: bool,
    pub force_close: bool,
    pub logo_texture: Option<egui::TextureHandle>,
    pub show_effects_dialog: bool,
    pub metronome_enabled: bool,
    pub max_decode_bytes: usize,

    // Export State
    pub export_destination_path: Option<String>,
    pub export_settings: ExportSettings,
    pub export_progress: Option<f32>,
    pub export_cancel_token: Option<Arc<std::sync::atomic::AtomicBool>>,

    // Effects & Analysis
    pub effects_state: EffectsRackState,
    pub spectrogram_state: SpectrogramState,
    pub noise_reduction_state: NoiseReductionDialogState,
    pub click_remover_state: ClickRemoverDialogState,
    pub loudness_report_state: LoudnessReportDialogState,

    // Phase F - Markers, Recovery & Keybindings
    pub named_marker_state: NamedMarkerDialogState,
    pub marker_list_state: MarkerListDialogState,
    pub batch_export_state: BatchExportDialogState,
    pub keybindings_state: KeybindingsDialogState,
    pub recovery_state: RecoveryDialogState,
    pub plugin_browser_state: PluginBrowserDialogState,
    pub stem_separation_state: StemSeparationDialogState,
    pub keybindings: KeyBindingMap,
    pub auto_save_last_time: std::time::Instant,
    pub auto_save_interval_sec: u64,
    pub recovery_file_path: PathBuf,

    // Status
    pub status_text: String,
}

impl StemApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let i18n = I18n::new(Locale::from_system());
        let (worker_tx, worker_rx) = unbounded();

        let mut audio_engine = match AudioEngine::new() {
            Ok(engine) => Some(engine),
            Err(err) => {
                eprintln!("steM. audio engine init warning: {err}");
                None
            }
        };

        let monitor_tx = audio_engine
            .as_mut()
            .and_then(|e| e.take_monitor_producer());

        let audio_recorder = match AudioRecorder::new(monitor_tx) {
            Ok(rec) => Some(rec),
            Err(err) => {
                eprintln!("steM. audio recorder init warning: {err}");
                None
            }
        };

        let available_inputs = AudioRecorder::available_input_devices();
        let selected_input = audio_recorder.as_ref().map(|r| r.device_name().to_string());
        let available_outputs = AudioEngine::available_output_devices();
        let selected_output = audio_engine.as_ref().map(|e| e.device_name().to_string());
        let selected_sr = audio_engine
            .as_ref()
            .map(|e| e.sample_rate())
            .or_else(|| audio_recorder.as_ref().map(|r| r.sample_rate()))
            .unwrap_or(48000);

        let recording_state = RecordingState {
            available_input_devices: available_inputs,
            selected_input_device: selected_input,
            available_output_devices: available_outputs,
            selected_output_device: selected_output,
            selected_sample_rate: selected_sr,
            ..Default::default()
        };

        let project = Project::new("Untitled Project", selected_sr);
        let status_text = i18n.t("status-ready");

        let recovery_file_path = default_recovery_file_path();
        stem_audio::stems::cleanup_stale_temp_artifacts(std::time::Duration::from_secs(86_400));

        let mut recovery_state = RecoveryDialogState::default();
        if recovery_file_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&recovery_file_path) {
                if metadata.len() > 0 {
                    recovery_state.show = true;
                    recovery_state.recovery_path = Some(recovery_file_path.clone());
                    if let Ok(modified) = metadata.modified() {
                        let duration = std::time::SystemTime::now()
                            .duration_since(modified)
                            .unwrap_or_default();
                        recovery_state.recovery_time_str = format!("{}s ago", duration.as_secs());
                    }
                }
            }
        }

        Self {
            project,
            project_file_path: None,
            i18n,
            history: UndoRedoStack::new(),
            timeline_state: TimelineViewState::default(),

            view_mode: AppViewMode::Multitrack,
            waveform_state: WaveformViewState::default(),
            clipboard: None,

            audio_engine,
            audio_recorder,
            telemetry: AudioTelemetry::default(),
            is_playing: false,
            is_recording: false,
            recording_state,
            source_buffers: HashMap::new(),
            peak_pyramids: HashMap::new(),
            active_timeline: None,

            timecode_mode: TimecodeMode::default(),

            worker_tx,
            worker_rx,

            show_mixer_panel: false,
            show_spectrogram_panel: false,
            show_history_panel: false,
            show_preferences_dialog: false,
            show_export_dialog: false,
            show_about_dialog: false,
            show_unsaved_close_dialog: false,
            force_close: false,
            logo_texture: None,
            show_effects_dialog: false,
            metronome_enabled: false,
            max_decode_bytes: stem_audio::decode::DEFAULT_MAX_DECODE_BYTES,

            export_destination_path: None,
            export_settings: ExportSettings::default(),
            export_progress: None,
            export_cancel_token: None,

            effects_state: EffectsRackState::default(),
            spectrogram_state: SpectrogramState::default(),
            noise_reduction_state: NoiseReductionDialogState::default(),
            click_remover_state: ClickRemoverDialogState::default(),
            loudness_report_state: LoudnessReportDialogState::default(),

            named_marker_state: NamedMarkerDialogState::default(),
            marker_list_state: MarkerListDialogState::default(),
            batch_export_state: BatchExportDialogState::default(),
            keybindings_state: KeybindingsDialogState::default(),
            recovery_state,
            plugin_browser_state: PluginBrowserDialogState::default(),
            stem_separation_state: StemSeparationDialogState::default(),
            keybindings: KeyBindingMap::default(),
            auto_save_last_time: std::time::Instant::now(),
            auto_save_interval_sec: 60,
            recovery_file_path,

            status_text,
        }
    }

    /// Returns the active audio engine sample rate (falling back to selected/default rate).
    pub fn active_sample_rate(&self) -> u32 {
        self.audio_engine
            .as_ref()
            .map(|e| e.sample_rate())
            .unwrap_or(self.recording_state.selected_sample_rate)
    }

    /// Re-renders project mixdown into a SharedAudioTimeline buffer and sends it to the real-time engine.
    pub fn sync_timeline_to_audio_engine(&mut self) {
        if let Ok(mut mix) = render_project_mixdown(&self.project, &self.source_buffers, None) {
            let mut total_frames = self.project.total_frames();
            let mut track_channels = render_project_tracks(&self.project, &self.source_buffers);

            if let Some(ref engine) = self.audio_engine {
                let engine_sr = engine.sample_rate();
                if engine_sr != self.project.sample_rate && self.project.sample_rate > 0 {
                    if let Ok(resampled_mix) =
                        stem_dsp::resample_audio(&mix, self.project.sample_rate, engine_sr)
                    {
                        total_frames = resampled_mix.first().map(|c| c.len() as u64).unwrap_or(0);
                        mix = resampled_mix;
                    }
                    for trk in &mut track_channels {
                        if let Ok(resampled_trk) =
                            stem_dsp::resample_audio(trk, self.project.sample_rate, engine_sr)
                        {
                            *trk = resampled_trk;
                        }
                    }
                }
            }

            let timeline = Arc::new(SharedAudioTimeline {
                channels: mix,
                track_channels,
                total_frames,
            });
            self.active_timeline = Some(timeline.clone());
            if let Some(ref mut engine) = self.audio_engine {
                engine.sync_project_mixer(&self.project);
                engine.update_timeline(timeline);
            }
        }
    }
}

impl eframe::App for StemApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Intercept window close button if there are unsaved changes
        if !self.force_close
            && ctx.input(|i| i.viewport().close_requested())
            && self.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_unsaved_close_dialog = true;
        }
        // Continuous repaint during playback, recording, count-in, or input monitoring
        if self.is_playing
            || self.is_recording
            || self.recording_state.count_in_active
            || self.project.input_monitoring
        {
            ctx.request_repaint();
        }

        // Handle global keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Periodic auto-save crash recovery journal
        if self.auto_save_last_time.elapsed().as_secs() >= self.auto_save_interval_sec {
            self.save_recovery_journal();
            self.auto_save_last_time = std::time::Instant::now();
        }

        // Check for audio hardware disconnection or recording disk write errors
        let mut lost_device = false;
        let mut rec_write_err = false;
        if let Some(ref recorder) = self.audio_recorder {
            if recorder.is_device_lost() {
                lost_device = true;
            }
            if recorder.has_write_error() {
                rec_write_err = true;
            }
        }
        if rec_write_err {
            self.status_text = self.i18n.t("recording-write-error");
        }
        if lost_device {
            if self.is_recording {
                self.action_toggle_record();
            }
            self.recording_state.device_lost_warning = true;
            self.audio_recorder = None;
            self.status_text = self.i18n.t("recording-device-lost");
        }

        // Check for audio output hardware disconnection
        let mut lost_output = false;
        if let Some(ref engine) = self.audio_engine {
            if engine.is_device_lost() {
                lost_output = true;
            }
        }
        if lost_output {
            self.action_stop();
            self.audio_engine = None;
            self.status_text = self.i18n.t("recording-device-lost");
        }

        // Drain telemetry from real-time audio thread
        if let Some(ref mut engine) = self.audio_engine {
            self.telemetry = engine.poll_telemetry();
            self.is_playing = engine.is_playing();
        }

        // Drain input telemetry from audio recorder
        if let Some(ref mut recorder) = self.audio_recorder {
            self.recording_state.input_telemetry = recorder.telemetry();
        }

        // Count-in countdown progression
        if self.recording_state.count_in_active {
            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            self.recording_state.count_in_seconds_remaining -= dt;
            if self.recording_state.count_in_seconds_remaining <= 0.0 {
                self.recording_state.count_in_active = false;
                self.start_actual_recording();
            }
        }

        // Punch-in boundary checking during playback
        if self.is_recording && self.project.punch_in_enabled {
            if let Some((punch_in, punch_out)) = self.project.punch_region {
                let current_frame = self.telemetry.playhead_frame;
                // Transition into punch recording
                if !self.recording_state.is_punching
                    && current_frame >= punch_in
                    && current_frame < punch_out
                {
                    self.recording_state.is_punching = true;
                    self.recording_state.record_start_frame = punch_in;
                    if let Some(ref recorder) = self.audio_recorder {
                        recorder.start_recording();
                    }
                    self.status_text = self.i18n.t("status-recording");
                }
                // Transition out of punch recording
                else if self.recording_state.is_punching && current_frame >= punch_out {
                    self.action_toggle_record();
                }
            }
        }

        if self.view_mode == AppViewMode::Waveform && self.is_playing {
            let doc_sr = self
                .waveform_state
                .document
                .as_ref()
                .map(|d| d.sample_rate)
                .unwrap_or(self.project.sample_rate)
                .max(1);
            let engine_sr = self.active_sample_rate().max(1);
            let doc_playhead = if doc_sr != engine_sr {
                ((self.telemetry.playhead_frame as f64) * (doc_sr as f64 / engine_sr as f64))
                    .round() as u64
            } else {
                self.telemetry.playhead_frame
            };
            self.waveform_state.playhead_frame = doc_playhead;
            if !self.waveform_state.loop_selection {
                if let Some((_, end)) = self.waveform_state.selection {
                    if doc_playhead >= end {
                        self.action_toggle_play();
                        self.action_seek_waveform(end);
                    }
                }
            }
        } else if self.view_mode == AppViewMode::Multitrack
            && self.is_playing
            && !self.project.loop_enabled
        {
            if let Some((_, end)) = self.timeline_state.time_selection {
                if self.telemetry.playhead_frame >= end {
                    self.action_toggle_play();
                    self.action_seek(end);
                }
            }
        }

        // Drain messages from background workers (decoding, peak caching, export)
        while let Ok(msg) = self.worker_rx.try_recv() {
            match msg {
                WorkerMessage::ImportComplete {
                    decoded,
                    pyramid,
                    target_track_idx,
                    target_offset_frames,
                } => {
                    let source = decoded.source.clone();
                    let source_id = source.id;
                    let source_frames = source.duration_frames;
                    let filename = source.filename.clone();

                    self.project.add_source(source);
                    self.source_buffers.insert(source_id, decoded.channels);
                    self.peak_pyramids.insert(source_id, *pyramid);

                    // Add clip on requested track (for drag & drop) or first available track
                    if !self.project.tracks.is_empty() {
                        let t_idx = target_track_idx
                            .unwrap_or(0)
                            .min(self.project.tracks.len().saturating_sub(1));
                        let offset = target_offset_frames.unwrap_or(0);
                        let track_id = self.project.tracks[t_idx].id;
                        let clip = Clip::new(&filename, source_id, 0, source_frames, offset);
                        let _ = self.project.add_clip_to_track(track_id, clip);
                    }

                    self.status_text =
                        self.i18n
                            .t_arg("status-imported-file", "filename", filename);
                    self.sync_timeline_to_audio_engine();
                }
                WorkerMessage::SourceReloaded {
                    source_id,
                    channels,
                    pyramid,
                } => {
                    self.source_buffers.insert(source_id, channels);
                    self.peak_pyramids.insert(source_id, *pyramid);
                    self.sync_timeline_to_audio_engine();
                }
                WorkerMessage::ImportFailed(err) => {
                    self.status_text = format!("Import failed: {err}");
                }
                WorkerMessage::ExportProgress(pct) => {
                    self.export_progress = Some(pct);
                }
                WorkerMessage::ExportFinished(res) => {
                    self.export_progress = None;
                    self.export_cancel_token = None;
                    match res {
                        Ok(_) => {
                            self.status_text = self.i18n.t("export-status-success");
                            self.show_export_dialog = false;
                        }
                        Err(err) => {
                            if err.to_lowercase().contains("cancel") {
                                self.status_text = self.i18n.t("export-status-cancelled");
                            } else {
                                self.status_text =
                                    self.i18n.t_arg("export-status-error", "reason", err);
                            }
                        }
                    }
                }
                WorkerMessage::BatchExportProgress(pct) => {
                    self.batch_export_state.progress = pct;
                }
                WorkerMessage::BatchExportFinished(summary) => {
                    self.batch_export_state.in_progress = false;
                    self.batch_export_state.cancel_token = None;
                    self.batch_export_state.progress = 1.0;
                    let msg = format!(
                        "Done: {} succeeded, {} failed, {} skipped{}",
                        summary.succeeded,
                        summary.failed,
                        summary.skipped,
                        if summary.cancelled {
                            " (cancelled)"
                        } else {
                            ""
                        }
                    );
                    self.batch_export_state.status_msg = Some(msg);
                    self.batch_export_state.summary = Some(summary);
                }
                WorkerMessage::StemSeparationProgress(pct) => {
                    self.stem_separation_state.progress = pct;
                }
                WorkerMessage::StemSeparationFinished {
                    offset_frames,
                    result,
                } => {
                    self.stem_separation_state.in_progress = false;
                    self.stem_separation_state.cancel_token = None;
                    self.stem_separation_state.progress = 1.0;
                    match result {
                        Ok(stems) => {
                            let count = stems.len();
                            for (kind, decoded, pyramid) in stems {
                                let track_name = kind.track_title().to_string();
                                let source = decoded.source.clone();
                                let source_id = source.id;
                                let dur = source.duration_frames;
                                let fname = source.filename.clone();
                                self.project.add_source(source);
                                self.source_buffers.insert(source_id, decoded.channels);
                                self.peak_pyramids.insert(source_id, pyramid);

                                let target_track_id =
                                    if self.stem_separation_state.replace_existing_stems {
                                        self.project
                                            .tracks
                                            .iter()
                                            .find(|t| t.name == track_name)
                                            .map(|t| t.id)
                                    } else {
                                        None
                                    };

                                let track_id = if let Some(tid) = target_track_id {
                                    if let Some(tr) = self.project.get_track_mut(tid) {
                                        tr.clips.clear();
                                    }
                                    tid
                                } else if self.project.tracks.len() < stem_core::MAX_PROJECT_TRACKS
                                {
                                    self.project.add_track(&track_name)
                                } else {
                                    continue;
                                };

                                let clip = Clip::new(&fname, source_id, 0, dur, offset_frames);
                                let _ = self.project.add_clip_to_track(track_id, clip);
                            }
                            self.sync_timeline_to_audio_engine();
                            self.stem_separation_state.status_msg =
                                Some(format!("Separated {count} synchronized stem(s)"));
                            self.status_text = format!("Demucs separated {count} stem(s)");
                        }
                        Err(err) => {
                            self.stem_separation_state.status_msg = Some(err.clone());
                            self.status_text = format!("Stem separation failed: {err}");
                        }
                    }
                }
            }
        }

        // ----------------- Top Menu Bar -----------------
        egui::TopBottomPanel::top("menu_panel").show(ctx, |ui| {
            render_menu_bar(self, ui);
        });

        // ----------------- Transport Panel -----------------
        egui::TopBottomPanel::top("transport_panel").show(ctx, |ui| {
            render_transport_bar(self, ui);
        });

        // ----------------- Status Bar (Bottom) -----------------
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.status_text)
                        .size(11.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} Hz", self.project.sample_rate))
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(stem_i18n::APP_DISPLAY_NAME)
                            .strong()
                            .size(11.0)
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                });
            });
        });

        if self.view_mode == AppViewMode::Multitrack {
            // ----------------- Bottom Mixer Panel (Collapsible) -----------------
            if self.show_mixer_panel {
                egui::TopBottomPanel::bottom("mixer_panel")
                    .resizable(true)
                    .default_height(200.0)
                    .min_height(140.0)
                    .show(ctx, |ui| {
                        render_mixer_panel(self, ui);
                    });
            }

            // ----------------- Bottom Spectrogram Panel (Collapsible) -----------------
            if self.show_spectrogram_panel {
                egui::TopBottomPanel::bottom("spectrogram_panel")
                    .resizable(true)
                    .default_height(140.0)
                    .show(ctx, |ui| {
                        render_spectrogram_panel(self, ui);
                    });
            }

            // ----------------- Central Timeline Canvas -----------------
            egui::CentralPanel::default().show(ctx, |ui| {
                render_timeline_view(self, ui);
            });
        } else {
            // ----------------- Waveform Editor Canvas -----------------
            egui::CentralPanel::default().show(ctx, |ui| {
                render_waveform_editor(self, ui);
            });
        }

        // ----------------- Modal Dialogs -----------------
        render_dialogs(self, ctx);
        render_effects_dialog(self, ctx);
    }
}

impl Drop for StemApp {
    fn drop(&mut self) {
        if self.stem_separation_state.in_progress {
            if let Some(ref tok) = self.stem_separation_state.cancel_token {
                tok.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        self.discard_recovery_journal();
    }
}

fn default_recovery_file_path() -> PathBuf {
    let base_dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base_dir.join("steM").join("recovery.stem")
}
