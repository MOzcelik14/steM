use crate::app::StemApp;

pub(super) fn render_named_marker_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.named_marker_state.show {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("dialog-marker-title"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(320.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("marker-name"));
                    ui.text_edit_singleline(&mut app.named_marker_state.marker_name);
                });

                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("marker-frame"));
                    let mut frame_str = app.named_marker_state.target_frame.to_string();
                    if ui.text_edit_singleline(&mut frame_str).changed() {
                        if let Ok(f) = frame_str.parse::<u64>() {
                            app.named_marker_state.target_frame = f;
                        }
                    }
                });

                ui.checkbox(
                    &mut app.named_marker_state.is_cd_track,
                    app.i18n.t("marker-cd-track"),
                );

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("dialog-btn-save")).clicked() {
                        if let Some(id) = app.named_marker_state.edit_marker_id {
                            if let Some(m) = app.project.markers.iter_mut().find(|m| m.id == id) {
                                m.name = app.named_marker_state.marker_name.clone();
                                m.frame = app.named_marker_state.target_frame;
                                m.is_cd_track = app.named_marker_state.is_cd_track;
                            }
                            app.project.markers.sort_by_key(|m| m.frame);
                        } else {
                            let frame = app.named_marker_state.target_frame;
                            let name = app.named_marker_state.marker_name.clone();
                            let is_cd = app.named_marker_state.is_cd_track;
                            let id = app.project.add_marker(frame, name);
                            if let Some(m) = app.project.markers.iter_mut().find(|m| m.id == id) {
                                m.is_cd_track = is_cd;
                            }
                        }
                        app.named_marker_state.show = false;
                    }
                    if ui.button(app.i18n.t("dialog-cancel")).clicked() {
                        app.named_marker_state.show = false;
                    }
                });
            });
        });

    if !open {
        app.named_marker_state.show = false;
    }
}

pub(super) fn render_marker_list_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.marker_list_state.show {
        return;
    }

    let mut open = true;
    let mut close_dialog = false;
    let mut to_delete = None;
    let mut to_jump = None;
    let mut to_add = false;

    egui::Window::new(app.i18n.t("dialog-marker-list-title"))
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(440.0, 320.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("marker-add")).clicked() {
                        to_add = true;
                    }
                    ui.label(format!("Total: {}", app.project.markers.len()));
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        egui::Grid::new("marker_list_grid")
                            .striped(true)
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                ui.strong(app.i18n.t("marker-name"));
                                ui.strong(app.i18n.t("marker-frame"));
                                ui.strong("Type");
                                ui.strong("Actions");
                                ui.end_row();

                                for marker in &app.project.markers {
                                    ui.label(&marker.name);
                                    ui.label(marker.frame.to_string());
                                    if marker.is_cd_track {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(255, 200, 60),
                                            "CD Track",
                                        );
                                    } else {
                                        ui.label(app.i18n.t("dialog-marker-default-label"));
                                    }
                                    ui.horizontal(|ui| {
                                        if ui.small_button(app.i18n.t("marker-jump")).clicked() {
                                            to_jump = Some(marker.frame);
                                        }
                                        if ui.small_button(app.i18n.t("marker-delete")).clicked() {
                                            to_delete = Some(marker.id);
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });

                ui.add_space(8.0);
                if ui.button(app.i18n.t("dialog-close")).clicked() {
                    close_dialog = true;
                }
            });
        });

    if let Some(id) = to_delete {
        app.project.remove_marker(id);
    }
    if let Some(frame) = to_jump {
        app.action_seek(frame);
    }
    if to_add {
        let frame = app.telemetry.playhead_frame;
        let count = app.project.markers.len() + 1;
        app.named_marker_state
            .open_for_new(frame, format!("Marker {}", count));
    }

    if !open || close_dialog {
        app.marker_list_state.show = false;
    }
}

pub(super) fn render_batch_export_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.batch_export_state.show {
        return;
    }

    let mut open = true;
    let mut close_dialog = false;
    let mut start_export = false;

    egui::Window::new(app.i18n.t("dialog-batch-export-title"))
        .open(&mut open)
        .resizable(false)
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.checkbox(
                        &mut app.batch_export_state.export_stems,
                        app.i18n.t("batch-export-stems"),
                    );
                    ui.checkbox(
                        &mut app.batch_export_state.export_marker_regions,
                        app.i18n.t("batch-export-regions"),
                    );
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("batch-export-pattern"));
                    ui.text_edit_singleline(&mut app.batch_export_state.filename_pattern);
                });
                ui.label(
                    egui::RichText::new(app.i18n.t("batch-export-pattern-help"))
                        .size(10.0)
                        .color(egui::Color32::from_rgb(150, 150, 160)),
                );

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("batch-export-output-dir"));
                    let path_str = app.batch_export_state.output_dir.to_string_lossy();
                    ui.label(egui::RichText::new(path_str.as_ref()).monospace());
                    if ui.button(app.i18n.t("batch-export-browse")).clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            app.batch_export_state.output_dir = folder;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("batch-export-format"));
                    egui::ComboBox::from_id_salt("batch_export_format")
                        .selected_text(match app.batch_export_state.format {
                            stem_audio::ExportFormat::Wav => "WAV",
                            stem_audio::ExportFormat::Flac => "FLAC",
                            stem_audio::ExportFormat::Mp3 => "MP3",
                            stem_audio::ExportFormat::Ogg => "OGG",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.batch_export_state.format,
                                stem_audio::ExportFormat::Wav,
                                "WAV",
                            );
                            ui.selectable_value(
                                &mut app.batch_export_state.format,
                                stem_audio::ExportFormat::Flac,
                                "FLAC",
                            );
                            ui.selectable_value(
                                &mut app.batch_export_state.format,
                                stem_audio::ExportFormat::Mp3,
                                "MP3",
                            );
                            ui.selectable_value(
                                &mut app.batch_export_state.format,
                                stem_audio::ExportFormat::Ogg,
                                "OGG",
                            );
                        });

                    ui.label(app.i18n.t("batch-export-bit-depth"));
                    egui::ComboBox::from_id_salt("batch_export_bit_depth")
                        .selected_text(match app.batch_export_state.bit_depth {
                            stem_audio::BitDepth::Bit16 => "16-bit",
                            stem_audio::BitDepth::Bit24 => "24-bit",
                            stem_audio::BitDepth::Float32 => "32-bit Float",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.batch_export_state.bit_depth,
                                stem_audio::BitDepth::Bit16,
                                "16-bit",
                            );
                            ui.selectable_value(
                                &mut app.batch_export_state.bit_depth,
                                stem_audio::BitDepth::Bit24,
                                "24-bit",
                            );
                            ui.selectable_value(
                                &mut app.batch_export_state.bit_depth,
                                stem_audio::BitDepth::Float32,
                                "32-bit Float",
                            );
                        });
                });

                ui.checkbox(
                    &mut app.batch_export_state.overwrite_existing,
                    "Overwrite existing files",
                );

                if app.batch_export_state.in_progress {
                    ui.add_space(6.0);
                    ui.add(
                        egui::ProgressBar::new(app.batch_export_state.progress).show_percentage(),
                    );
                } else if let Some(ref msg) = app.batch_export_state.status_msg {
                    ui.add_space(6.0);
                    let color = app
                        .batch_export_state
                        .summary
                        .as_ref()
                        .map(|s| {
                            if s.failed > 0 {
                                egui::Color32::from_rgb(245, 160, 80)
                            } else {
                                egui::Color32::from_rgb(80, 220, 120)
                            }
                        })
                        .unwrap_or_else(|| egui::Color32::from_rgb(80, 220, 120));
                    ui.label(egui::RichText::new(msg).color(color));
                }

                if let Some(ref summary) = app.batch_export_state.summary {
                    if !summary.items.is_empty() {
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .max_height(130.0)
                            .show(ui, |ui| {
                                for item in &summary.items {
                                    match &item.status {
                                        crate::state::BatchItemStatus::Succeeded => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(80, 220, 120),
                                                format!("[OK] {}", item.filename),
                                            );
                                        }
                                        crate::state::BatchItemStatus::Skipped(reason) => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(220, 190, 90),
                                                format!("[SKIP] {} — {}", item.filename, reason),
                                            );
                                        }
                                        crate::state::BatchItemStatus::Failed(err) => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(245, 100, 100),
                                                format!("[ERR] {} — {}", item.filename, err),
                                            );
                                        }
                                    }
                                }
                            });
                    }
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if app.batch_export_state.in_progress {
                        if ui.button(app.i18n.t("dialog-cancel")).clicked() {
                            if let Some(ref tok) = app.batch_export_state.cancel_token {
                                tok.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    } else if ui
                        .button(egui::RichText::new(app.i18n.t("batch-export-btn")).strong())
                        .clicked()
                    {
                        start_export = true;
                    }
                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        close_dialog = true;
                    }
                });
            });
        });

    if start_export {
        app.run_batch_export();
    }

    if !open || close_dialog {
        app.batch_export_state.show = false;
    }
}
