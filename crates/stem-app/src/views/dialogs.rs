use crate::app::StemApp;
use stem_audio::{BitDepth, ExportFormat, ExportRange};
use stem_i18n::Locale;

pub fn render_dialogs(app: &mut StemApp, ctx: &egui::Context) {
    // ----------------- Preferences Dialog -----------------
    if app.show_preferences_dialog {
        let mut open = true;
        egui::Window::new(app.i18n.t("dialog-preferences-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        // Language Selector
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-preferences-language"));
                            let current_locale = app.i18n.current_locale();
                            egui::ComboBox::from_id_salt("pref_language_combo")
                                .selected_text(current_locale.native_name())
                                .show_ui(ui, |ui| {
                                    for loc in Locale::ALL {
                                        if ui
                                            .selectable_label(
                                                *loc == current_locale,
                                                loc.native_name(),
                                            )
                                            .clicked()
                                        {
                                            app.i18n.set_locale(*loc);
                                        }
                                    }
                                });
                        });

                        ui.separator();

                        // Audio Device
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-preferences-audio-device"));
                            let device_name = app
                                .audio_engine
                                .as_ref()
                                .map(|e| e.device_name())
                                .unwrap_or("None");
                            ui.label(egui::RichText::new(device_name).strong());
                        });

                        // Sample Rate
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-preferences-sample-rate"));
                            let sr = app.project.sample_rate;
                            ui.label(format!("{sr} Hz"));
                        });
                    });

                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(app.i18n.t("dialog-close")).clicked() {
                            app.show_preferences_dialog = false;
                        }
                    });
                });
            });
        if !open {
            app.show_preferences_dialog = false;
        }
    }

    // ----------------- Export Dialog -----------------
    if app.show_export_dialog {
        let mut open = true;
        egui::Window::new(app.i18n.t("export-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.group(|ui| {
                        // Output Path
                        ui.horizontal(|ui| {
                            let path_str = app
                                .export_destination_path
                                .as_deref()
                                .unwrap_or("No path selected");
                            ui.add(
                                egui::Label::new(egui::RichText::new(path_str).monospace())
                                    .truncate(),
                            );
                            if ui.button(app.i18n.t("export-btn-browse")).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("WAV Audio", &["wav"])
                                    .add_filter("FLAC Audio", &["flac"])
                                    .save_file()
                                {
                                    app.export_destination_path =
                                        Some(path.to_string_lossy().to_string());
                                }
                            }
                        });

                        ui.separator();

                        // Format
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("export-format"));
                            ui.radio_value(
                                &mut app.export_settings.format,
                                ExportFormat::Wav,
                                app.i18n.t("export-format-wav"),
                            );
                            ui.radio_value(
                                &mut app.export_settings.format,
                                ExportFormat::Flac,
                                app.i18n.t("export-format-flac"),
                            );
                        });

                        // Sample Rate
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("export-sample-rate"));
                            egui::ComboBox::from_id_salt("export_sample_rate")
                                .selected_text(format!("{} Hz", app.export_settings.sample_rate))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut app.export_settings.sample_rate,
                                        44100,
                                        "44100 Hz",
                                    );
                                    ui.selectable_value(
                                        &mut app.export_settings.sample_rate,
                                        48000,
                                        "48000 Hz",
                                    );
                                    ui.selectable_value(
                                        &mut app.export_settings.sample_rate,
                                        96000,
                                        "96000 Hz",
                                    );
                                });
                        });

                        // Bit Depth
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("export-bit-depth"));
                            ui.radio_value(
                                &mut app.export_settings.bit_depth,
                                BitDepth::Bit16,
                                app.i18n.t("export-16bit"),
                            );
                            ui.radio_value(
                                &mut app.export_settings.bit_depth,
                                BitDepth::Bit24,
                                app.i18n.t("export-24bit"),
                            );
                            if app.export_settings.format == ExportFormat::Wav {
                                ui.radio_value(
                                    &mut app.export_settings.bit_depth,
                                    BitDepth::Float32,
                                    app.i18n.t("export-32bit-float"),
                                );
                            }
                        });

                        // Export Range
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("export-range"));
                            let is_entire =
                                matches!(app.export_settings.range, ExportRange::EntireProject);
                            if ui
                                .selectable_label(is_entire, app.i18n.t("export-range-entire"))
                                .clicked()
                            {
                                app.export_settings.range = ExportRange::EntireProject;
                            }
                            let has_clip = app.timeline_state.selected_clip_id.is_some();
                            if ui
                                .add_enabled(
                                    has_clip,
                                    egui::SelectableLabel::new(
                                        !is_entire,
                                        app.i18n.t("export-range-selection"),
                                    ),
                                )
                                .clicked()
                            {
                                if let Some(clip_id) = app.timeline_state.selected_clip_id {
                                    for track in &app.project.tracks {
                                        if let Some(clip) =
                                            track.clips.iter().find(|c| c.id == clip_id)
                                        {
                                            app.export_settings.range = ExportRange::TimeRange {
                                                start_frame: clip.track_offset_frames,
                                                end_frame: clip.track_offset_frames
                                                    + clip.length_frames,
                                            };
                                            break;
                                        }
                                    }
                                }
                            }
                        });

                        // TPDF Dither
                        ui.checkbox(
                            &mut app.export_settings.apply_dither,
                            app.i18n.t("export-dither-enabled"),
                        );

                        // Normalize Before Export
                        let mut normalize = app.export_settings.normalize_target_lufs.is_some();
                        if ui
                            .checkbox(&mut normalize, app.i18n.t("export-normalize-before"))
                            .changed()
                        {
                            app.export_settings.normalize_target_lufs =
                                if normalize { Some(-14.0) } else { None };
                        }
                    });

                    // Progress bar if exporting
                    if let Some(progress) = app.export_progress {
                        ui.add_space(4.0);
                        ui.label(app.i18n.t("export-status-in-progress"));
                        ui.add(egui::ProgressBar::new(progress).show_percentage());
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if app.export_progress.is_some() {
                            if ui
                                .button(
                                    egui::RichText::new(app.i18n.t("export-btn-cancel"))
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                )
                                .clicked()
                            {
                                app.action_cancel_export();
                            }
                        } else {
                            if ui.button(app.i18n.t("dialog-cancel")).clicked() {
                                app.show_export_dialog = false;
                            }

                            let can_export = app.export_destination_path.is_some();
                            if ui
                                .add_enabled(
                                    can_export,
                                    egui::Button::new(app.i18n.t("export-btn-start")),
                                )
                                .clicked()
                            {
                                app.action_start_export();
                            }
                        }
                    });
                });
            });
        if !open {
            app.show_export_dialog = false;
        }
    }

    // ----------------- About Dialog -----------------
    if app.show_about_dialog {
        let mut open = true;
        egui::Window::new(app.i18n.t("dialog-about-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(stem_i18n::APP_DISPLAY_NAME)
                            .strong()
                            .size(24.0)
                            .color(egui::Color32::from_rgb(90, 180, 255)),
                    );
                    ui.label(egui::RichText::new(app.i18n.t("app-tagline")).italics());
                    ui.add_space(6.0);
                    ui.label(app.i18n.t("dialog-about-description"));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(app.i18n.t("dialog-about-license"))
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.label(
                        egui::RichText::new(app.i18n.t("dialog-about-built-with"))
                            .size(10.0)
                            .color(egui::Color32::GRAY),
                    );

                    ui.add_space(10.0);
                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        app.show_about_dialog = false;
                    }
                });
            });
        if !open {
            app.show_about_dialog = false;
        }
    }
}
