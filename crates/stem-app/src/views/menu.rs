//! Menu bar implementation

use crate::app::StemApp;
use stem_i18n::Locale;

pub fn render_menu_bar(app: &mut StemApp, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        // App brand display
        ui.label(
            egui::RichText::new(stem_i18n::APP_DISPLAY_NAME)
                .strong()
                .color(egui::Color32::from_rgb(90, 180, 255)),
        );
        ui.separator();

        // ------------------ File Menu ------------------
        ui.menu_button(app.i18n.t("menu-file"), |ui| {
            if ui
                .button(format!("{} (Ctrl+N)", app.i18n.t("menu-file-new")))
                .clicked()
            {
                app.action_new_project();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-file-load-demo")).clicked() {
                app.action_load_demo();
                ui.close_menu();
            }
            if ui
                .button(format!("{} (Ctrl+O)", app.i18n.t("menu-file-open")))
                .clicked()
            {
                app.action_open_project();
                ui.close_menu();
            }
            if ui
                .button(format!("{} (Ctrl+S)", app.i18n.t("menu-file-save")))
                .clicked()
            {
                app.action_save_project();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-file-save-as")).clicked() {
                app.action_save_project_as();
                ui.close_menu();
            }
            ui.separator();
            if ui.button(app.i18n.t("menu-file-import-audio")).clicked() {
                app.action_import_audio();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-file-open-waveform")).clicked() {
                app.action_open_file_in_waveform_editor();
                ui.close_menu();
            }
            if ui
                .button(format!("{} (Ctrl+E)", app.i18n.t("menu-file-export")))
                .clicked()
            {
                app.show_export_dialog = true;
                ui.close_menu();
            }
            if ui.button(app.i18n.t("dialog-batch-export-title")).clicked() {
                app.batch_export_state.show = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button(app.i18n.t("menu-file-preferences")).clicked() {
                app.show_preferences_dialog = true;
                ui.close_menu();
            }
            ui.separator();
            if ui.button(app.i18n.t("menu-file-exit")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                ui.close_menu();
            }
        });

        // ------------------ Edit Menu ------------------
        ui.menu_button(app.i18n.t("menu-edit"), |ui| {
            if app.view_mode == crate::state::AppViewMode::Waveform {
                let has_doc = app.waveform_state.document.is_some();
                let has_sel = app.waveform_state.selection.is_some();
                let can_undo = app.waveform_state.history.can_undo();
                let can_redo = app.waveform_state.history.can_redo();

                if ui
                    .add_enabled(
                        has_doc && can_undo,
                        egui::Button::new(format!("{} (Ctrl+Z)", app.i18n.t("menu-edit-undo"))),
                    )
                    .clicked()
                {
                    app.action_waveform_undo();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc && can_redo,
                        egui::Button::new(format!("{} (Ctrl+Y)", app.i18n.t("menu-edit-redo"))),
                    )
                    .clicked()
                {
                    app.action_waveform_redo();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_doc && has_sel,
                        egui::Button::new(format!("{} (Ctrl+X)", app.i18n.t("waveform-btn-cut"))),
                    )
                    .clicked()
                {
                    app.action_waveform_cut();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc && has_sel,
                        egui::Button::new(format!("{} (Ctrl+C)", app.i18n.t("waveform-btn-copy"))),
                    )
                    .clicked()
                {
                    app.action_waveform_copy();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc && app.clipboard.is_some(),
                        egui::Button::new(format!("{} (Ctrl+V)", app.i18n.t("waveform-btn-paste"))),
                    )
                    .clicked()
                {
                    app.action_waveform_paste();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc && has_sel,
                        egui::Button::new(format!("{} (Del)", app.i18n.t("waveform-btn-delete"))),
                    )
                    .clicked()
                {
                    app.action_waveform_delete();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc && has_sel,
                        egui::Button::new(format!("{} (Ctrl+T)", app.i18n.t("waveform-btn-trim"))),
                    )
                    .clicked()
                {
                    app.action_waveform_trim();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        has_doc,
                        egui::Button::new(format!(
                            "{} (Ctrl+I)",
                            app.i18n.t("waveform-btn-silence")
                        )),
                    )
                    .clicked()
                {
                    app.action_waveform_insert_silence();
                    ui.close_menu();
                }
            } else {
                let can_undo = app.history.can_undo();
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new(format!("{} (Ctrl+Z)", app.i18n.t("menu-edit-undo"))),
                    )
                    .clicked()
                {
                    app.action_undo();
                    ui.close_menu();
                }
                let can_redo = app.history.can_redo();
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new(format!("{} (Ctrl+Y)", app.i18n.t("menu-edit-redo"))),
                    )
                    .clicked()
                {
                    app.action_redo();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .button(format!("{} (S)", app.i18n.t("menu-edit-split")))
                    .clicked()
                {
                    app.action_split_selected_clip();
                    ui.close_menu();
                }
                if ui
                    .button(format!("{} (Del)", app.i18n.t("menu-edit-delete")))
                    .clicked()
                {
                    app.action_delete_selected_clip();
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button(app.i18n.t("dialog-keybindings-title")).clicked() {
                app.keybindings_state.show = true;
                ui.close_menu();
            }
        });

        // ------------------ View Menu ------------------
        ui.menu_button(app.i18n.t("menu-view"), |ui| {
            if ui
                .button(format!("{} (1)", app.i18n.t("view-mode-multitrack")))
                .clicked()
            {
                app.action_switch_to_multitrack();
                ui.close_menu();
            }
            if ui
                .button(format!("{} (2)", app.i18n.t("view-mode-waveform")))
                .clicked()
            {
                app.action_switch_to_waveform();
                ui.close_menu();
            }
            ui.separator();
            if ui.button(app.i18n.t("menu-view-zoom-in")).clicked() {
                if app.view_mode == crate::state::AppViewMode::Waveform {
                    app.waveform_state.pixels_per_second =
                        (app.waveform_state.pixels_per_second * 1.25).min(5000.0);
                } else {
                    app.timeline_state.pixels_per_second =
                        (app.timeline_state.pixels_per_second * 1.25).min(500.0);
                }
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-view-zoom-out")).clicked() {
                if app.view_mode == crate::state::AppViewMode::Waveform {
                    app.waveform_state.pixels_per_second =
                        (app.waveform_state.pixels_per_second * 0.8).max(5.0);
                } else {
                    app.timeline_state.pixels_per_second =
                        (app.timeline_state.pixels_per_second * 0.8).max(5.0);
                }
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-view-reset-zoom")).clicked() {
                if app.view_mode == crate::state::AppViewMode::Waveform {
                    app.waveform_state.pixels_per_second = 120.0;
                } else {
                    app.timeline_state.pixels_per_second = 60.0;
                }
                ui.close_menu();
            }
            ui.separator();
            let mut mixer_open = app.show_mixer_panel;
            if ui
                .checkbox(&mut mixer_open, app.i18n.t("menu-view-toggle-mixer"))
                .clicked()
            {
                app.show_mixer_panel = mixer_open;
            }
            let mut spec_open = app.show_spectrogram_panel;
            if ui
                .checkbox(&mut spec_open, app.i18n.t("menu-view-toggle-spectrogram"))
                .clicked()
            {
                app.show_spectrogram_panel = spec_open;
            }
            let mut hist_open = app.show_history_panel;
            if ui
                .checkbox(&mut hist_open, app.i18n.t("menu-view-toggle-history"))
                .clicked()
            {
                app.show_history_panel = hist_open;
            }
        });

        // ------------------ Track Menu ------------------
        ui.menu_button(app.i18n.t("menu-track"), |ui| {
            if ui.button(app.i18n.t("menu-track-add-audio")).clicked() {
                let track_num = app.project.tracks.len() + 1;
                let track_name = app
                    .i18n
                    .t_arg("track-default-name", "number", track_num as i64);
                app.action_add_track(track_name);
                ui.close_menu();
            }
            if let Some(track_id) = app.timeline_state.selected_track_id {
                if ui.button(app.i18n.t("menu-track-delete")).clicked() {
                    app.action_delete_track(track_id);
                    ui.close_menu();
                }
            }
        });

        // ------------------ Markers Menu ------------------
        ui.menu_button(app.i18n.t("marker-title"), |ui| {
            if ui
                .button(format!("{} (M)", app.i18n.t("marker-add")))
                .clicked()
            {
                app.action_drop_marker(None);
                ui.close_menu();
            }
            if ui
                .button(format!("{} (Shift+M)", app.i18n.t("marker-add-named")))
                .clicked()
            {
                let frame = app.telemetry.playhead_frame;
                let count = app.project.markers.len() + 1;
                app.named_marker_state
                    .open_for_new(frame, format!("Marker {}", count));
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button(format!("{} ([)", app.i18n.t("marker-prev")))
                .clicked()
            {
                app.action_jump_prev_marker();
                ui.close_menu();
            }
            if ui
                .button(format!("{} (])", app.i18n.t("marker-next")))
                .clicked()
            {
                app.action_jump_next_marker();
                ui.close_menu();
            }
            ui.separator();
            if ui.button(app.i18n.t("marker-list")).clicked() {
                app.marker_list_state.show = true;
                ui.close_menu();
            }
        });

        // ------------------ Cleanup Menu ------------------
        ui.menu_button(app.i18n.t("menu-cleanup"), |ui| {
            let has_doc = app.waveform_state.document.is_some();
            let has_sel = app.waveform_state.selection.is_some();

            if ui
                .add_enabled(
                    has_doc && has_sel,
                    egui::Button::new(app.i18n.t("menu-cleanup-capture-noise")),
                )
                .clicked()
            {
                app.action_capture_noise_profile();
                ui.close_menu();
            }

            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(app.i18n.t("menu-cleanup-noise-reduction")),
                )
                .clicked()
            {
                app.noise_reduction_state.show = true;
                ui.close_menu();
            }

            ui.separator();

            if ui
                .add_enabled(
                    has_doc,
                    egui::Button::new(app.i18n.t("menu-cleanup-click-remover")),
                )
                .clicked()
            {
                app.click_remover_state.show = true;
                ui.close_menu();
            }
        });

        // ------------------ Analysis Menu ------------------
        ui.menu_button(app.i18n.t("menu-analysis"), |ui| {
            if ui.button(app.i18n.t("menu-analysis-loudness")).clicked() {
                app.action_open_loudness_report();
                ui.close_menu();
            }

            let mut spec_open = app.show_spectrogram_panel;
            if ui
                .checkbox(&mut spec_open, app.i18n.t("menu-analysis-spectrogram"))
                .clicked()
            {
                app.show_spectrogram_panel = spec_open;
            }
        });

        // ------------------ Language Quick Selector ------------------
        let current_locale = app.i18n.current_locale();
        ui.menu_button(format!("🌐 {}", current_locale.native_name()), |ui| {
            for locale in Locale::ALL {
                let is_active = *locale == current_locale;
                if ui
                    .selectable_label(is_active, locale.native_name())
                    .clicked()
                {
                    app.i18n.set_locale(*locale);
                    ui.close_menu();
                }
            }
        });

        // ------------------ Help Menu ------------------
        ui.menu_button(app.i18n.t("menu-help"), |ui| {
            if ui.button(app.i18n.t("menu-help-about")).clicked() {
                app.show_about_dialog = true;
                ui.close_menu();
            }
        });
    });
}
