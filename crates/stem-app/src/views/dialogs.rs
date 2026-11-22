use crate::app::StemApp;
use stem_audio::{BitDepth, ExportFormat, ExportRange};
use stem_i18n::Locale;

pub fn render_dialogs(app: &mut StemApp, ctx: &egui::Context) {
    // ----------------- Unsaved Changes Close Confirmation Dialog -----------------
    if app.show_unsaved_close_dialog {
        let mut close_dialog = false;
        let mut do_quit = false;
        let mut do_save_and_quit = false;

        egui::Window::new(app.i18n.t("dialog-unsaved-changes-title"))
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(app.i18n.t("dialog-unsaved-changes-message"));
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button(app.i18n.t("dialog-save")).clicked() {
                            do_save_and_quit = true;
                        }
                        if ui.button(app.i18n.t("dialog-dont-save")).clicked() {
                            do_quit = true;
                        }
                        if ui.button(app.i18n.t("dialog-cancel")).clicked() {
                            close_dialog = true;
                        }
                    });
                });
            });

        if do_save_and_quit {
            app.action_save_project();
            if app.project_file_path.is_some() {
                app.discard_recovery_journal();
                app.force_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            app.show_unsaved_close_dialog = false;
        } else if do_quit {
            app.discard_recovery_journal();
            app.show_unsaved_close_dialog = false;
            app.force_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if close_dialog {
            app.show_unsaved_close_dialog = false;
        }
    }

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

                        // Audio Output Device
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-preferences-audio-device"));
                            let device_name = app
                                .audio_engine
                                .as_ref()
                                .map(|e| e.device_name())
                                .unwrap_or("None");
                            ui.label(egui::RichText::new(device_name).strong());
                        });

                        // Audio Input Device
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("recording-input-device"));
                            let current_input = app
                                .recording_state
                                .selected_input_device
                                .as_deref()
                                .unwrap_or("Default");
                            let mut selected = current_input.to_string();
                            let available = app.recording_state.available_input_devices.clone();

                            egui::ComboBox::from_id_salt("pref_input_device_combo")
                                .selected_text(&selected)
                                .width(180.0)
                                .show_ui(ui, |ui| {
                                    for dev in &available {
                                        if ui
                                            .selectable_value(&mut selected, dev.clone(), dev)
                                            .clicked()
                                        {
                                            app.action_set_input_device(Some(dev.clone()));
                                        }
                                    }
                                });
                        });

                        // Output Device Selection
                        ui.horizontal(|ui| {
                            ui.label("Output Device:");
                            let selected_out = app
                                .recording_state
                                .selected_output_device
                                .clone()
                                .unwrap_or_else(|| app.i18n.t("recording-no-device"));
                            egui::ComboBox::from_id_salt("pref_output_device")
                                .selected_text(selected_out)
                                .show_ui(ui, |ui| {
                                    for dev in app.recording_state.available_output_devices.clone()
                                    {
                                        let is_sel =
                                            app.recording_state.selected_output_device.as_ref()
                                                == Some(&dev);
                                        if ui.selectable_label(is_sel, &dev).clicked() {
                                            app.recording_state.selected_output_device =
                                                Some(dev.clone());
                                            app.action_reconfigure_audio_devices();
                                        }
                                    }
                                });
                        });

                        // Audio Buffer Size / Latency Preset
                        ui.horizontal(|ui| {
                            ui.label("Buffer Size:");
                            let cur_buf = app.recording_state.buffer_size_frames;
                            let cur_label = match cur_buf {
                                None => "Auto".to_string(),
                                Some(n) => format!("{n} samples"),
                            };
                            egui::ComboBox::from_id_salt("pref_buffer_size")
                                .selected_text(cur_label)
                                .show_ui(ui, |ui| {
                                    for opt in [
                                        None,
                                        Some(128),
                                        Some(256),
                                        Some(512),
                                        Some(1024),
                                        Some(2048),
                                    ] {
                                        let lbl = match opt {
                                            None => "Auto".to_string(),
                                            Some(n) => format!("{n} samples"),
                                        };
                                        if ui.selectable_label(cur_buf == opt, lbl).clicked() {
                                            app.recording_state.buffer_size_frames = opt;
                                            app.action_reconfigure_audio_devices();
                                        }
                                    }
                                });
                            if ui.button("Reconnect").clicked() {
                                app.action_reconfigure_audio_devices();
                            }
                        });

                        // Latency Compensation
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("recording-latency-compensation"));
                            let mut lat = app.project.latency_compensation_ms;
                            if ui
                                .add(egui::Slider::new(&mut lat, 0.0..=100.0).suffix(" ms"))
                                .changed()
                            {
                                app.project.latency_compensation_ms = lat;
                            }
                            ui.label(format!(
                                "({} samples)",
                                app.project.latency_compensation_frames()
                            ));
                        });

                        // Sample Rate
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-preferences-sample-rate"));
                            let sr = app
                                .audio_engine
                                .as_ref()
                                .map(|e| e.sample_rate())
                                .unwrap_or(app.project.sample_rate);
                            ui.label(format!("{sr} Hz"));
                        });

                        // Software Input Monitoring Toggle
                        ui.horizontal(|ui| {
                            let mut mon = app.project.input_monitoring;
                            if ui
                                .checkbox(&mut mon, app.i18n.t("recording-monitoring"))
                                .changed()
                            {
                                app.action_toggle_input_monitoring();
                            }
                        });

                        ui.separator();

                        // Keyboard Shortcuts Editor Button
                        ui.horizontal(|ui| {
                            ui.label(app.i18n.t("dialog-keybindings-title"));
                            if ui.button(app.i18n.t("dialog-keybindings-title")).clicked() {
                                app.keybindings_state.show = true;
                            }
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

    // ----------------- Hardware Disconnection Alert -----------------
    if app.recording_state.device_lost_warning {
        let mut open = true;
        egui::Window::new(
            egui::RichText::new(format!(
                "⚠ {}",
                app.i18n.t("recording-device-warning-title")
            ))
            .color(egui::Color32::from_rgb(255, 100, 100)),
        )
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(app.i18n.t("recording-device-warning-msg"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Reconnect").clicked() {
                        app.action_reconfigure_audio_devices();
                    }
                    if ui.button(app.i18n.t("dialog-close")).clicked() {
                        app.recording_state.device_lost_warning = false;
                    }
                    if ui.button(app.i18n.t("menu-file-preferences")).clicked() {
                        app.recording_state.device_lost_warning = false;
                        app.show_preferences_dialog = true;
                    }
                });
            });
        });
        if !open {
            app.recording_state.device_lost_warning = false;
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
                                    .add_filter("WAV Audio", &["wav", "WAV"])
                                    .add_filter("FLAC Audio", &["flac", "FLAC"])
                                    .add_filter("MP3 Audio", &["mp3", "MP3"])
                                    .add_filter("OGG Vorbis", &["ogg", "OGG"])
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
                            ui.radio_value(
                                &mut app.export_settings.format,
                                ExportFormat::Mp3,
                                "MP3",
                            );
                            ui.radio_value(
                                &mut app.export_settings.format,
                                ExportFormat::Ogg,
                                "OGG",
                            );
                        });

                        // MP3 Bitrate or OGG Quality
                        if app.export_settings.format == ExportFormat::Mp3 {
                            ui.horizontal(|ui| {
                                ui.label(app.i18n.t("export-mp3-bitrate"));
                                ui.radio_value(
                                    &mut app.export_settings.mp3_bitrate_kbps,
                                    128,
                                    "128 kbps",
                                );
                                ui.radio_value(
                                    &mut app.export_settings.mp3_bitrate_kbps,
                                    192,
                                    "192 kbps",
                                );
                                ui.radio_value(
                                    &mut app.export_settings.mp3_bitrate_kbps,
                                    320,
                                    "320 kbps",
                                );
                            });
                        } else if app.export_settings.format == ExportFormat::Ogg {
                            ui.horizontal(|ui| {
                                ui.label(app.i18n.t("export-ogg-quality"));
                                ui.add(
                                    egui::Slider::new(
                                        &mut app.export_settings.ogg_quality,
                                        0.1..=1.0,
                                    )
                                    .fixed_decimals(2),
                                );
                            });
                        }

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

                        // Bit Depth (for lossless formats)
                        if matches!(
                            app.export_settings.format,
                            ExportFormat::Wav | ExportFormat::Flac
                        ) {
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
                        }

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
                            let has_range_or_clip = app.timeline_state.time_selection.is_some()
                                || app.timeline_state.selected_clip_id.is_some();
                            if ui
                                .add_enabled(
                                    has_range_or_clip,
                                    egui::SelectableLabel::new(
                                        !is_entire,
                                        app.i18n.t("export-range-selection"),
                                    ),
                                )
                                .clicked()
                            {
                                if let Some((s, e)) = app.timeline_state.time_selection {
                                    app.export_settings.range = ExportRange::TimeRange {
                                        start_frame: s,
                                        end_frame: e,
                                    };
                                } else if let Some(clip_id) = app.timeline_state.selected_clip_id {
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

                        // Normalize Before Export + LUFS Presets
                        let mut normalize = app.export_settings.normalize_target_lufs.is_some();
                        if ui
                            .checkbox(&mut normalize, app.i18n.t("export-normalize-before"))
                            .changed()
                        {
                            app.export_settings.normalize_target_lufs =
                                if normalize { Some(-14.0) } else { None };
                        }
                        if let Some(ref mut lufs) = app.export_settings.normalize_target_lufs {
                            ui.horizontal(|ui| {
                                ui.selectable_value(
                                    lufs,
                                    -14.0,
                                    app.i18n.t("export-lufs-streaming"),
                                );
                                ui.selectable_value(lufs, -16.0, app.i18n.t("export-lufs-podcast"));
                                ui.selectable_value(lufs, -23.0, app.i18n.t("export-lufs-ebu"));
                            });
                        }

                        if !app.project.markers.is_empty() {
                            ui.add_space(4.0);
                            if ui.button(app.i18n.t("export-chapter-markers")).clicked() {
                                app.action_export_chapter_markers();
                            }
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
                    let texture = app.logo_texture.get_or_insert_with(|| {
                        let icon_rgba = include_bytes!("../../../../assets/icon.rgba");
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied([256, 256], icon_rgba);
                        ctx.load_texture("stem_logo", color_image, egui::TextureOptions::LINEAR)
                    });
                    ui.image((texture.id(), egui::vec2(96.0, 96.0)));

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

    // ----------------- Phase F Dialogs -----------------
    render_named_marker_dialog(app, ctx);
    render_marker_list_dialog(app, ctx);
    render_batch_export_dialog(app, ctx);
    render_keybindings_dialog(app, ctx);
    render_recovery_dialog(app, ctx);
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

fn render_named_marker_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

fn render_marker_list_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

fn render_batch_export_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

fn render_keybindings_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.keybindings_state.show {
        return;
    }

    let mut open = true;
    let mut close_dialog = false;
    let conflicts = app.keybindings.detect_conflicts();

    egui::Window::new(app.i18n.t("dialog-keybindings-title"))
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(520.0, 420.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Conflict warning banner
                if !conflicts.is_empty() {
                    ui.group(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 100, 80),
                            format!("⚠ {}", app.i18n.t("keybindings-conflicts-warning")),
                        );
                        for (a1, a2, sc) in &conflicts {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Clash: '{}' and '{}' both use '{}'",
                                    a1.display_name(),
                                    a2.display_name(),
                                    sc.display_string()
                                ))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(255, 160, 100)),
                            );
                        }
                    });
                    ui.add_space(4.0);
                }

                // Search Filter
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("keybindings-filter"));
                    ui.text_edit_singleline(&mut app.keybindings_state.filter);
                });
                ui.separator();

                // Shortcuts list table
                let filter_lower = app.keybindings_state.filter.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        egui::Grid::new("keybindings_grid")
                            .striped(true)
                            .min_col_width(120.0)
                            .show(ui, |ui| {
                                ui.strong(app.i18n.t("keybindings-action"));
                                ui.strong(app.i18n.t("keybindings-shortcut"));
                                ui.end_row();

                                for action in crate::state::AppAction::all() {
                                    let name = action.display_name();
                                    if !filter_lower.is_empty()
                                        && !name.to_lowercase().contains(&filter_lower)
                                    {
                                        continue;
                                    }

                                    ui.label(name);
                                    let shortcut_str = app
                                        .keybindings
                                        .bindings
                                        .get(action)
                                        .map(|s| s.display_string())
                                        .unwrap_or_else(|| "None".to_string());
                                    ui.label(
                                        egui::RichText::new(shortcut_str)
                                            .monospace()
                                            .strong()
                                            .color(egui::Color32::from_rgb(80, 200, 255)),
                                    );
                                    ui.end_row();
                                }
                            });
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("keybindings-reset")).clicked() {
                        app.keybindings.reset_to_defaults();
                    }
                    if ui.button(app.i18n.t("keybindings-export")).clicked() {
                        if let Ok(json) = app.keybindings.to_json() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .set_file_name("stem_keybindings.json")
                                .save_file()
                            {
                                let _ = std::fs::write(path, json);
                            }
                        }
                    }
                    if ui.button(app.i18n.t("keybindings-import")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            if let Ok(json) = std::fs::read_to_string(path) {
                                if let Ok(imported) = crate::state::KeyBindingMap::from_json(&json)
                                {
                                    app.keybindings = imported;
                                }
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(app.i18n.t("dialog-close")).clicked() {
                            close_dialog = true;
                        }
                    });
                });
            });
        });

    if !open || close_dialog {
        app.keybindings_state.show = false;
    }
}

fn render_recovery_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.recovery_state.show {
        return;
    }

    let mut open = true;
    let mut restore_clicked = false;
    let mut discard_clicked = false;

    egui::Window::new(app.i18n.t("dialog-recovery-title"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 60),
                    format!("⚠ {}", app.i18n.t("dialog-recovery-title")),
                );
                ui.add_space(4.0);
                let time_str = &app.recovery_state.recovery_time_str;
                let prompt = app.i18n.t_arg("recovery-prompt", "time", time_str.clone());
                ui.label(prompt);

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new(app.i18n.t("recovery-restore")).strong())
                        .clicked()
                    {
                        restore_clicked = true;
                    }
                    if ui.button(app.i18n.t("recovery-discard")).clicked() {
                        discard_clicked = true;
                    }
                });
            });
        });

    if restore_clicked {
        app.restore_recovery_journal();
    }
    if discard_clicked {
        app.discard_recovery_journal();
    }
    if !open {
        app.recovery_state.show = false;
    }

    render_plugin_browser_dialog(app, ctx);
    render_stem_separation_dialog(app, ctx);
}

fn render_plugin_browser_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.plugin_browser_state.show {
        return;
    }

    if !app.plugin_browser_state.scanned_once {
        app.plugin_browser_state.discovered = stem_audio::PluginScanner::scan_default_paths();
        app.plugin_browser_state.scanned_once = true;
    }

    let mut open = true;
    let mut close_dialog = false;

    egui::Window::new("Plugin Browser (VST3 / LV2)")
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(620.0, 480.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Scan Plugins").clicked() {
                    app.plugin_browser_state.discovered =
                        stem_audio::PluginScanner::scan_default_paths();
                    app.plugin_browser_state.status_msg = Some(format!(
                        "Found {} plugin(s)",
                        app.plugin_browser_state.discovered.len()
                    ));
                }
                ui.add(
                    egui::TextEdit::singleline(&mut app.plugin_browser_state.search_query)
                        .hint_text("Search plugins..."),
                );
                egui::ComboBox::from_id_salt("plugin_fmt_filter")
                    .selected_text(match app.plugin_browser_state.format_filter {
                        crate::state::PluginFormatFilter::All => "All",
                        crate::state::PluginFormatFilter::Vst3 => "VST3",
                        crate::state::PluginFormatFilter::Lv2 => "LV2",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.plugin_browser_state.format_filter,
                            crate::state::PluginFormatFilter::All,
                            "All",
                        );
                        ui.selectable_value(
                            &mut app.plugin_browser_state.format_filter,
                            crate::state::PluginFormatFilter::Vst3,
                            "VST3",
                        );
                        ui.selectable_value(
                            &mut app.plugin_browser_state.format_filter,
                            crate::state::PluginFormatFilter::Lv2,
                            "LV2",
                        );
                    });
            });

            // Target track selector
            if !app.project.tracks.is_empty() {
                if app.plugin_browser_state.target_track_idx >= app.project.tracks.len() {
                    app.plugin_browser_state.target_track_idx = 0;
                }
                let t_idx = app.plugin_browser_state.target_track_idx;
                ui.horizontal(|ui| {
                    ui.label("Target Track:");
                    egui::ComboBox::from_id_salt("plugin_target_track")
                        .selected_text(&app.project.tracks[t_idx].name)
                        .show_ui(ui, |ui| {
                            for (i, tr) in app.project.tracks.iter().enumerate() {
                                ui.selectable_value(
                                    &mut app.plugin_browser_state.target_track_idx,
                                    i,
                                    &tr.name,
                                );
                            }
                        });
                });
            }

            if let Some(ref msg) = app.plugin_browser_state.status_msg {
                ui.colored_label(egui::Color32::from_rgb(80, 200, 255), msg);
            }

            ui.separator();
            ui.label(egui::RichText::new("Discovered System Plugins").strong());
            let q = app.plugin_browser_state.search_query.to_lowercase();
            let filter = app.plugin_browser_state.format_filter;
            let mut insert_idx: Option<usize> = None;

            egui::ScrollArea::vertical()
                .id_salt("plugin_discovered_scroll")
                .max_height(170.0)
                .show(ui, |ui| {
                    if app.plugin_browser_state.discovered.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "No VST3 or LV2 bundles found in default system directories.",
                            )
                            .weak(),
                        );
                    }
                    for (i, desc) in app.plugin_browser_state.discovered.iter().enumerate() {
                        match filter {
                            crate::state::PluginFormatFilter::Vst3
                                if desc.format != stem_core::PluginFormat::Vst3 =>
                            {
                                continue;
                            }
                            crate::state::PluginFormatFilter::Lv2
                                if desc.format != stem_core::PluginFormat::Lv2 =>
                            {
                                continue;
                            }
                            _ => {}
                        }
                        if !q.is_empty()
                            && !desc.name.to_lowercase().contains(&q)
                            && !desc.vendor.to_lowercase().contains(&q)
                        {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            let fmt_tag = match desc.format {
                                stem_core::PluginFormat::Vst3 => "[VST3]",
                                stem_core::PluginFormat::Lv2 => "[LV2]",
                            };
                            ui.label(
                                egui::RichText::new(fmt_tag)
                                    .monospace()
                                    .color(egui::Color32::from_rgb(56, 189, 248)),
                            );
                            ui.label(egui::RichText::new(&desc.name).strong());
                            ui.label(egui::RichText::new(&desc.vendor).weak());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if !app.project.tracks.is_empty()
                                        && ui.button("Insert").clicked()
                                    {
                                        insert_idx = Some(i);
                                    }
                                },
                            );
                        });
                    }
                });

            if let Some(i) = insert_idx {
                if let Some(desc) = app.plugin_browser_state.discovered.get(i).cloned() {
                    let t_idx = app.plugin_browser_state.target_track_idx;
                    if let Some(track) = app.project.tracks.get_mut(t_idx) {
                        let slot = desc.to_slot_config();
                        let msg = match stem_audio::PluginHostInstance::load(&slot) {
                            Ok(_) => format!("Inserted '{}' on {}", slot.name, track.name),
                            Err(err) => format!("Inserted '{}' (warning: {err})", slot.name),
                        };
                        track.plugins.push(slot);
                        app.plugin_browser_state.status_msg = Some(msg);
                    }
                }
            }

            ui.separator();
            ui.label(egui::RichText::new("Track Plugin Inserts").strong());
            let mut remove_slot: Option<usize> = None;
            let mut add_auto_plugin: Option<uuid::Uuid> = None;
            let t_idx = app.plugin_browser_state.target_track_idx;
            if let Some(track) = app.project.tracks.get_mut(t_idx) {
                if track.plugins.is_empty() {
                    ui.label(egui::RichText::new("No external plugins on this track.").weak());
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("plugin_track_inserts_scroll")
                        .max_height(130.0)
                        .show(ui, |ui| {
                            for (s_idx, slot) in track.plugins.iter_mut().enumerate() {
                                let status =
                                    stem_audio::PluginHostInstance::validate_slot_status(slot);
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut slot.bypassed, "Bypass");
                                    ui.label(egui::RichText::new(&slot.name).strong());
                                    match status {
                                        stem_audio::PluginStatus::Loaded
                                        | stem_audio::PluginStatus::Available => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(80, 220, 120),
                                                "Loaded",
                                            );
                                        }
                                        stem_audio::PluginStatus::Missing => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(245, 100, 100),
                                                "Missing",
                                            );
                                        }
                                        stem_audio::PluginStatus::Incompatible(ref r) => {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(245, 170, 80),
                                                format!("Incompatible: {r}"),
                                            );
                                        }
                                    }
                                    if ui.button("Automate").clicked() {
                                        add_auto_plugin = Some(slot.id);
                                    }
                                    if ui.button("Remove").clicked() {
                                        remove_slot = Some(s_idx);
                                    }
                                });
                            }
                        });
                }
                if let Some(pid) = add_auto_plugin {
                    let target = stem_core::project::AutomationTarget::PluginParam {
                        plugin_id: pid,
                        param_index: 0,
                    };
                    if !track.automation_lanes.iter().any(|l| l.target == target) {
                        track
                            .automation_lanes
                            .push(stem_core::project::AutomationLane::new(target));
                    }
                    app.timeline_state.expanded_automation_track_id = Some(track.id);
                    app.timeline_state.active_automation_target = target;
                }
                if let Some(s_idx) = remove_slot {
                    if s_idx < track.plugins.len() {
                        track.plugins.remove(s_idx);
                    }
                }
            }

            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(app.i18n.t("dialog-close")).clicked() {
                    close_dialog = true;
                }
            });
        });

    if !open || close_dialog {
        app.plugin_browser_state.show = false;
    }
}

fn render_stem_separation_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.stem_separation_state.show {
        return;
    }

    if app.stem_separation_state.demucs_available.is_none() {
        app.stem_separation_state.demucs_available = Some(stem_audio::DemucsRunner::is_available());
    }

    let mut open = true;
    let mut close_dialog = false;
    let mut start_sep = false;

    egui::Window::new("AI Stem Separation (Demucs)")
        .open(&mut open)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            let available = app.stem_separation_state.demucs_available.unwrap_or(false);
            if available {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 220, 120),
                    "Demucs CLI detected on system PATH.",
                );
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(245, 170, 80),
                    "Demucs CLI not found. Install with: pip install demucs",
                );
            }

            ui.separator();

            let sources: Vec<_> = app.project.sources.values().collect();
            if sources.is_empty() {
                ui.label("Import or load an audio source first.");
            } else {
                if app.stem_separation_state.source_idx >= sources.len() {
                    app.stem_separation_state.source_idx = 0;
                }
                ui.horizontal(|ui| {
                    ui.label("Source Audio:");
                    let cur_name = &sources[app.stem_separation_state.source_idx].filename;
                    egui::ComboBox::from_id_salt("demucs_source_combo")
                        .selected_text(cur_name)
                        .show_ui(ui, |ui| {
                            for (i, s) in sources.iter().enumerate() {
                                ui.selectable_value(
                                    &mut app.stem_separation_state.source_idx,
                                    i,
                                    &s.filename,
                                );
                            }
                        });
                });
            }

            ui.horizontal(|ui| {
                ui.label("Model:");
                egui::ComboBox::from_id_salt("demucs_model_combo")
                    .selected_text(app.stem_separation_state.model.display_name())
                    .show_ui(ui, |ui| {
                        for m in [
                            stem_audio::DemucsModel::Htdemucs,
                            stem_audio::DemucsModel::HtdemucsFt,
                            stem_audio::DemucsModel::MdxExtra,
                            stem_audio::DemucsModel::Htdemucs6s,
                        ] {
                            ui.selectable_value(
                                &mut app.stem_separation_state.model,
                                m,
                                m.display_name(),
                            );
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Quality:");
                egui::ComboBox::from_id_salt("demucs_quality_combo")
                    .selected_text(match app.stem_separation_state.quality {
                        stem_audio::SeparationQuality::Fast => "Fast (0 shifts)",
                        stem_audio::SeparationQuality::Balanced => "Balanced (1 shift)",
                        stem_audio::SeparationQuality::HighQuality => "High Quality (2 shifts)",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.stem_separation_state.quality,
                            stem_audio::SeparationQuality::Fast,
                            "Fast (0 shifts)",
                        );
                        ui.selectable_value(
                            &mut app.stem_separation_state.quality,
                            stem_audio::SeparationQuality::Balanced,
                            "Balanced (1 shift)",
                        );
                        ui.selectable_value(
                            &mut app.stem_separation_state.quality,
                            stem_audio::SeparationQuality::HighQuality,
                            "High Quality (2 shifts)",
                        );
                    });
            });

            ui.checkbox(
                &mut app.stem_separation_state.prefer_gpu,
                "Prefer GPU acceleration (CUDA if available)",
            );
            ui.checkbox(
                &mut app.stem_separation_state.replace_existing_stems,
                "Replace existing Vocals/Drums/Bass/Other tracks",
            );

            if app.stem_separation_state.in_progress {
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(app.stem_separation_state.progress).show_percentage(),
                );
            } else if let Some(ref msg) = app.stem_separation_state.status_msg {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::from_rgb(80, 200, 255), msg);
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if app.stem_separation_state.in_progress {
                    if ui.button(app.i18n.t("dialog-cancel")).clicked() {
                        if let Some(ref tok) = app.stem_separation_state.cancel_token {
                            tok.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                } else if ui
                    .add_enabled(!sources.is_empty(), egui::Button::new("Separate Stems"))
                    .clicked()
                {
                    start_sep = true;
                }
                if ui.button(app.i18n.t("dialog-close")).clicked() {
                    close_dialog = true;
                }
            });
        });

    if start_sep {
        app.action_start_stem_separation();
    }
    if !open || close_dialog {
        app.stem_separation_state.show = false;
    }

    // ----------------- Edit History Panel -----------------
    if app.show_history_panel {
        let mut hist_open = true;
        let mut trigger_undo = false;
        let mut trigger_redo = false;
        let undo_items = app.history.undo_descriptions();
        let redo_items = app.history.redo_descriptions();

        egui::Window::new(app.i18n.t("history-panel-title"))
            .open(&mut hist_open)
            .default_width(280.0)
            .default_height(320.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            app.history.can_undo(),
                            egui::Button::new(app.i18n.t("menu-edit-undo")),
                        )
                        .clicked()
                    {
                        trigger_undo = true;
                    }
                    if ui
                        .add_enabled(
                            app.history.can_redo(),
                            egui::Button::new(app.i18n.t("menu-edit-redo")),
                        )
                        .clicked()
                    {
                        trigger_redo = true;
                    }
                });
                ui.separator();

                if undo_items.is_empty() && redo_items.is_empty() {
                    ui.label(app.i18n.t("history-empty"));
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if !undo_items.is_empty() {
                            ui.strong(app.i18n.t("history-undo-stack"));
                            for (idx, desc) in undo_items.iter().enumerate() {
                                ui.label(format!("{}. {}", idx + 1, desc));
                            }
                            ui.add_space(6.0);
                        }
                        if !redo_items.is_empty() {
                            ui.strong(app.i18n.t("history-redo-stack"));
                            for (idx, desc) in redo_items.iter().enumerate() {
                                ui.weak(format!("{}. {}", idx + 1, desc));
                            }
                        }
                    });
                }
            });

        if trigger_undo {
            app.action_undo();
        }
        if trigger_redo {
            app.action_redo();
        }
        if !hist_open {
            app.show_history_panel = false;
        }
    }
}
