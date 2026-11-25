use super::TimelineGeom;
use crate::app::StemApp;
use egui::{pos2, vec2, Color32, CornerRadius, Rect, Stroke};

pub(super) fn render_track_headers(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    geom: &TimelineGeom,
) -> Option<uuid::Uuid> {
    let available_size = geom.available_size;
    let header_width = geom.header_width;
    let ruler_height = geom.ruler_height;
    let track_heights = &geom.track_heights;
    let track_y_offsets = &geom.track_y_offsets;
    let num_tracks = track_heights.len();
    // ================= Left Column: Track Headers =================
    let (col_rect, _) =
        ui.allocate_exact_size(vec2(header_width, available_size.y), egui::Sense::hover());
    let tracks_top = col_rect.min.y + ruler_height;
    let header_clip_rect = Rect::from_min_max(
        pos2(col_rect.min.x, tracks_top),
        pos2(col_rect.max.x, col_rect.max.y),
    );

    // Fixed Top Header above tracks (ruler spacer)
    let top_hdr_rect = Rect::from_min_size(col_rect.min, vec2(header_width, ruler_height));
    ui.painter().rect_filled(
        top_hdr_rect,
        CornerRadius::ZERO,
        Color32::from_rgb(26, 28, 33),
    );
    ui.painter().line_segment(
        [
            pos2(top_hdr_rect.min.x, top_hdr_rect.max.y),
            pos2(top_hdr_rect.max.x, top_hdr_rect.max.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 60)),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(top_hdr_rect), |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(app.i18n.t_arg(
                    "timeline-tracks-header",
                    "count",
                    num_tracks.to_string(),
                ))
                .strong()
                .size(11.0)
                .color(Color32::LIGHT_GRAY),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui
                    .button(app.i18n.t("timeline-add-track"))
                    .on_hover_text(app.i18n.t("timeline-add-track-tooltip"))
                    .clicked()
                {
                    let default_name = app.i18n.t_arg(
                        "timeline-track-default-name",
                        "index",
                        (num_tracks + 1).to_string(),
                    );
                    app.action_add_track(default_name);
                }
            });
        });
    });

    // Draw continuous vertical border between Track Header column and Timeline Canvas
    ui.painter().line_segment(
        [
            pos2(col_rect.max.x, col_rect.min.y),
            pos2(col_rect.max.x, col_rect.max.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55)),
    );

    let mut delete_track_id: Option<uuid::Uuid> = None;

    // Scrolled track headers
    for t_idx in 0..num_tracks {
        let track = &app.project.tracks[t_idx];
        let track_id = track.id;
        let is_auto_expanded = app.timeline_state.expanded_automation_track_id == Some(track_id);
        let track_y = tracks_top - app.timeline_state.scroll_y + track_y_offsets[t_idx];
        let header_h = track_heights[t_idx];
        let rect = Rect::from_min_size(pos2(col_rect.min.x, track_y), vec2(header_width, header_h));

        // Cull if completely outside visible view
        if rect.max.y < tracks_top || rect.min.y > col_rect.max.y {
            continue;
        }

        let painter = ui.painter().with_clip_rect(header_clip_rect);
        let is_selected = app.timeline_state.selected_track_id == Some(track_id);
        let bg_color = if is_selected {
            Color32::from_rgb(38, 44, 58)
        } else {
            Color32::from_rgb(26, 28, 34)
        };
        painter.rect_filled(rect, CornerRadius::ZERO, bg_color);

        // Left track color stripe (4px)
        let track_colors = [
            Color32::from_rgb(60, 160, 240),  // blue
            Color32::from_rgb(50, 200, 140),  // emerald
            Color32::from_rgb(240, 160, 50),  // amber
            Color32::from_rgb(180, 100, 240), // purple
            Color32::from_rgb(240, 80, 100),  // red
            Color32::from_rgb(80, 220, 220),  // cyan
        ];
        let stripe_color = track_colors[t_idx % track_colors.len()];
        painter.rect_filled(
            Rect::from_min_size(pos2(rect.min.x, rect.min.y), vec2(4.0, header_h)),
            CornerRadius::ZERO,
            stripe_color,
        );

        // Live Per-Track Peak VU Meter (3px bar)
        let peak = app.telemetry.track_peaks.get(t_idx).copied().unwrap_or(0.0);
        let meter_bg = Rect::from_min_size(
            pos2(rect.min.x + 5.0, rect.min.y + 4.0),
            vec2(3.0, header_h - 8.0),
        );
        crate::views::render_vu_meter_bar(&painter, meter_bg, peak, 0.0, 1.0);

        // Bottom border matching timeline lane separator
        painter.line_segment(
            [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55)),
        );

        // Track Header Controls with top_down layout
        let inner_rect = Rect::from_min_max(
            pos2(rect.min.x + 8.0, rect.min.y + 4.0),
            pos2(rect.max.x - 6.0, rect.max.y - 4.0),
        );
        let ui_builder = egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .id_salt(format!("hdr_{}", track_id));
        ui.allocate_new_ui(ui_builder, |ui| {
            ui.set_clip_rect(header_clip_rect.intersect(inner_rect));
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);

            // Row 1: Delete button [x] + Track Name + Auto button
            ui.horizontal(|ui| {
                if ui.small_button("×").on_hover_text("Kanalı Sil").clicked() {
                    delete_track_id = Some(track_id);
                }
                let track_name = app.project.tracks[t_idx].name.clone();
                ui.label(egui::RichText::new(&track_name).strong().size(12.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let auto_active = is_auto_expanded;
                    let auto_color = if auto_active {
                        Color32::from_rgb(100, 220, 150)
                    } else {
                        Color32::GRAY
                    };
                    if ui
                        .selectable_label(
                            auto_active,
                            egui::RichText::new("Auto").size(10.0).color(auto_color),
                        )
                        .clicked()
                    {
                        if auto_active {
                            app.timeline_state.expanded_automation_track_id = None;
                        } else {
                            app.timeline_state.expanded_automation_track_id = Some(track_id);
                        }
                    }
                });
            });

            ui.add_space(2.0);

            // Row 2: Mute, Solo, Arm & Input Routing
            ui.horizontal(|ui| {
                // Mute button (M)
                let mut muted = app.project.tracks[t_idx].muted;
                let (mute_bg, mute_text) = if muted {
                    (Color32::from_rgb(220, 38, 38), Color32::WHITE)
                } else {
                    (
                        Color32::from_rgb(30, 34, 44),
                        Color32::from_rgb(148, 163, 184),
                    )
                };
                let mute_btn = egui::Button::new(
                    egui::RichText::new("M")
                        .strong()
                        .size(11.0)
                        .color(mute_text),
                )
                .fill(mute_bg)
                .min_size(vec2(24.0, 20.0))
                .corner_radius(CornerRadius::same(3));
                if ui
                    .add(mute_btn)
                    .on_hover_text("Sessize Al (Mute)")
                    .clicked()
                {
                    muted = !muted;
                    app.action_set_track_mute(t_idx, muted);
                }

                // Solo button (S)
                let mut soloed = app.project.tracks[t_idx].soloed;
                let (solo_bg, solo_text) = if soloed {
                    (
                        Color32::from_rgb(234, 179, 8),
                        Color32::from_rgb(20, 20, 20),
                    )
                } else {
                    (
                        Color32::from_rgb(30, 34, 44),
                        Color32::from_rgb(148, 163, 184),
                    )
                };
                let solo_btn = egui::Button::new(
                    egui::RichText::new("S")
                        .strong()
                        .size(11.0)
                        .color(solo_text),
                )
                .fill(solo_bg)
                .min_size(vec2(24.0, 20.0))
                .corner_radius(CornerRadius::same(3));
                if ui.add(solo_btn).on_hover_text("Solo Dinle").clicked() {
                    soloed = !soloed;
                    app.action_set_track_solo(t_idx, soloed);
                }

                // Arm button (R)
                let mut armed = app.project.tracks[t_idx].armed;
                let (arm_bg, arm_text) = if armed {
                    (Color32::from_rgb(225, 29, 72), Color32::WHITE)
                } else {
                    (
                        Color32::from_rgb(30, 34, 44),
                        Color32::from_rgb(148, 163, 184),
                    )
                };
                let arm_btn =
                    egui::Button::new(egui::RichText::new("R").strong().size(11.0).color(arm_text))
                        .fill(arm_bg)
                        .min_size(vec2(24.0, 20.0))
                        .corner_radius(CornerRadius::same(3));
                if ui.add(arm_btn).on_hover_text("Kayıt Hazır (Arm)").clicked() {
                    armed = !armed;
                    app.action_set_track_arm(t_idx, armed);
                }

                // Input Routing Selector
                let mut routing = app.project.tracks[t_idx].input_routing;
                let routing_label = match routing {
                    stem_core::project::TrackInputRouting::Mono1 => {
                        app.i18n.t("recording-input-mono1")
                    }
                    stem_core::project::TrackInputRouting::Mono2 => {
                        app.i18n.t("recording-input-mono2")
                    }
                    stem_core::project::TrackInputRouting::Stereo => {
                        app.i18n.t("recording-input-stereo")
                    }
                    stem_core::project::TrackInputRouting::None => {
                        app.i18n.t("recording-input-none")
                    }
                };
                egui::ComboBox::from_id_salt(format!("track_in_{}", track_id))
                    .selected_text(egui::RichText::new(routing_label).size(9.0))
                    .width(55.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut routing,
                            stem_core::project::TrackInputRouting::Mono1,
                            app.i18n.t("recording-input-mono1"),
                        );
                        ui.selectable_value(
                            &mut routing,
                            stem_core::project::TrackInputRouting::Mono2,
                            app.i18n.t("recording-input-mono2"),
                        );
                        ui.selectable_value(
                            &mut routing,
                            stem_core::project::TrackInputRouting::Stereo,
                            app.i18n.t("recording-input-stereo"),
                        );
                        ui.selectable_value(
                            &mut routing,
                            stem_core::project::TrackInputRouting::None,
                            app.i18n.t("recording-input-none"),
                        );
                    });
                app.project.tracks[t_idx].input_routing = routing;
            });

            ui.add_space(2.0);

            // Row 3: Volume Slider
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(app.i18n.t("track-header-volume"))
                        .size(10.0)
                        .color(Color32::LIGHT_GRAY),
                );
                let mut gain_db = app.project.tracks[t_idx].gain_db;
                if ui
                    .add_sized(
                        [105.0, 16.0],
                        egui::Slider::new(&mut gain_db, -48.0..=12.0).show_value(false),
                    )
                    .changed()
                {
                    app.action_set_track_gain(t_idx, gain_db);
                }
                ui.label(
                    egui::RichText::new(format!("{:.1}dB", gain_db))
                        .size(9.0)
                        .monospace()
                        .color(Color32::GRAY),
                );
            });

            // Row 4: Pan Slider
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(app.i18n.t("track-header-pan"))
                        .size(10.0)
                        .color(Color32::LIGHT_GRAY),
                );
                let mut pan = app.project.tracks[t_idx].pan;
                if ui
                    .add_sized(
                        [105.0, 16.0],
                        egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false),
                    )
                    .changed()
                {
                    app.action_set_track_pan(t_idx, pan);
                }
                let pan_text = if pan.abs() < 0.05 {
                    "C".to_string()
                } else if pan < 0.0 {
                    format!("L{:.0}", -pan * 100.0)
                } else {
                    format!("R{:.0}", pan * 100.0)
                };
                ui.label(
                    egui::RichText::new(pan_text)
                        .size(9.0)
                        .monospace()
                        .color(Color32::GRAY),
                );
            });

            // Expanded Automation Controls
            if is_auto_expanded {
                ui.separator();
                ui.horizontal(|ui| {
                    let mut target = app.timeline_state.active_automation_target;
                    let target_str = match target {
                        stem_core::project::AutomationTarget::Volume => {
                            app.i18n.t("automation-volume")
                        }
                        stem_core::project::AutomationTarget::Pan => app.i18n.t("automation-pan"),
                        stem_core::project::AutomationTarget::PluginParam {
                            param_index, ..
                        } => format!("FX #{}", param_index + 1),
                        stem_core::project::AutomationTarget::SendLevel(i) => {
                            format!("Send {}", i + 1)
                        }
                        stem_core::project::AutomationTarget::EqFreq(b) => {
                            format!("EQ{} Freq", b + 1)
                        }
                        stem_core::project::AutomationTarget::EqGain(b) => {
                            format!("EQ{} Gain", b + 1)
                        }
                        stem_core::project::AutomationTarget::EqQ(b) => {
                            format!("EQ{} Q", b + 1)
                        }
                        stem_core::project::AutomationTarget::CompressorThreshold => {
                            "Comp Thr".to_string()
                        }
                        stem_core::project::AutomationTarget::CompressorRatio => {
                            "Comp Rat".to_string()
                        }
                        stem_core::project::AutomationTarget::ReverbMix => "Rev Mix".to_string(),
                        stem_core::project::AutomationTarget::ReverbRoomSize => {
                            "Rev Size".to_string()
                        }
                    };
                    let send_count = app.project.tracks[t_idx].sends.len();
                    egui::ComboBox::from_id_salt(format!("auto_tgt_{}", track_id))
                        .selected_text(egui::RichText::new(target_str).size(9.0))
                        .width(72.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::Volume,
                                app.i18n.t("automation-volume"),
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::Pan,
                                app.i18n.t("automation-pan"),
                            );
                            for s_idx in 0..send_count {
                                ui.selectable_value(
                                    &mut target,
                                    stem_core::project::AutomationTarget::SendLevel(s_idx),
                                    format!("Send {}", s_idx + 1),
                                );
                            }
                            ui.separator();
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::EqFreq(0),
                                "EQ1 Freq",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::EqGain(0),
                                "EQ1 Gain",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::EqQ(0),
                                "EQ1 Q",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::CompressorThreshold,
                                "Comp Thresh",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::CompressorRatio,
                                "Comp Ratio",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::ReverbMix,
                                "Reverb Mix",
                            );
                            ui.selectable_value(
                                &mut target,
                                stem_core::project::AutomationTarget::ReverbRoomSize,
                                "Reverb Size",
                            );
                        });
                    if target != app.timeline_state.active_automation_target {
                        app.timeline_state.active_automation_target = target;
                        app.project.tracks[t_idx].ensure_automation_lane(target);
                    }

                    if let Some(lane) = app.project.tracks[t_idx]
                        .automation_lanes
                        .iter_mut()
                        .find(|l| l.target == target)
                    {
                        ui.checkbox(&mut lane.enabled, egui::RichText::new("On").size(9.0));
                    }
                });
                ui.horizontal(|ui| {
                    let curve_label = match app.timeline_state.automation_default_curve {
                        stem_core::project::AutomationCurve::Linear => "Lin",
                        stem_core::project::AutomationCurve::Bezier => "Bez",
                        stem_core::project::AutomationCurve::Step => "Stp",
                    };
                    if ui
                        .small_button(egui::RichText::new(curve_label).size(9.0))
                        .on_hover_text("Cycle default curve (Linear / Bezier / Step)")
                        .clicked()
                    {
                        app.timeline_state.automation_default_curve =
                            match app.timeline_state.automation_default_curve {
                                stem_core::project::AutomationCurve::Linear => {
                                    stem_core::project::AutomationCurve::Bezier
                                }
                                stem_core::project::AutomationCurve::Bezier => {
                                    stem_core::project::AutomationCurve::Step
                                }
                                stem_core::project::AutomationCurve::Step => {
                                    stem_core::project::AutomationCurve::Linear
                                }
                            };
                    }
                    let active_target = app.timeline_state.active_automation_target;
                    if ui
                        .small_button(egui::RichText::new("Copy").size(9.0))
                        .clicked()
                    {
                        if let Some(lane) = app.project.tracks[t_idx]
                            .automation_lanes
                            .iter()
                            .find(|l| l.target == active_target)
                        {
                            let (start_f, end_f) =
                                app.timeline_state.time_selection.unwrap_or((0, u64::MAX));
                            app.timeline_state.automation_clipboard =
                                lane.copy_range(start_f, end_f);
                        }
                    }
                    let can_paste = !app.timeline_state.automation_clipboard.is_empty();
                    if ui
                        .add_enabled(
                            can_paste,
                            egui::Button::new(egui::RichText::new("Paste").size(9.0)).small(),
                        )
                        .clicked()
                    {
                        let target_frame = app
                            .timeline_state
                            .time_selection
                            .map(|(s, _)| s)
                            .unwrap_or(app.telemetry.playhead_frame);
                        let clipboard = app.timeline_state.automation_clipboard.clone();
                        let lane = app.project.tracks[t_idx].ensure_automation_lane(active_target);
                        lane.paste_at(target_frame, &clipboard);
                        lane.enabled = true;
                        app.sync_timeline_to_audio_engine();
                    }
                });
            }
        });
    }

    delete_track_id
}
