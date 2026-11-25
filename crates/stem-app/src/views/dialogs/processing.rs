use crate::app::StemApp;

pub(super) fn render_noise_reduction_dialog(app: &mut StemApp, ctx: &egui::Context) {
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
                        .on_hover_text(app.i18n.t("nr-capture-profile-tooltip"))
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
                    let has_sel = app.waveform_state.selection.is_some()
                        || app.timeline_state.time_selection.is_some();
                    let has_target = app.waveform_state.document.is_some()
                        || app.timeline_state.selected_clip_id.is_some();

                    if ui
                        .add_enabled(has_target, egui::Button::new("Capture Profile"))
                        .clicked()
                    {
                        app.action_capture_noise_profile();
                    }

                    if ui
                        .add_enabled(
                            has_target && has_sel,
                            egui::Button::new(app.i18n.t("cleanup-btn-apply")),
                        )
                        .clicked()
                    {
                        app.action_apply_noise_reduction(false);
                        app.noise_reduction_state.show = false;
                    }

                    if ui
                        .add_enabled(
                            has_target,
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

pub(super) fn render_click_remover_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

pub(super) fn render_loudness_report_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

                    if let Some(ref report) = app.loudness_report_state.mastering_report.clone() {
                        ui.separator();
                        ui.label(egui::RichText::new("Mastering Assistant").strong());
                        egui::Grid::new("mastering_dyn_grid")
                            .num_columns(2)
                            .spacing([18.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("RMS Level:");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.2} dBFS",
                                        report.dynamics.rms_dbfs
                                    ))
                                    .monospace(),
                                );
                                ui.end_row();

                                ui.label("Crest Factor:");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.1} dB",
                                        report.dynamics.crest_factor_db
                                    ))
                                    .monospace(),
                                );
                                ui.end_row();

                                ui.label("Dynamic Range (PLR):");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.1} dB",
                                        report.dynamics.dynamic_range_db
                                    ))
                                    .monospace(),
                                );
                                ui.end_row();

                                ui.label("Clipped Samples:");
                                let clip_col = if report.dynamics.clipped_samples > 0 {
                                    egui::Color32::from_rgb(245, 90, 90)
                                } else {
                                    egui::Color32::from_rgb(80, 220, 120)
                                };
                                ui.colored_label(
                                    clip_col,
                                    format!("{}", report.dynamics.clipped_samples),
                                );
                                ui.end_row();

                                ui.label("Tonal Balance (L/M/H):");
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:.0}% / {:.0}% / {:.0}%",
                                        report.tonal_balance.low_pct,
                                        report.tonal_balance.mid_pct,
                                        report.tonal_balance.high_pct
                                    ))
                                    .monospace(),
                                );
                                ui.end_row();
                            });

                        for w in &report.warnings {
                            ui.colored_label(egui::Color32::from_rgb(245, 180, 80), w);
                        }

                        ui.horizontal(|ui| {
                            if ui
                                .button(format!(
                                    "Apply Safe Gain ({:+.1} dB)",
                                    report.suggested_gain_db
                                ))
                                .clicked()
                            {
                                app.action_set_master_gain(report.suggested_gain_db);
                            }
                            let mut ab = app.loudness_report_state.ab_reference_active;
                            if ui.toggle_value(&mut ab, "A/B Unity (0 dB)").changed() {
                                app.loudness_report_state.ab_reference_active = ab;
                                let g = if ab { 0.0 } else { report.suggested_gain_db };
                                app.action_set_master_gain(g);
                            }
                            if ui.button("Import Reference...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Audio", &["wav", "flac", "mp3", "ogg"])
                                    .pick_file()
                                {
                                    app.action_compare_reference_audio(&path);
                                }
                            }
                        });

                        if let Some(ref cmp) =
                            app.loudness_report_state.reference_comparison.clone()
                        {
                            ui.separator();
                            let ref_name = app
                                .loudness_report_state
                                .reference_audio_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Reference".to_string());
                            ui.label(
                                egui::RichText::new(format!("Reference Comparison ({})", ref_name))
                                    .strong(),
                            );
                            egui::Grid::new("ref_cmp_grid")
                                .num_columns(2)
                                .spacing([18.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label("Loudness Delta:");
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:+.1} LUFS (Ref: {:.1} LUFS)",
                                            cmp.lufs_delta_db,
                                            cmp.reference.dynamics.integrated_lufs
                                        ))
                                        .monospace(),
                                    );
                                    ui.end_row();

                                    ui.label("True Peak Delta:");
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:+.2} dBTP (Ref: {:.2} dBTP)",
                                            cmp.true_peak_delta_db,
                                            cmp.reference.dynamics.true_peak_dbfs
                                        ))
                                        .monospace(),
                                    );
                                    ui.end_row();

                                    ui.label("Spectrum Delta (L/M/H):");
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:+.0}% / {:+.0}% / {:+.0}%",
                                            cmp.low_delta_pct,
                                            cmp.mid_delta_pct,
                                            cmp.high_delta_pct
                                        ))
                                        .monospace(),
                                    );
                                    ui.end_row();
                                });
                        }
                    }
                } else {
                    ui.label(app.i18n.t("dialog-loudness-no-data"));
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
