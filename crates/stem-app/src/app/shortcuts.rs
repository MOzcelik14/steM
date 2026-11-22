use super::StemApp;
use eframe::egui;

use crate::state::{AppAction, AppViewMode};

impl StemApp {
    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let input = ctx.input(|i| i.clone());

        // File shortcuts (Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+E) & Loop (L)
        if input.modifiers.command && input.key_pressed(egui::Key::N) {
            self.action_new_project();
        }
        if input.modifiers.command && input.key_pressed(egui::Key::O) {
            self.action_open_project();
        }
        if input.modifiers.command && input.key_pressed(egui::Key::S) {
            self.action_save_project();
        }
        if input.modifiers.command && input.key_pressed(egui::Key::E) {
            self.show_export_dialog = true;
        }
        if !input.modifiers.any() && input.key_pressed(egui::Key::L) {
            self.action_set_loop(!self.project.loop_enabled);
        }

        // Play / Pause
        if self
            .keybindings
            .is_action_triggered(AppAction::PlayPause, &input)
        {
            self.action_toggle_play();
        }
        // Stop
        if self
            .keybindings
            .is_action_triggered(AppAction::Stop, &input)
        {
            self.action_stop();
        }
        // Record
        if self
            .keybindings
            .is_action_triggered(AppAction::Record, &input)
        {
            self.action_toggle_record();
        }
        // Drop Marker
        if self
            .keybindings
            .is_action_triggered(AppAction::DropMarker, &input)
        {
            self.action_drop_marker(None);
        }
        // Named Marker Dialog
        if self
            .keybindings
            .is_action_triggered(AppAction::NamedMarkerDialog, &input)
        {
            let frame = self.telemetry.playhead_frame;
            let count = self.project.markers.len() + 1;
            let default_name = format!("Marker {}", count);
            self.named_marker_state.open_for_new(frame, default_name);
        }
        // Prev Marker
        if self
            .keybindings
            .is_action_triggered(AppAction::PrevMarker, &input)
        {
            self.action_jump_prev_marker();
        }
        // Next Marker
        if self
            .keybindings
            .is_action_triggered(AppAction::NextMarker, &input)
        {
            self.action_jump_next_marker();
        }
        // Zoom In
        if self
            .keybindings
            .is_action_triggered(AppAction::ZoomIn, &input)
        {
            if self.view_mode == AppViewMode::Waveform {
                self.waveform_state.pixels_per_second =
                    (self.waveform_state.pixels_per_second * 1.25).clamp(5.0, 1000.0);
            } else {
                self.timeline_state.pixels_per_second =
                    (self.timeline_state.pixels_per_second * 1.25).clamp(5.0, 1000.0);
            }
        }
        // Zoom Out
        if self
            .keybindings
            .is_action_triggered(AppAction::ZoomOut, &input)
        {
            if self.view_mode == AppViewMode::Waveform {
                self.waveform_state.pixels_per_second =
                    (self.waveform_state.pixels_per_second / 1.25).clamp(5.0, 1000.0);
            } else {
                self.timeline_state.pixels_per_second =
                    (self.timeline_state.pixels_per_second / 1.25).clamp(5.0, 1000.0);
            }
        }
        // Toggle Spectrogram
        if self
            .keybindings
            .is_action_triggered(AppAction::ToggleSpectrogram, &input)
        {
            self.show_spectrogram_panel = !self.show_spectrogram_panel;
        }

        // View switching: 1 for Multitrack, 2 for Waveform (only without modifiers)
        if !input.modifiers.any() && input.key_pressed(egui::Key::Num1) {
            self.action_switch_to_multitrack();
        }
        if !input.modifiers.any() && input.key_pressed(egui::Key::Num2) {
            self.action_switch_to_waveform();
        }

        // Home: Seek to frame 0 (both views)
        if input.key_pressed(egui::Key::Home) {
            if self.view_mode == AppViewMode::Waveform {
                self.action_seek_waveform(0);
            } else {
                self.action_seek(0);
            }
        }

        // Escape: Deselect / clear selection, or stop playback/recording if nothing selected
        if input.key_pressed(egui::Key::Escape) {
            let had_selection = self.timeline_state.time_selection.is_some()
                || self.timeline_state.selected_clip_id.is_some()
                || !self.timeline_state.selected_clip_ids.is_empty()
                || self.waveform_state.selection.is_some();
            if had_selection {
                self.timeline_state.time_selection = None;
                self.timeline_state.selected_clip_id = None;
                self.timeline_state.selected_clip_ids.clear();
                self.waveform_state.selection = None;
            } else if self.is_playing || self.is_recording {
                self.action_stop();
            }
        }

        // Ctrl+A: Select All (both views)
        if input.modifiers.command && input.key_pressed(egui::Key::A) {
            if self.view_mode == AppViewMode::Waveform {
                if let Some(ref doc) = self.waveform_state.document {
                    let total = doc.total_frames();
                    if total > 0 {
                        self.waveform_state.selection = Some((0, total));
                    }
                }
            } else {
                let total = self.project.total_frames();
                if total > 0 {
                    self.timeline_state.time_selection = Some((0, total));
                }
            }
        }

        let has_cut_evt = input.events.iter().any(|e| matches!(e, egui::Event::Cut));
        let has_copy_evt = input.events.iter().any(|e| matches!(e, egui::Event::Copy));
        let has_paste_evt = input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Paste(_)));

        let cut_triggered =
            has_cut_evt || self.keybindings.is_action_triggered(AppAction::Cut, &input);
        let copy_triggered = has_copy_evt
            || self
                .keybindings
                .is_action_triggered(AppAction::Copy, &input);
        let paste_triggered = has_paste_evt
            || self
                .keybindings
                .is_action_triggered(AppAction::Paste, &input);

        if self.view_mode == AppViewMode::Waveform {
            if cut_triggered {
                self.action_waveform_cut();
            }
            if copy_triggered {
                self.action_waveform_copy();
            }
            if paste_triggered {
                self.action_waveform_paste();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::T) {
                self.action_waveform_trim();
            }
            if input.modifiers.command && input.key_pressed(egui::Key::I) {
                self.action_waveform_insert_silence();
            }
            if self
                .keybindings
                .is_action_triggered(AppAction::Delete, &input)
                || input.key_pressed(egui::Key::Backspace)
            {
                self.action_waveform_delete();
            }
            if self
                .keybindings
                .is_action_triggered(AppAction::Undo, &input)
            {
                self.action_waveform_undo();
            }
            if self
                .keybindings
                .is_action_triggered(AppAction::Redo, &input)
                || (input.modifiers.command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::Z))
            {
                self.action_waveform_redo();
            }
        } else {
            // Cut / Copy / Paste Clips (Ctrl+X / Ctrl+C / Ctrl+V)
            if cut_triggered {
                self.action_cut_selected_clip();
            }
            if copy_triggered {
                self.action_copy_selected_clip();
            }
            if paste_triggered {
                self.action_paste_clip();
            }

            // Split Clip (Ctrl+K or bare S)
            if self
                .keybindings
                .is_action_triggered(AppAction::SplitClip, &input)
                || (input.modifiers.command && input.key_pressed(egui::Key::K))
                || (!input.modifiers.any() && input.key_pressed(egui::Key::S))
            {
                self.action_split_selected_clip();
            }

            // Duplicate Clip (Ctrl+D)
            if self
                .keybindings
                .is_action_triggered(AppAction::DuplicateClip, &input)
                || (input.modifiers.command && input.key_pressed(egui::Key::D))
            {
                self.action_duplicate_selected_clip();
            }

            // Delete time selection or selected clip
            if self
                .keybindings
                .is_action_triggered(AppAction::Delete, &input)
                || input.key_pressed(egui::Key::Backspace)
            {
                self.action_delete_timeline_selection_or_clip();
            }

            // Undo
            if self
                .keybindings
                .is_action_triggered(AppAction::Undo, &input)
            {
                self.action_undo();
            }

            // Redo
            if self
                .keybindings
                .is_action_triggered(AppAction::Redo, &input)
                || (input.modifiers.command
                    && input.modifiers.shift
                    && input.key_pressed(egui::Key::Z))
            {
                self.action_redo();
            }
        }
    }
}
