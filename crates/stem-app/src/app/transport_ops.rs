use super::StemApp;
use uuid::Uuid;

use stem_audio::{AudioEngine, AudioRecorder};
use stem_core::history::Action;
use stem_core::project::{Clip, Track};

use crate::state::AppViewMode;

impl StemApp {
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
        } else if !self.is_playing {
            if let Some((start, end)) = self.timeline_state.time_selection {
                let cur = self.telemetry.playhead_frame;
                if cur < start || cur >= end {
                    self.action_seek(start);
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

    /// Re-initializes output engine and input recorder with current device, sample rate, and buffer size preferences.
    pub fn action_reconfigure_audio_devices(&mut self) {
        self.action_stop();
        self.recording_state.available_output_devices = AudioEngine::available_output_devices();
        self.recording_state.available_input_devices = AudioRecorder::available_input_devices();

        let cfg = stem_audio::AudioEngineConfig {
            preferred_device: self.recording_state.selected_output_device.clone(),
            preferred_sample_rate: Some(self.project.sample_rate),
            buffer_size_frames: self.recording_state.buffer_size_frames,
        };
        match AudioEngine::with_config(cfg) {
            Ok(mut engine) => {
                let sr = engine.sample_rate();
                self.recording_state.selected_output_device =
                    Some(engine.device_name().to_string());
                self.recording_state.selected_sample_rate = sr;
                let mon_tx = engine.take_monitor_producer();
                self.audio_engine = Some(engine);

                if let Ok(rec) = AudioRecorder::with_full_config(
                    self.recording_state.selected_input_device.as_deref(),
                    Some(sr),
                    self.recording_state.buffer_size_frames,
                    mon_tx,
                ) {
                    if self.project.input_monitoring {
                        rec.set_monitoring(true);
                    }
                    self.audio_recorder = Some(rec);
                }

                self.recording_state.device_lost_warning = false;
                self.sync_timeline_to_audio_engine();
                self.action_set_loop(self.project.loop_enabled);
                self.action_set_metronome(self.metronome_enabled);
                self.status_text = self.i18n.t("recording-device-connected");
            }
            Err(err) => {
                self.status_text = format!("Audio device error: {err}");
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
        if self.project.tracks.len() >= stem_core::MAX_PROJECT_TRACKS {
            self.status_text = format!(
                "Track limit reached (max {})",
                stem_core::MAX_PROJECT_TRACKS
            );
            return;
        }
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

    pub fn has_unsaved_changes(&self) -> bool {
        self.history.can_undo() || self.waveform_state.history.can_undo()
    }

    pub fn action_split_selected_clip(&mut self) {
        let playhead = self.telemetry.playhead_frame;
        let mut target: Option<(Uuid, Uuid)> = None;

        // 1. Check selected clip first
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &self.project.tracks {
                if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
                    if playhead > clip.track_offset_frames && playhead < clip.timeline_end_frame() {
                        target = Some((track.id, clip.id));
                        break;
                    }
                }
            }
        }

        // 2. Fallback: clip under playhead on selected track
        if target.is_none() {
            if let Some(track_id) = self.timeline_state.selected_track_id {
                if let Some(track) = self.project.get_track(track_id) {
                    if let Some(clip) = track.clips.iter().find(|c| {
                        playhead > c.track_offset_frames && playhead < c.timeline_end_frame()
                    }) {
                        target = Some((track.id, clip.id));
                    }
                }
            }
        }

        // 3. Fallback: first clip under playhead on any track
        if target.is_none() {
            for track in &self.project.tracks {
                if let Some(clip) = track
                    .clips
                    .iter()
                    .find(|c| playhead > c.track_offset_frames && playhead < c.timeline_end_frame())
                {
                    target = Some((track.id, clip.id));
                    break;
                }
            }
        }

        if let Some((track_id, clip_id)) = target {
            if let Ok((_first_id, second_id)) = self.project.split_clip(track_id, clip_id, playhead)
            {
                self.timeline_state.selected_clip_id = Some(second_id);
                self.sync_timeline_to_audio_engine();
                self.save_recovery_journal();
            }
        }
    }

    pub fn action_duplicate_selected_clip(&mut self) {
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &self.project.tracks {
                if let Some(orig) = track.clips.iter().find(|c| c.id == clip_id) {
                    let mut dup = orig.clone();
                    dup.id = Uuid::new_v4();
                    dup.track_offset_frames = orig.timeline_end_frame();
                    let new_id = dup.id;
                    let track_id = track.id;
                    let _ = self.history.push_and_execute(
                        Action::AddClip {
                            track_id,
                            clip: dup,
                        },
                        &mut self.project,
                    );
                    self.timeline_state.selected_clip_id = Some(new_id);
                    self.sync_timeline_to_audio_engine();
                    self.save_recovery_journal();
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
                    self.save_recovery_journal();
                    break;
                }
            }
        }
    }

    /// Deletes/silences the active timeline time selection if one exists,
    /// otherwise deletes the selected clip.
    pub fn action_delete_timeline_selection_or_clip(&mut self) {
        if self.timeline_state.time_selection.is_some() {
            self.action_silence_selection();
        } else {
            self.action_delete_selected_clip();
        }
    }

    pub fn action_copy_selected_clip(&mut self) {
        if let Some((sel_s, sel_e)) = self.timeline_state.time_selection {
            let (lo, hi) = (sel_s.min(sel_e), sel_s.max(sel_e));
            if lo < hi {
                let target_track = self.timeline_state.selected_track_id;
                for track in &self.project.tracks {
                    if target_track.is_some() && target_track != Some(track.id) {
                        continue;
                    }
                    for clip in &track.clips {
                        let c_start = clip.track_offset_frames;
                        let c_end = clip.timeline_end_frame();
                        if hi > c_start && lo < c_end {
                            let ov_s = lo.max(c_start);
                            let ov_e = hi.min(c_end);
                            let mut sliced = clip.clone();
                            sliced.id = uuid::Uuid::new_v4();
                            sliced.source_start_frame = clip.source_start_frame + (ov_s - c_start);
                            sliced.length_frames = ov_e - ov_s;
                            sliced.track_offset_frames = ov_s;
                            self.timeline_state.timeline_clip_clipboard = Some(sliced);
                            self.status_text = format!("Copied selection from: {}", clip.name);
                            return;
                        }
                    }
                }
            }
        }
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &self.project.tracks {
                if let Some(clip) = track.clips.iter().find(|c| c.id == clip_id) {
                    self.timeline_state.timeline_clip_clipboard = Some(clip.clone());
                    self.status_text = format!("Copied clip: {}", clip.name);
                    return;
                }
            }
        }
    }

    pub fn action_cut_selected_clip(&mut self) {
        self.action_copy_selected_clip();
        if self.timeline_state.timeline_clip_clipboard.is_some() {
            self.action_delete_timeline_selection_or_clip();
        }
    }

    pub fn action_paste_clip(&mut self) {
        let Some(src_clip) = self.timeline_state.timeline_clip_clipboard.clone() else {
            return;
        };
        let target_track_id = self
            .timeline_state
            .selected_track_id
            .or_else(|| self.project.tracks.first().map(|t| t.id));
        let Some(track_id) = target_track_id else {
            return;
        };
        let mut new_clip = src_clip.clone();
        let new_id = uuid::Uuid::new_v4();
        new_clip.id = new_id;
        let playhead = self.telemetry.playhead_frame;
        let overlaps_existing = self
            .project
            .get_track(track_id)
            .map(|t| {
                t.clips.iter().any(|c| {
                    let p_end = playhead + new_clip.length_frames;
                    p_end > c.track_offset_frames && playhead < c.timeline_end_frame()
                })
            })
            .unwrap_or(false);
        let paste_frame = if overlaps_existing {
            self.project
                .get_track(track_id)
                .map(|t| t.total_end_frame())
                .unwrap_or(playhead)
        } else {
            playhead
        };
        new_clip.track_offset_frames = paste_frame;
        let _ = self.history.push_and_execute(
            Action::AddClip {
                track_id,
                clip: new_clip,
            },
            &mut self.project,
        );
        self.timeline_state.time_selection = None;
        self.timeline_state.selected_clip_id = Some(new_id);
        self.action_seek(paste_frame);
        self.sync_timeline_to_audio_engine();
        self.save_recovery_journal();
        self.status_text = "Pasted clip".to_string();
    }

    pub fn action_fade_in_selected_clip(&mut self, duration_ms: f32) {
        let frames = ((duration_ms / 1000.0) * self.project.sample_rate as f32).round() as u64;
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &mut self.project.tracks {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    let max_fade = clip.length_frames.saturating_sub(clip.fade_out_frames);
                    clip.fade_in_frames = frames.min(max_fade);
                    self.sync_timeline_to_audio_engine();
                    self.save_recovery_journal();
                    self.status_text = format!("Fade In applied ({:.0} ms)", duration_ms);
                    return;
                }
            }
        } else if let Some((sel_s, sel_e)) = self.timeline_state.time_selection {
            let (lo, hi) = (sel_s.min(sel_e), sel_s.max(sel_e));
            let sel_len = hi.saturating_sub(lo);
            let mut changed = false;
            for track in &mut self.project.tracks {
                for clip in &mut track.clips {
                    if hi > clip.track_offset_frames && lo < clip.timeline_end_frame() {
                        let max_fade = clip.length_frames.saturating_sub(clip.fade_out_frames);
                        clip.fade_in_frames = sel_len.max(frames).min(max_fade);
                        changed = true;
                    }
                }
            }
            if changed {
                self.sync_timeline_to_audio_engine();
                self.save_recovery_journal();
                self.status_text = "Fade In applied to selection".to_string();
            }
        }
    }

    pub fn action_fade_out_selected_clip(&mut self, duration_ms: f32) {
        let frames = ((duration_ms / 1000.0) * self.project.sample_rate as f32).round() as u64;
        if let Some(clip_id) = self.timeline_state.selected_clip_id {
            for track in &mut self.project.tracks {
                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                    let max_fade = clip.length_frames.saturating_sub(clip.fade_in_frames);
                    clip.fade_out_frames = frames.min(max_fade);
                    self.sync_timeline_to_audio_engine();
                    self.save_recovery_journal();
                    self.status_text = format!("Fade Out applied ({:.0} ms)", duration_ms);
                    return;
                }
            }
        } else if let Some((sel_s, sel_e)) = self.timeline_state.time_selection {
            let (lo, hi) = (sel_s.min(sel_e), sel_s.max(sel_e));
            let sel_len = hi.saturating_sub(lo);
            let mut changed = false;
            for track in &mut self.project.tracks {
                for clip in &mut track.clips {
                    if hi > clip.track_offset_frames && lo < clip.timeline_end_frame() {
                        let max_fade = clip.length_frames.saturating_sub(clip.fade_in_frames);
                        clip.fade_out_frames = sel_len.max(frames).min(max_fade);
                        changed = true;
                    }
                }
            }
            if changed {
                self.sync_timeline_to_audio_engine();
                self.save_recovery_journal();
                self.status_text = "Fade Out applied to selection".to_string();
            }
        }
    }

    pub fn action_drop_marker(&mut self, name: Option<String>) {
        let frame = self.telemetry.playhead_frame;
        let count = self.project.markers.len() + 1;
        let marker_name = name.unwrap_or_else(|| format!("Marker {}", count));
        self.project.add_marker(frame, marker_name.clone());
        self.status_text = format!("Marker dropped: {}", marker_name);
    }

    pub fn action_jump_prev_marker(&mut self) {
        if let Some(frame) = self
            .project
            .find_prev_marker_frame(self.telemetry.playhead_frame)
        {
            self.action_seek(frame);
        }
    }

    pub fn action_jump_next_marker(&mut self) {
        if let Some(frame) = self
            .project
            .find_next_marker_frame(self.telemetry.playhead_frame)
        {
            self.action_seek(frame);
        }
    }
}
