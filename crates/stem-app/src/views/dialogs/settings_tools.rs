use crate::app::StemApp;

pub(super) fn render_keybindings_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

pub(super) fn render_recovery_dialog(app: &mut StemApp, ctx: &egui::Context) {
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

pub(super) fn render_plugin_browser_dialog(app: &mut StemApp, ctx: &egui::Context) {
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
            ui.colored_label(
                egui::Color32::from_rgb(245, 190, 90),
                app.i18n.t("plugin-passthrough-notice"),
            );
            ui.add_space(4.0);
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
                        let passthrough_note = app.i18n.t("plugin-slot-passthrough-warning");
                        let msg = match stem_audio::PluginHostInstance::load(&slot) {
                            Ok(_) => format!(
                                "Inserted '{}' on {} — {passthrough_note}",
                                slot.name, track.name
                            ),
                            Err(err) => format!(
                                "Inserted '{}' (warning: {err}) — {passthrough_note}",
                                slot.name
                            ),
                        };
                        track.plugins.push(slot);
                        app.plugin_browser_state.status_msg = Some(msg);
                        app.status_text = passthrough_note;
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

pub(super) fn render_stem_separation_dialog(app: &mut StemApp, ctx: &egui::Context) {
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
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("demucs-copy-cmd")).clicked() {
                        ui.ctx().copy_text("pip install demucs".to_string());
                    }
                    if ui.button(app.i18n.t("demucs-recheck")).clicked() {
                        app.stem_separation_state.demucs_available =
                            Some(stem_audio::DemucsRunner::is_available());
                    }
                });
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
                    .add_enabled(
                        available && !sources.is_empty(),
                        egui::Button::new("Separate Stems"),
                    )
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
