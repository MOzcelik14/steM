//! Central application state and eframe::App event loop implementation

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use stem_audio::{
    export_project_to_file, render_project_mixdown, render_project_tracks, AudioEngine,
    AudioRecorder, AudioTelemetry, ExportSettings, SharedAudioTimeline,
};
use stem_core::history::{Action, UndoRedoStack};
use stem_core::peaks::PeakPyramid;
use stem_core::project::{Clip, Project, Track};
use stem_core::waveform::{AudioClipboard, WaveformDocument};
use stem_i18n::{I18n, Locale, TimecodeMode};

use crate::state::{
    AppViewMode, ClickRemoverDialogState, LoudnessReportDialogState, NoiseReductionDialogState,
    RecordingState, TimelineViewState, WaveformViewState, WorkerMessage,
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
    pub show_effects_dialog: bool,

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
        let selected_sr = audio_recorder
            .as_ref()
            .map(|r| r.sample_rate())
            .unwrap_or(48000);

        let recording_state = RecordingState {
            available_input_devices: available_inputs,
            selected_input_device: selected_input,
            selected_sample_rate: selected_sr,
            ..Default::default()
        };

        let project = Project::new("Untitled Project", 48000);
        let status_text = i18n.t("status-ready");

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
            noise_reduction_state: NoiseReductionDialogState::default(),
            click_remover_state: ClickRemoverDialogState::default(),
            loudness_report_state: LoudnessReportDialogState::default(),
            status_text,
        }
    }

    /// Re-renders project mixdown into a SharedAudioTimeline buffer and sends it to the real-time engine.
    pub fn sync_timeline_to_audio_engine(&mut self) {
        if let Ok(mix) = render_project_mixdown(&self.project, &self.source_buffers, None) {
            let total_frames = self.project.total_frames();
            let track_channels = render_project_tracks(&self.project, &self.source_buffers);
            let timeline = Arc::new(SharedAudioTimeline {
                channels: mix,
                track_channels,
                total_frames,
            });
            self.active_timeline = Some(timeline.clone());
            if let Some(ref mut engine) = self.audio_engine {
                engine.update_timeline(timeline);
            }
        }
    }

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

    pub fn action_open_file_in_waveform_editor(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio Files", &["wav", "flac", "mp3", "ogg", "aac", "m4a"])
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
        if let Some(profile) = self.noise_reduction_state.noise_profile.clone() {
            let red_db = self.noise_reduction_state.reduction_db;
            let sens = self.noise_reduction_state.sensitivity;
            let gate = stem_dsp::SpectralGate::new(profile, red_db, sens);

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
                    self.recompute_waveform_cache();
                }
            }
        }
    }

    pub fn action_apply_click_remover(&mut self) {
        let sens = self.click_remover_state.sensitivity;
        let max_w = self.click_remover_state.max_width;
        let declicker = stem_dsp::ClickRemover::new(sens, max_w);

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
                self.recompute_waveform_cache();
            }
        }
    }

    pub fn action_open_loudness_report(&mut self) {
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

        if !interleaved.is_empty() {
            match stem_dsp::analyze_loudness(&interleaved, channels, sample_rate) {
                Ok(res) => {
                    let tp = stem_dsp::compute_true_peak_dbfs(&interleaved);
                    self.loudness_report_state.result = Some(res);
                    self.loudness_report_state.true_peak_dbfs = tp;
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
        if self.view_mode == AppViewMode::Waveform {
            if self.waveform_state.document.is_none() {
                return;
            }
            if !self.is_playing {
                self.sync_waveform_to_audio_engine();
                if let Some((start, end)) = self.waveform_state.selection {
                    let cur = self.waveform_state.playhead_frame;
                    if cur < start || cur >= end {
                        self.action_seek_waveform(start);
                    }
                }
            }
        }
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
            if self.view_mode == AppViewMode::Waveform {
                self.waveform_state.playhead_frame = 0;
            }
            self.status_text = self.i18n.t("status-ready");
        }
    }

    pub fn action_toggle_record(&mut self) {
        if self.is_recording || self.recording_state.count_in_active {
            // Stop recording / cancel count-in
            if self.recording_state.count_in_active {
                self.recording_state.count_in_active = false;
                self.is_recording = false;
                self.status_text = self.i18n.t("status-ready");
                return;
            }

            self.finish_recording_take();
            self.is_recording = false;
            self.recording_state.is_punching = false;
        } else {
            // Start recording flow
            if self.audio_recorder.is_none() {
                self.status_text = self.i18n.t("recording-no-device");
                return;
            }

            // Check count-in
            let count_in_dur = self
                .project
                .count_in_mode
                .duration_seconds(self.project.tempo_bpm, self.project.time_signature.0);

            if count_in_dur > 0.0 {
                self.recording_state.count_in_active = true;
                self.recording_state.count_in_seconds_remaining = count_in_dur;
                self.recording_state.count_in_total_seconds = count_in_dur;
                self.status_text = self.i18n.t("recording-count-in-active");
                if !self.is_playing {
                    self.action_toggle_play();
                }
            } else {
                self.start_actual_recording();
            }
        }
    }

    /// Engages audio recording from the hardware stream.
    pub fn start_actual_recording(&mut self) {
        if self.audio_recorder.is_none() {
            return;
        }

        let playhead = self.telemetry.playhead_frame;
        self.recording_state.record_start_frame = playhead;

        if self.project.punch_in_enabled {
            if let Some((punch_in, punch_out)) = self.project.punch_region {
                if playhead < punch_in {
                    // Playhead is in preroll before punch-in point
                    self.is_recording = true;
                    self.recording_state.is_punching = false;
                    self.status_text = self.i18n.t("recording-punch-preroll");
                    if !self.is_playing {
                        self.action_toggle_play();
                    }
                    return;
                } else if playhead >= punch_out {
                    // Past punch-out: reposition playhead at punch-in
                    self.action_seek(punch_in);
                    self.recording_state.record_start_frame = punch_in;
                }
            }
        }

        if let Some(ref recorder) = self.audio_recorder {
            recorder.start_recording();
        }
        self.is_recording = true;
        self.recording_state.is_punching = self.project.punch_in_enabled;
        self.status_text = self.i18n.t("status-recording");
        if !self.is_playing {
            self.action_toggle_play();
        }
    }

    /// Stops hardware stream capture, applies latency compensation, routes channels, and places take clips.
    pub fn finish_recording_take(&mut self) {
        if let Some(ref recorder) = self.audio_recorder {
            if let Some((source, planar)) = recorder.stop_recording() {
                let source_frames = source.duration_frames;
                if source_frames == 0 {
                    return;
                }

                let latency_frames = self.project.latency_compensation_frames();
                let start_frame = self.recording_state.record_start_frame;

                // Determine armed tracks (or default to first track)
                let armed_tracks: Vec<Uuid> = self
                    .project
                    .tracks
                    .iter()
                    .filter(|t| t.armed)
                    .map(|t| t.id)
                    .collect();

                let target_tracks = if !armed_tracks.is_empty() {
                    armed_tracks
                } else if let Some(first) = self.project.tracks.first() {
                    vec![first.id]
                } else {
                    Vec::new()
                };

                for &track_id in &target_tracks {
                    let routing = self
                        .project
                        .get_track(track_id)
                        .map(|t| t.input_routing)
                        .unwrap_or_default();

                    let routed_planar = routing.extract_channels(&planar);
                    let track_channels = routed_planar.len();
                    let channel_refs: Vec<&[f32]> =
                        routed_planar.iter().map(|ch| ch.as_slice()).collect();
                    let pyramid = stem_dsp::build_peak_pyramid(&channel_refs);

                    let take_source_id = Uuid::new_v4();
                    let take_filename = format!(
                        "Take_{}_{}.wav",
                        self.project
                            .get_track(track_id)
                            .map(|t| t.name.as_str())
                            .unwrap_or("Track"),
                        &take_source_id.to_string()[..6]
                    );

                    let take_source = stem_core::project::AudioSource {
                        id: take_source_id,
                        path: std::path::PathBuf::from(&take_filename),
                        filename: take_filename.clone(),
                        sample_rate: self.project.sample_rate,
                        channels: track_channels.max(1),
                        duration_frames: source_frames,
                        file_hash: None,
                    };

                    self.project.add_source(take_source);
                    self.source_buffers.insert(take_source_id, routed_planar);
                    self.peak_pyramids.insert(take_source_id, pyramid);

                    // Latency compensation calculation
                    let (adjusted_offset, source_start, clip_len) = if start_frame >= latency_frames
                    {
                        (start_frame - latency_frames, 0, source_frames)
                    } else {
                        let excess = latency_frames - start_frame;
                        let excess_clamped = excess.min(source_frames);
                        (0, excess_clamped, source_frames - excess_clamped)
                    };

                    let clip = Clip::new(
                        &take_filename,
                        take_source_id,
                        source_start,
                        clip_len,
                        adjusted_offset,
                    );

                    if self.project.punch_in_enabled {
                        let _ = self.project.punch_clip_into_track(track_id, clip);
                    } else {
                        let _ = self.project.add_clip_to_track(track_id, clip);
                    }
                }

                self.status_text = self.i18n.t("status-recording-saved");
                self.sync_timeline_to_audio_engine();
            }
        }
    }

    /// Toggle software input monitoring on or off.
    pub fn action_toggle_input_monitoring(&mut self) {
        self.project.input_monitoring = !self.project.input_monitoring;
        let enabled = self.project.input_monitoring;
        if let Some(ref recorder) = self.audio_recorder {
            recorder.set_monitoring(enabled);
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_input_monitoring(enabled);
        }
        self.status_text = if enabled {
            self.i18n.t("recording-monitoring-enabled")
        } else {
            self.i18n.t("recording-monitoring-disabled")
        };
    }

    /// Select hardware input device.
    pub fn action_set_input_device(&mut self, device_name: Option<String>) {
        self.recording_state.selected_input_device = device_name.clone();
        let mon_tx = self
            .audio_engine
            .as_mut()
            .and_then(|e| e.take_monitor_producer());
        match AudioRecorder::with_config(
            device_name.as_deref(),
            Some(self.recording_state.selected_sample_rate),
            mon_tx,
        ) {
            Ok(rec) => {
                if self.project.input_monitoring {
                    rec.set_monitoring(true);
                }
                self.audio_recorder = Some(rec);
                self.recording_state.device_lost_warning = false;
                self.status_text = self.i18n.t("recording-device-connected");
            }
            Err(err) => {
                eprintln!("steM. error setting input device: {err}");
                self.status_text = format!("Input device error: {err}");
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
        let (group_id, old_gain_db) = if let Some(track) = self.project.tracks.get(track_idx) {
            (track.group_id, track.gain_db)
        } else {
            return;
        };

        let link_gain = group_id
            .and_then(|gid| self.project.get_group(gid))
            .map(|g| g.link_gain)
            .unwrap_or(false);

        if link_gain {
            let delta_db = gain_db - old_gain_db;
            let peer_indices: Vec<usize> = self
                .project
                .tracks
                .iter()
                .enumerate()
                .filter(|(idx, t)| t.group_id == group_id && *idx != track_idx)
                .map(|(idx, _)| idx)
                .collect();

            if let Some(t) = self.project.tracks.get_mut(track_idx) {
                t.gain_db = gain_db.clamp(-60.0, 12.0);
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_gain_db(track_idx, gain_db);
            }

            for p_idx in peer_indices {
                if let Some(peer) = self.project.tracks.get_mut(p_idx) {
                    peer.gain_db = (peer.gain_db + delta_db).clamp(-60.0, 12.0);
                    let new_p_gain = peer.gain_db;
                    if let Some(ref mut engine) = self.audio_engine {
                        engine.set_track_gain_db(p_idx, new_p_gain);
                    }
                }
            }
        } else {
            if let Some(track) = self.project.tracks.get_mut(track_idx) {
                track.gain_db = gain_db.clamp(-60.0, 12.0);
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_gain_db(track_idx, gain_db);
            }
        }
    }

    pub fn action_set_track_pan(&mut self, track_idx: usize, pan: f32) {
        let (group_id, old_pan) = if let Some(track) = self.project.tracks.get(track_idx) {
            (track.group_id, track.pan)
        } else {
            return;
        };

        let link_pan = group_id
            .and_then(|gid| self.project.get_group(gid))
            .map(|g| g.link_pan)
            .unwrap_or(false);

        if link_pan {
            let delta_pan = pan - old_pan;
            let peer_indices: Vec<usize> = self
                .project
                .tracks
                .iter()
                .enumerate()
                .filter(|(idx, t)| t.group_id == group_id && *idx != track_idx)
                .map(|(idx, _)| idx)
                .collect();

            if let Some(t) = self.project.tracks.get_mut(track_idx) {
                t.pan = pan.clamp(-1.0, 1.0);
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_pan(track_idx, pan);
            }

            for p_idx in peer_indices {
                if let Some(peer) = self.project.tracks.get_mut(p_idx) {
                    peer.pan = (peer.pan + delta_pan).clamp(-1.0, 1.0);
                    let new_p_pan = peer.pan;
                    if let Some(ref mut engine) = self.audio_engine {
                        engine.set_track_pan(p_idx, new_p_pan);
                    }
                }
            }
        } else {
            if let Some(track) = self.project.tracks.get_mut(track_idx) {
                track.pan = pan.clamp(-1.0, 1.0);
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_pan(track_idx, pan);
            }
        }
    }

    pub fn action_set_track_mute(&mut self, track_idx: usize, muted: bool) {
        let group_id = self.project.tracks.get(track_idx).and_then(|t| t.group_id);
        let link_mute = group_id
            .and_then(|gid| self.project.get_group(gid))
            .map(|g| g.link_mute)
            .unwrap_or(false);

        if link_mute {
            let group_indices: Vec<usize> = self
                .project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id == group_id)
                .map(|(idx, _)| idx)
                .collect();

            for idx in group_indices {
                if let Some(t) = self.project.tracks.get_mut(idx) {
                    t.muted = muted;
                }
                if let Some(ref mut engine) = self.audio_engine {
                    engine.set_track_mute(idx, muted);
                }
            }
        } else {
            if let Some(track) = self.project.tracks.get_mut(track_idx) {
                track.muted = muted;
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_mute(track_idx, muted);
            }
        }
    }

    pub fn action_set_track_solo(&mut self, track_idx: usize, soloed: bool) {
        let group_id = self.project.tracks.get(track_idx).and_then(|t| t.group_id);
        let link_solo = group_id
            .and_then(|gid| self.project.get_group(gid))
            .map(|g| g.link_solo)
            .unwrap_or(false);

        if link_solo {
            let group_indices: Vec<usize> = self
                .project
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.group_id == group_id)
                .map(|(idx, _)| idx)
                .collect();

            for idx in group_indices {
                if let Some(t) = self.project.tracks.get_mut(idx) {
                    t.soloed = soloed;
                }
                if let Some(ref mut engine) = self.audio_engine {
                    engine.set_track_solo(idx, soloed);
                }
            }
        } else {
            if let Some(track) = self.project.tracks.get_mut(track_idx) {
                track.soloed = soloed;
            }
            if let Some(ref mut engine) = self.audio_engine {
                engine.set_track_solo(track_idx, soloed);
            }
        }
    }

    pub fn action_set_track_arm(&mut self, track_idx: usize, armed: bool) {
        let group_id = self.project.tracks.get(track_idx).and_then(|t| t.group_id);
        let link_arm = group_id
            .and_then(|gid| self.project.get_group(gid))
            .map(|g| g.link_arm)
            .unwrap_or(false);

        if link_arm {
            for t in &mut self.project.tracks {
                if t.group_id == group_id {
                    t.armed = armed;
                }
            }
        } else if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.armed = armed;
        }
    }

    pub fn action_add_bus(&mut self, name: impl Into<String>) -> Uuid {
        let id = self.project.add_bus(name);
        self.sync_timeline_to_audio_engine();
        id
    }

    pub fn action_delete_bus(&mut self, bus_id: Uuid) {
        self.project.remove_bus(bus_id);
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_set_bus_gain(&mut self, bus_id: Uuid, gain_db: f32) {
        if let Some(bus) = self.project.get_bus_mut(bus_id) {
            bus.gain_db = gain_db.clamp(-60.0, 12.0);
        }
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_set_bus_pan(&mut self, bus_id: Uuid, pan: f32) {
        if let Some(bus) = self.project.get_bus_mut(bus_id) {
            bus.pan = pan.clamp(-1.0, 1.0);
        }
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_set_bus_mute(&mut self, bus_id: Uuid, muted: bool) {
        if let Some(bus) = self.project.get_bus_mut(bus_id) {
            bus.muted = muted;
        }
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_set_bus_solo(&mut self, bus_id: Uuid, soloed: bool) {
        if let Some(bus) = self.project.get_bus_mut(bus_id) {
            bus.soloed = soloed;
        }
        self.sync_timeline_to_audio_engine();
    }

    pub fn action_add_group(&mut self, name: impl Into<String>) -> Uuid {
        self.project.add_group(name)
    }

    #[allow(dead_code)]
    pub fn action_delete_group(&mut self, group_id: Uuid) {
        self.project.remove_group(group_id);
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

        // View switching: 1 for Multitrack, 2 for Waveform
        if input.key_pressed(egui::Key::Num1) {
            self.action_switch_to_multitrack();
        }
        if input.key_pressed(egui::Key::Num2) {
            self.action_switch_to_waveform();
        }

        if self.view_mode == AppViewMode::Waveform {
            if input.modifiers.command && input.key_pressed(egui::Key::X) {
                self.action_waveform_cut();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::C) {
                self.action_waveform_copy();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::V) {
                self.action_waveform_paste();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::T) {
                self.action_waveform_trim();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::I) {
                self.action_waveform_insert_silence();
            }
            if input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace) {
                self.action_waveform_delete();
            }
            if input.modifiers.command && !input.modifiers.shift && input.key_pressed(egui::Key::Z)
            {
                self.action_waveform_undo();
            }
            if (input.modifiers.command && input.key_pressed(egui::Key::Y))
                || (input.modifiers.command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::Z))
            {
                self.action_waveform_redo();
            }
        } else {
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
            if input.modifiers.command && !input.modifiers.shift && input.key_pressed(egui::Key::Z)
            {
                self.action_undo();
            }

            // Ctrl+Y or Ctrl+Shift+Z: Redo
            if (input.modifiers.command && input.key_pressed(egui::Key::Y))
                || (input.modifiers.command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::Z))
            {
                self.action_redo();
            }
        }
    }
}

impl eframe::App for StemApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        // Check for audio hardware disconnection
        let mut lost_device = false;
        if let Some(ref recorder) = self.audio_recorder {
            if recorder.is_device_lost() {
                lost_device = true;
            }
        }
        if lost_device {
            if self.is_recording {
                self.action_toggle_record();
            }
            self.recording_state.device_lost_warning = true;
            self.audio_recorder = None;
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
            self.waveform_state.playhead_frame = self.telemetry.playhead_frame;
            if !self.waveform_state.loop_selection {
                if let Some((_, end)) = self.waveform_state.selection {
                    if self.telemetry.playhead_frame >= end {
                        self.action_toggle_play();
                        self.action_seek_waveform(end);
                    }
                }
            }
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
