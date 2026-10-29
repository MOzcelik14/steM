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

    // ----------------- Noise Reduction Dialog -----------------
    render_noise_reduction_dialog(app, ctx);

    // ----------------- Click & Pop Remover Dialog -----------------
    render_click_remover_dialog(app, ctx);

    // ----------------- Loudness Report Dialog -----------------
    render_loudness_report_dialog(app, ctx);
}

fn render_noise_reduction_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.noise_reduction_state.show {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("cleanup-noise-reduction"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(app.i18n.t("cleanup-profile-status")).strong());
                    if let Some(ref prof) = app.noise_reduction_state.noise_profile {
                        ui.label(
                            egui::RichText::new(app.i18n.t_arg(
                                "cleanup-profile-captured",
                                "bins",
                                prof.magnitudes.len().to_string(),
                            ))
                            .color(egui::Color32::from_rgb(80, 220, 120)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(app.i18n.t("cleanup-profile-none"))
                                .color(egui::Color32::from_rgb(220, 180, 80)),
                        );
                    }

                    let has_sel = app.waveform_state.selection.is_some();
                    if ui
                        .add_enabled(
                            has_sel,
                            egui::Button::new(app.i18n.t("cleanup-btn-capture")),
                        )
                        .on_hover_text("Capture noise profile from current waveform selection")
                        .clicked()
                    {
                        app.action_capture_noise_profile();
                    }
                });

                ui.add_space(6.0);

                // Reduction Amount Slider
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("cleanup-reduction-amount"));
                    ui.add(
                        egui::Slider::new(&mut app.noise_reduction_state.reduction_db, 0.0..=48.0)
                            .suffix(" dB"),
                    );
                });

                // Sensitivity Slider
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("cleanup-sensitivity"));
                    ui.add(
                        egui::Slider::new(&mut app.noise_reduction_state.sensitivity, 0.2..=3.0)
                            .step_by(0.05),
                    );
                });

                if let Some(ref msg) = app.noise_reduction_state.status_message {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(msg).size(11.0).italics());
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let has_prof = app.noise_reduction_state.noise_profile.is_some();
                    let has_sel = app.waveform_state.selection.is_some();

                    if ui
                        .add_enabled(
                            has_prof && has_sel,
                            egui::Button::new(app.i18n.t("cleanup-btn-apply")),
                        )
                        .clicked()
                    {
                        app.action_apply_noise_reduction(false);
                        app.noise_reduction_state.show = false;
                    }

                    if ui
                        .add_enabled(
                            has_prof,
                            egui::Button::new(app.i18n.t("cleanup-btn-apply-all")),
                        )
                        .clicked()
                    {
                        app.action_apply_noise_reduction(true);
                        app.noise_reduction_state.show = false;
                    }

                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        app.noise_reduction_state.show = false;
                    }
                });
            });
        });

    if !open {
        app.noise_reduction_state.show = false;
    }
}

fn render_click_remover_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.click_remover_state.show {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("cleanup-click-remover"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("cleanup-click-sensitivity"));
                    ui.add(
                        egui::Slider::new(&mut app.click_remover_state.sensitivity, 1.0..=10.0)
                            .step_by(0.1),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("cleanup-click-max-width"));
                    ui.add(
                        egui::Slider::new(&mut app.click_remover_state.max_width, 2..=64)
                            .suffix(" smp"),
                    );
                });

                if let Some(ref msg) = app.click_remover_state.status_message {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(msg)
                            .color(egui::Color32::from_rgb(100, 220, 120))
                            .size(11.0),
                    );
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let has_doc = app.waveform_state.document.is_some();
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::Button::new(app.i18n.t("cleanup-click-btn-repair")),
                        )
                        .clicked()
                    {
                        app.action_apply_click_remover();
                    }

                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        app.click_remover_state.show = false;
                    }
                });
            });
        });

    if !open {
        app.click_remover_state.show = false;
    }
}

fn render_loudness_report_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.loudness_report_state.show {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("analysis-loudness-title"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Target Standard Selector
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(app.i18n.t("analysis-target-standard")).strong());
                    let current_std = app.loudness_report_state.standard;
                    egui::ComboBox::from_id_salt("loudness_std_combo")
                        .selected_text(current_std.name())
                        .show_ui(ui, |ui| {
                            let standards = [
                                crate::state::LoudnessStandard::EbuR128,
                                crate::state::LoudnessStandard::Streaming,
                                crate::state::LoudnessStandard::AppleMusic,
                                crate::state::LoudnessStandard::AesTd1004,
                            ];
                            for std in standards {
                                if ui
                                    .selectable_label(current_std == std, std.name())
                                    .clicked()
                                {
                                    app.loudness_report_state.standard = std;
                                }
                            }
                        });
                });

                ui.separator();

                if let Some(res) = app.loudness_report_state.result {
                    let target = app.loudness_report_state.standard.target_lufs();
                    let diff = res.integrated_lufs - target;
                    let is_compliant = diff.abs() <= 1.0;

                    let badge_color = if is_compliant {
                        egui::Color32::from_rgb(60, 200, 80)
                    } else if diff.abs() <= 2.0 {
                        egui::Color32::from_rgb(220, 180, 50)
                    } else {
                        egui::Color32::from_rgb(240, 60, 60)
                    };

                    let badge_text = if is_compliant {
                        app.i18n.t("analysis-compliant")
                    } else {
                        app.i18n.t("analysis-non-compliant")
                    };

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(badge_text)
                                .color(egui::Color32::BLACK)
                                .background_color(badge_color)
                                .strong(),
                        );
                        ui.label(format!(
                            "{}: {:+.1} LU",
                            app.i18n.t("analysis-target-diff"),
                            diff
                        ));
                    });

                    ui.add_space(6.0);

                    egui::Grid::new("loudness_metrics_grid")
                        .striped(true)
                        .spacing(egui::vec2(16.0, 6.0))
                        .show(ui, |ui| {
                            ui.label(app.i18n.t("analysis-integrated-lufs"));
                            ui.label(
                                egui::RichText::new(format!("{:.1} LUFS", res.integrated_lufs))
                                    .strong()
                                    .monospace(),
                            );
                            ui.end_row();

                            ui.label(app.i18n.t("analysis-momentary-max"));
                            ui.label(
                                egui::RichText::new(format!("{:.1} LUFS", res.momentary_max_lufs))
                                    .monospace(),
                            );
                            ui.end_row();

                            ui.label(app.i18n.t("analysis-short-term-max"));
                            ui.label(
                                egui::RichText::new(format!("{:.1} LUFS", res.short_term_max_lufs))
                                    .monospace(),
                            );
                            ui.end_row();

                            ui.label(app.i18n.t("analysis-loudness-range"));
                            ui.label(
                                egui::RichText::new(format!("{:.1} LU", res.loudness_range_lu))
                                    .monospace(),
                            );
                            ui.end_row();

                            ui.label(app.i18n.t("analysis-true-peak"));
                            let tp = app.loudness_report_state.true_peak_dbfs;
                            let tp_color =
                                if tp > app.loudness_report_state.standard.max_true_peak_dbfs() {
                                    egui::Color32::from_rgb(255, 60, 60)
                                } else {
                                    egui::Color32::from_rgb(80, 220, 120)
                                };
                            ui.label(
                                egui::RichText::new(format!("{:.2} dBFS", tp))
                                    .monospace()
                                    .strong()
                                    .color(tp_color),
                            );
                            ui.end_row();
                        });
                } else {
                    ui.label("No analysis data available. Click Re-analyze to measure.");
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("analysis-btn-reanalyze")).clicked() {
                        app.action_open_loudness_report();
                    }
                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        app.loudness_report_state.show = false;
                    }
                });
            });
        });

    if !open {
        app.loudness_report_state.show = false;
    }
}
