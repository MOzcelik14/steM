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
            if ui
                .button(format!("{} (Ctrl+E)", app.i18n.t("menu-file-export")))
                .clicked()
            {
                app.show_export_dialog = true;
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
        });

        // ------------------ View Menu ------------------
        ui.menu_button(app.i18n.t("menu-view"), |ui| {
            if ui.button(app.i18n.t("menu-view-zoom-in")).clicked() {
                app.timeline_state.pixels_per_second =
                    (app.timeline_state.pixels_per_second * 1.25).min(500.0);
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-view-zoom-out")).clicked() {
                app.timeline_state.pixels_per_second =
                    (app.timeline_state.pixels_per_second * 0.8).max(5.0);
                ui.close_menu();
            }
            if ui.button(app.i18n.t("menu-view-reset-zoom")).clicked() {
                app.timeline_state.pixels_per_second = 60.0;
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
