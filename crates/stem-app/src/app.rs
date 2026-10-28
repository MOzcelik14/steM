//! Central application state and eframe::App event loop implementation

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use stem_audio::{
    export_project_to_file, render_project_mixdown, AudioEngine, AudioRecorder, AudioTelemetry,
    ExportSettings, SharedAudioTimeline,
};
use stem_core::history::{Action, UndoRedoStack};
use stem_core::peaks::PeakPyramid;
use stem_core::project::{Clip, Project, Track};
use stem_i18n::{I18n, Locale, TimecodeMode};

use crate::state::{TimelineViewState, WorkerMessage};
use crate::views::{
    render_dialogs, render_effects_dialog, render_menu_bar, render_mixer_panel,
    render_spectrogram_panel, render_timeline_view, render_transport_bar, EffectsRackState,
    SpectrogramState,
};

pub struct StemApp {
    pub project: Project,
    pub project_file_path: Option<PathBuf>,
    pub i18n: I18n,
    pub history: UndoRedoStack,
    pub timeline_state: TimelineViewState,

    // Audio Engine & Buffers
    pub audio_engine: Option<AudioEngine>,
    pub audio_recorder: Option<AudioRecorder>,
    pub telemetry: AudioTelemetry,
    pub is_playing: bool,
    pub is_recording: bool,
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
    pub show_effects_dialog: bool,

    // Export State
    pub export_destination_path: Option<String>,
    pub export_settings: ExportSettings,
    pub export_progress: Option<f32>,
    pub export_cancel_token: Option<Arc<std::sync::atomic::AtomicBool>>,

    // Effects & Analysis
    pub effects_state: EffectsRackState,
    pub spectrogram_state: SpectrogramState,

    // Status
    pub status_text: String,
}

impl StemApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let i18n = I18n::new(Locale::from_system());
        let (worker_tx, worker_rx) = unbounded();

        let audio_engine = match AudioEngine::new() {
            Ok(engine) => Some(engine),
            Err(err) => {
                eprintln!("steM. audio engine init warning: {err}");
                None
            }
        };

        let audio_recorder = match AudioRecorder::new() {
            Ok(rec) => Some(rec),
            Err(err) => {
                eprintln!("steM. audio recorder init warning: {err}");
                None
            }
        };

        let project = Project::new("Untitled Project", 48000);
        let status_text = i18n.t("status-ready");

        Self {
            project,
            project_file_path: None,
            i18n,
            history: UndoRedoStack::new(),
            timeline_state: TimelineViewState::default(),

            audio_engine,
            audio_recorder,
            telemetry: AudioTelemetry::default(),
            is_playing: false,
            is_recording: false,
            source_buffers: HashMap::new(),
            peak_pyramids: HashMap::new(),
            active_timeline: None,

            timecode_mode: TimecodeMode::default(),

            worker_tx,
            worker_rx,

            show_mixer_panel: true,
            show_spectrogram_panel: false,
            show_history_panel: false,
            show_preferences_dialog: false,
            show_export_dialog: false,
            show_about_dialog: false,
            show_effects_dialog: false,

            export_destination_path: None,
            export_settings: ExportSettings::default(),
            export_progress: None,
            export_cancel_token: None,

            effects_state: EffectsRackState::default(),
            spectrogram_state: SpectrogramState::default(),
            status_text,
        }
    }

    /// Re-renders project mixdown into a SharedAudioTimeline buffer and sends it to the real-time engine.
    pub fn sync_timeline_to_audio_engine(&mut self) {
        if let Ok(mix) = render_project_mixdown(&self.project, &self.source_buffers, None) {
            let total_frames = self.project.total_frames();
            let timeline = Arc::new(SharedAudioTimeline {
                channels: mix,
                total_frames,
            });
            self.active_timeline = Some(timeline.clone());
            if let Some(ref mut engine) = self.audio_engine {
                engine.update_timeline(timeline);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Project Actions
    // -------------------------------------------------------------------------

    pub fn action_new_project(&mut self) {
        self.action_stop();
        self.project = Project::new("Untitled Project", 48000);
        self.project_file_path = None;
        self.source_buffers.clear();
        self.peak_pyramids.clear();
        self.history = UndoRedoStack::new();
        self.active_timeline = None;
        self.status_text = self.i18n.t("status-ready");
    }

    pub fn action_load_demo(&mut self) {
        self.action_stop();
        let (demo_proj, demo_buffers) = stem_core::generate_demo_project(48000);
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

    pub fn action_open_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("steM Project", &["ron", "stem"])
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

                        // Reload source files
                        for source in self.project.sources.values() {
                            let tx = self.worker_tx.clone();
                            let path = source.path.clone();
                            std::thread::spawn(move || {
                                if let Ok(decoded) = stem_audio::decode_file(&path) {
                                    let channel_refs: Vec<&[f32]> =
                                        decoded.channels.iter().map(|ch| ch.as_slice()).collect();
                                    let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
                                    let _ = tx.send(WorkerMessage::ImportComplete {
                                        decoded: Box::new(decoded),
                                        pyramid: Box::new(pyramid),
                                    });
                                }
                            });
                        }

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
                Ok(ron_str) => {
                    let _ = std::fs::write(path, ron_str);
                    self.status_text = self.i18n.t("status-project-saved");
                }
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
            .add_filter("steM Project", &["stem", "ron"])
            .save_file()
        {
            match self.project.to_ron() {
                Ok(ron_str) => {
                    let _ = std::fs::write(&path, ron_str);
                    self.project_file_path = Some(path);
                    self.status_text = self.i18n.t("status-project-saved");
                }
                Err(e) => {
                    self.status_text = format!("Save error: {e}");
                }
            }
        }
    }

    pub fn action_import_audio(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["wav", "flac", "mp3", "ogg", "aac", "m4a"])
            .pick_file()
        {
            self.status_text = self.i18n.t("status-loading");
            let tx = self.worker_tx.clone();

            std::thread::spawn(move || match stem_audio::decode_file(&path) {
                Ok(decoded) => {
                    let channel_refs: Vec<&[f32]> =
                        decoded.channels.iter().map(|ch| ch.as_slice()).collect();
                    let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);
                    let _ = tx.send(WorkerMessage::ImportComplete {
                        decoded: Box::new(decoded),
                        pyramid: Box::new(pyramid),
                    });
                }
                Err(e) => {
                    let _ = tx.send(WorkerMessage::ImportFailed(e.to_string()));
                }
            });
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

    pub fn action_toggle_play(&mut self) {
        if let Some(ref mut engine) = self.audio_engine {
            engine.toggle_play();
            self.is_playing = engine.is_playing();
            self.status_text = if self.is_playing {
                self.i18n.t("status-playing")
            } else {
                self.i18n.t("status-paused")
            };
        }
    }

    pub fn action_stop(&mut self) {
        if self.is_recording {
            self.action_toggle_record();
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.stop();
            self.is_playing = false;
            self.telemetry.playhead_frame = 0;
            self.status_text = self.i18n.t("status-ready");
        }
    }

    pub fn action_toggle_record(&mut self) {
        if self.is_recording {
            // Stop recording take
            if let Some(ref recorder) = self.audio_recorder {
                if let Some((source, planar)) = recorder.stop_recording() {
                    let source_id = source.id;
                    let source_frames = source.duration_frames;
                    let filename = source.filename.clone();

                    let channel_refs: Vec<&[f32]> = planar.iter().map(|ch| ch.as_slice()).collect();
                    let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);

                    self.project.add_source(source);
                    self.source_buffers.insert(source_id, planar);
                    self.peak_pyramids.insert(source_id, pyramid);

                    // Place recorded clip on armed track or first track
                    let target_track_id = self
                        .project
                        .tracks
                        .iter()
                        .find(|t| t.armed)
                        .map(|t| t.id)
                        .or_else(|| self.project.tracks.first().map(|t| t.id));

                    if let Some(t_id) = target_track_id {
                        let playhead = self.telemetry.playhead_frame;
                        let clip = Clip::new(&filename, source_id, 0, source_frames, playhead);
                        let _ = self.project.add_clip_to_track(t_id, clip);
                    }

                    self.status_text = self.i18n.t("status-recording-saved");
                    self.sync_timeline_to_audio_engine();
                }
            }
            self.is_recording = false;
        } else {
            // Start recording take
            if let Some(ref recorder) = self.audio_recorder {
                recorder.start_recording();
                self.is_recording = true;
                self.status_text = self.i18n.t("status-recording");
                if !self.is_playing {
                    self.action_toggle_play();
                }
            } else {
                self.status_text = "No audio input device available to record".to_string();
            }
        }
    }

    pub fn action_seek(&mut self, frame: u64) {
        if let Some(ref mut engine) = self.audio_engine {
            engine.seek(frame);
        }
        self.telemetry.playhead_frame = frame;
    }

    pub fn action_set_loop(&mut self, enabled: bool) {
        self.project.loop_enabled = enabled;
        let total = self.project.total_frames();
        let (start, end) = self.project.loop_region.unwrap_or((0, total));
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_loop(enabled, start, end);
        }
    }

    pub fn action_set_master_gain(&mut self, gain_db: f32) {
        self.project.master_gain_db = gain_db;
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_master_gain_db(gain_db);
        }
    }

    pub fn action_set_track_gain(&mut self, track_idx: usize, gain_db: f32) {
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.gain_db = gain_db;
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_track_gain_db(track_idx, gain_db);
        }
    }

    pub fn action_set_track_pan(&mut self, track_idx: usize, pan: f32) {
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.pan = pan;
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_track_pan(track_idx, pan);
        }
    }

    pub fn action_set_track_mute(&mut self, track_idx: usize, muted: bool) {
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.muted = muted;
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_track_mute(track_idx, muted);
        }
    }

    pub fn action_set_track_solo(&mut self, track_idx: usize, soloed: bool) {
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.soloed = soloed;
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_track_solo(track_idx, soloed);
        }
    }

    // -------------------------------------------------------------------------
    // Editing Actions (Undo / Redo)
    // -------------------------------------------------------------------------

    pub fn action_undo(&mut self) {
        let _ = self.history.undo(&mut self.project);
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_redo(&mut self) {
        let _ = self.history.redo(&mut self.project);
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_add_track(&mut self, name: String) {
        let track = Track::new(name);
        let _ = self
            .history
            .push_and_execute(Action::AddTrack(track), &mut self.project);
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_delete_track(&mut self, track_id: Uuid) {
        if let Some(idx) = self.project.tracks.iter().position(|t| t.id == track_id) {
            let track = self.project.tracks[idx].clone();
            let _ = self
                .history
                .push_and_execute(Action::RemoveTrack { track, index: idx }, &mut self.project);
            self.timeline_state.selected_track_id = None;
            self.sync_timeline_to_audio_engine();
        }
    }

    pub fn action_split_selected_clip(&mut self) {
        let playhead = self.telemetry.playhead_frame;
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &self.project.tracks {
                if track.clips.iter().any(|c| c.id == clip_id) {
                    let track_id = track.id;
                    let _ = self.project.split_clip(track_id, clip_id, playhead);
                    self.sync_timeline_to_audio_engine();
                    break;
                }
            }
        }
    }

    pub fn action_delete_selected_clip(&mut self) {
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &self.project.tracks {
                if let Some(c_idx) = track.clips.iter().position(|c| c.id == clip_id) {
                    let clip = track.clips[c_idx].clone();
                    let track_id = track.id;
                    let _ = self.history.push_and_execute(
                        Action::RemoveClip {
                            track_id,
                            clip,
                            index: c_idx,
                        },
                        &mut self.project,
                    );
                    self.timeline_state.selected_clip_id = None;
                    self.sync_timeline_to_audio_engine();
                    break;
                }
            }
        }
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let input = ctx.input(|i| i.clone());

        // Space: Toggle Play / Pause
        if input.key_pressed(egui::Key::Space) {
            self.action_toggle_play();
        }

        // S: Split selected clip at playhead
        if input.key_pressed(egui::Key::S) {
            self.action_split_selected_clip();
        }

        // Delete / Backspace: Delete selected clip
        if input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace) {
            self.action_delete_selected_clip();
        }

        // Home: Seek to frame 0
        if input.key_pressed(egui::Key::Home) {
            self.action_seek(0);
        }

        // Ctrl+Z: Undo
        if input.modifiers.command && !input.modifiers.shift && input.key_pressed(egui::Key::Z) {
            self.action_undo();
        }

        // Ctrl+Y or Ctrl+Shift+Z: Redo
        if (input.modifiers.command && input.key_pressed(egui::Key::Y))
            || (input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::Z))
        {
            self.action_redo();
        }
    }
}

impl eframe::App for StemApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Continuous repaint during playback or recording for 60fps animations
        if self.is_playing || self.is_recording {
            ctx.request_repaint();
        }

        // Handle global keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Drain telemetry from real-time audio thread
        if let Some(ref mut engine) = self.audio_engine {
            self.telemetry = engine.poll_telemetry();
            self.is_playing = engine.is_playing();
        }

        // Drain messages from background workers (decoding, peak caching, export)
        while let Ok(msg) = self.worker_rx.try_recv() {
            match msg {
                WorkerMessage::ImportComplete { decoded, pyramid } => {
                    let source = decoded.source.clone();
                    let source_id = source.id;
                    let source_frames = source.duration_frames;
                    let filename = source.filename.clone();

                    self.project.add_source(source);
                    self.source_buffers.insert(source_id, decoded.channels);
                    self.peak_pyramids.insert(source_id, *pyramid);

                    // Add a default clip on the first available track
                    if !self.project.tracks.is_empty() {
                        let track_id = self.project.tracks[0].id;
                        let clip = Clip::new(&filename, source_id, 0, source_frames, 0);
                        let _ = self.project.add_clip_to_track(track_id, clip);
                    }

                    self.status_text =
                        self.i18n
                            .t_arg("status-imported-file", "filename", filename);
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

        // ----------------- Modal Dialogs -----------------
        render_dialogs(self, ctx);
        render_effects_dialog(self, ctx);
    }
}
