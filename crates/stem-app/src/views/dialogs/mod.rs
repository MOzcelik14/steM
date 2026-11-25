use crate::app::StemApp;
use stem_audio::{BitDepth, ExportFormat, ExportRange};
use stem_i18n::Locale;

mod markers_export;
mod processing;
mod settings_tools;

use markers_export::{
    render_batch_export_dialog, render_marker_list_dialog, render_named_marker_dialog,
};
use processing::{
    render_click_remover_dialog, render_loudness_report_dialog, render_noise_reduction_dialog,
};
use settings_tools::{render_keybindings_dialog, render_recovery_dialog};

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
                        let icon_rgba = include_bytes!("../../../../../assets/icon.rgba");
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
