//! Multi-track timeline view with smooth multi-resolution waveform visualization

use crate::app::StemApp;
use crate::state::DragOp;
use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_core::peaks::PeakPyramid;
use stem_core::project::Clip;

pub fn render_timeline_view(app: &mut StemApp, ui: &mut egui::Ui) {
    let available_size = ui.available_size();
    let header_width = 240.0;
    let ruler_height = 26.0;
    let scrollbar_h = 14.0_f32;

    let sample_rate = app.project.sample_rate;
    let pps = app.timeline_state.pixels_per_second;

    let canvas_width = (available_size.x - header_width).max(100.0);
    let track_spacing = 2.0_f32;
    let base_track_height = app.timeline_state.track_height.max(120.0);
    app.timeline_state.track_height = base_track_height;

    let num_tracks = app.project.tracks.len();
    let mut track_heights = Vec::with_capacity(num_tracks);
    let mut track_y_offsets = Vec::with_capacity(num_tracks);
    let mut total_tracks_h = 0.0_f32;

    for track in &app.project.tracks {
        let is_auto_expanded = app.timeline_state.expanded_automation_track_id == Some(track.id);
        let h = if is_auto_expanded {
            base_track_height + 55.0
        } else {
            base_track_height
        };
        track_heights.push(h);
        track_y_offsets.push(total_tracks_h);
        total_tracks_h += h + track_spacing;
    }

    let visible_tracks_h = (available_size.y - ruler_height - scrollbar_h).max(50.0);
    let max_scroll_y = (total_tracks_h - visible_tracks_h).max(0.0);

    let project_end_frame = app
        .project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .map(|c| c.timeline_end_frame())
        .max()
        .unwrap_or(0);
    let project_duration_sec = (project_end_frame as f32 / sample_rate as f32).max(120.0);
    let total_timeline_w = project_duration_sec * pps * 1.5;
    let max_scroll_x = (total_timeline_w - canvas_width).max(0.0);

    // Handle horizontal / vertical mouse wheel zooming and scrolling
    let scroll_delta = ui.input(|i| {
        let s = i.smooth_scroll_delta;
        let r = i.raw_scroll_delta;
        if s != egui::Vec2::ZERO {
            s
        } else {
            r
        }
    });
    let zoom_delta = ui.input(|i| i.zoom_delta());

    if ui.input(|i| i.modifiers.ctrl) && (scroll_delta.y != 0.0 || zoom_delta != 1.0) {
        let factor = if zoom_delta != 1.0 {
            zoom_delta
        } else if scroll_delta.y > 0.0 {
            1.15
        } else {
            0.85
        };
        app.timeline_state.pixels_per_second = (pps * factor).clamp(5.0, 1000.0);
    } else if scroll_delta.x != 0.0 || (ui.input(|i| i.modifiers.shift) && scroll_delta.y != 0.0) {
        let dx = if scroll_delta.x != 0.0 {
            scroll_delta.x
        } else {
            scroll_delta.y
        };
        app.timeline_state.scroll_x = (app.timeline_state.scroll_x - dx).clamp(0.0, max_scroll_x);
    } else if scroll_delta.y != 0.0 {
        app.timeline_state.scroll_y =
            (app.timeline_state.scroll_y - scroll_delta.y).clamp(0.0, max_scroll_y);
    }

    // Middle mouse button drag panning (horizontal and vertical)
    if ui.input(|i| i.pointer.middle_down()) {
        let delta = ui.input(|i| i.pointer.delta());
        if delta != egui::Vec2::ZERO {
            app.timeline_state.scroll_x =
                (app.timeline_state.scroll_x - delta.x).clamp(0.0, max_scroll_x);
            app.timeline_state.scroll_y =
                (app.timeline_state.scroll_y - delta.y).clamp(0.0, max_scroll_y);
        }
    }

    ui.horizontal(|ui| {
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
        ui.painter()
            .rect_filled(top_hdr_rect, Rounding::ZERO, Color32::from_rgb(26, 28, 33));
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
                    egui::RichText::new(format!("Tracks ({})", num_tracks))
                        .strong()
                        .size(11.0)
                        .color(Color32::LIGHT_GRAY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui
                        .button("+ Add Track")
                        .on_hover_text("Add a new audio track")
                        .clicked()
                    {
                        app.action_add_track(format!("Track {}", num_tracks + 1));
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

        // Scrolled track headers
        for t_idx in 0..num_tracks {
            let track = &app.project.tracks[t_idx];
            let track_id = track.id;
            let is_auto_expanded =
                app.timeline_state.expanded_automation_track_id == Some(track_id);
            let track_y = tracks_top - app.timeline_state.scroll_y + track_y_offsets[t_idx];
            let header_h = track_heights[t_idx];
            let rect =
                Rect::from_min_size(pos2(col_rect.min.x, track_y), vec2(header_width, header_h));

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
            painter.rect_filled(rect, Rounding::ZERO, bg_color);

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
                Rounding::ZERO,
                stripe_color,
            );

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
                        app.action_delete_track(track_id);
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
                    .rounding(Rounding::same(3.0));
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
                    .rounding(Rounding::same(3.0));
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
                    let arm_btn = egui::Button::new(
                        egui::RichText::new("R").strong().size(11.0).color(arm_text),
                    )
                    .fill(arm_bg)
                    .min_size(vec2(24.0, 20.0))
                    .rounding(Rounding::same(3.0));
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
                            stem_core::project::AutomationTarget::Pan => {
                                app.i18n.t("automation-pan")
                            }
                        };
                        egui::ComboBox::from_id_salt(format!("auto_tgt_{}", track_id))
                            .selected_text(egui::RichText::new(target_str).size(9.0))
                            .width(65.0)
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
                            });
                        app.timeline_state.active_automation_target = target;

                        if let Some(lane) = app.project.tracks[t_idx]
                            .automation_lanes
                            .iter_mut()
                            .find(|l| l.target == target)
                        {
                            ui.checkbox(&mut lane.enabled, egui::RichText::new("On").size(9.0));
                        }
                    });
                }
            });
        }

        // ================= Right Column: Timeline Canvas =================
        let canvas_width = (available_size.x - header_width).max(100.0);
        let canvas_size = vec2(canvas_width, available_size.y);

        let (canvas_rect, response) =
            ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
        let painter = ui.painter().with_clip_rect(canvas_rect);

        // Background
        painter.rect_filled(canvas_rect, Rounding::ZERO, Color32::from_rgb(20, 22, 26));

        // ---------------- Timeline Ruler ----------------
        let ruler_rect =
            Rect::from_min_size(canvas_rect.min, vec2(canvas_rect.width(), ruler_height));
        painter.rect_filled(ruler_rect, Rounding::ZERO, Color32::from_rgb(28, 30, 36));
        painter.line_segment(
            [
                pos2(ruler_rect.min.x, ruler_rect.max.y),
                pos2(ruler_rect.max.x, ruler_rect.max.y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 60)),
        );

        // Draw time tick marks with dynamic step sizing (no label overlap)
        let scroll_x = app.timeline_state.scroll_x;
        let start_time_sec = (scroll_x / pps).max(0.0);
        let visible_duration = canvas_width / pps;
        let end_time_sec = start_time_sec + visible_duration;

        // Choose step size dynamically so labels have at least 70px spacing
        let min_label_width_px = 70.0_f32;
        let ideal_step_sec = (min_label_width_px / pps) as f64;
        let standard_steps: &[f64] = &[
            0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0,
            1800.0, 3600.0,
        ];
        let step_sec = standard_steps
            .iter()
            .copied()
            .find(|&s| s >= ideal_step_sec)
            .unwrap_or(3600.0);

        let first_tick = (start_time_sec as f64 / step_sec).floor() * step_sec;
        let mut t = first_tick;
        while t <= (end_time_sec as f64) + step_sec {
            let x = canvas_rect.min.x + (t as f32 * pps) - scroll_x;
            if x >= canvas_rect.min.x && x <= canvas_rect.max.x {
                painter.line_segment(
                    [pos2(x, ruler_rect.max.y - 8.0), pos2(x, ruler_rect.max.y)],
                    Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 130)),
                );

                // Format ruler text cleanly like Audacity
                let label = if step_sec >= 60.0 {
                    let total_secs = t.round() as i64;
                    let mins = total_secs / 60;
                    let secs = (total_secs % 60).abs();
                    format!("{}:{:02}", mins, secs)
                } else if step_sec >= 1.0 {
                    let total_secs = t.round() as i64;
                    let mins = total_secs / 60;
                    let secs = (total_secs % 60).abs();
                    if mins > 0 {
                        format!("{}:{:02}", mins, secs)
                    } else {
                        format!("{}s", secs)
                    }
                } else {
                    let total_secs = t.floor() as i64;
                    let frac = ((t - t.floor()) * 10.0).round() as i64;
                    let mins = total_secs / 60;
                    let secs = (total_secs % 60).abs();
                    if mins > 0 {
                        format!("{}:{:02}.{}", mins, secs, frac)
                    } else {
                        format!("{}.{}s", secs, frac)
                    }
                };

                painter.text(
                    pos2(x + 4.0, ruler_rect.min.y + 3.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::monospace(10.5),
                    Color32::from_rgb(190, 195, 205),
                );
            }
            t += step_sec;
        }

        // ---------------- Track Waveform Lanes ----------------
        let tracks_top = canvas_rect.min.y + ruler_height;
        let tracks_clip_rect = Rect::from_min_max(
            pos2(canvas_rect.min.x, tracks_top),
            pos2(canvas_rect.max.x, canvas_rect.max.y - scrollbar_h),
        );
        let lane_painter = ui.painter().with_clip_rect(tracks_clip_rect);

        let mouse_pos = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
            .or_else(|| ui.input(|i| i.pointer.latest_pos()));
        let is_mouse_down = ui.input(|i| i.pointer.primary_down());
        let is_primary_clicked = ui.input(|i| i.pointer.primary_clicked());
        let is_secondary_clicked = ui.input(|i| i.pointer.secondary_clicked());
        let mut open_clip_in_waveform: Option<uuid::Uuid> = None;
        let mut timeline_needs_sync = false;

        for t_idx in 0..app.project.tracks.len() {
            let track_id = app.project.tracks[t_idx].id;
            let is_auto_expanded =
                app.timeline_state.expanded_automation_track_id == Some(track_id);
            let track_top = tracks_top - app.timeline_state.scroll_y + track_y_offsets[t_idx];
            let track_h = track_heights[t_idx];
            let lane_h = if is_auto_expanded {
                track_h - 50.0
            } else {
                track_h
            };
            let track_rect = Rect::from_min_size(
                pos2(canvas_rect.min.x, track_top),
                vec2(canvas_rect.width(), track_h),
            );

            // Cull if track is completely outside the visible viewport
            if track_rect.max.y < tracks_top || track_rect.min.y > canvas_rect.max.y - scrollbar_h {
                continue;
            }

            // Alternate track background
            let track_bg = if t_idx % 2 == 0 {
                Color32::from_rgb(24, 26, 30)
            } else {
                Color32::from_rgb(28, 30, 35)
            };
            lane_painter.rect_filled(track_rect, Rounding::ZERO, track_bg);
            lane_painter.line_segment(
                [
                    pos2(track_rect.min.x, track_rect.max.y),
                    pos2(track_rect.max.x, track_rect.max.y),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(40, 42, 48)),
            );

            // Render Clips in this track
            let num_clips = app.project.tracks[t_idx].clips.len();
            for clip_idx in 0..num_clips {
                let clip = &app.project.tracks[t_idx].clips[clip_idx];
                let clip_x_start = canvas_rect.min.x
                    + (clip.track_offset_frames as f32 / sample_rate as f32 * pps)
                    - scroll_x;
                let clip_width = (clip.length_frames as f32 / sample_rate as f32 * pps).max(4.0);
                let clip_rect = Rect::from_min_size(
                    pos2(clip_x_start, track_top + 4.0),
                    vec2(clip_width, lane_h - 8.0),
                );

                // Check crossfade with next clip on the track
                if clip_idx + 1 < num_clips {
                    let next_clip = &app.project.tracks[t_idx].clips[clip_idx + 1];
                    let clip_end = clip.timeline_end_frame();
                    if next_clip.track_offset_frames < clip_end {
                        let xf_start = next_clip.track_offset_frames;
                        let xf_end = clip_end;
                        let x1 = canvas_rect.min.x + (xf_start as f32 / sample_rate as f32 * pps)
                            - scroll_x;
                        let x2 = canvas_rect.min.x + (xf_end as f32 / sample_rate as f32 * pps)
                            - scroll_x;
                        let xf_rect = Rect::from_min_max(
                            pos2(x1.max(canvas_rect.min.x), track_top + 4.0),
                            pos2(x2.min(canvas_rect.max.x), track_top + lane_h - 4.0),
                        );
                        if xf_rect.width() > 2.0 {
                            // Crossfade region overlay
                            lane_painter.rect_filled(
                                xf_rect,
                                Rounding::ZERO,
                                Color32::from_rgba_premultiplied(40, 150, 220, 50),
                            );
                            lane_painter.line_segment(
                                [
                                    pos2(xf_rect.min.x, xf_rect.min.y),
                                    pos2(xf_rect.max.x, xf_rect.max.y),
                                ],
                                Stroke::new(1.5_f32, Color32::from_rgb(100, 220, 255)),
                            );
                            lane_painter.line_segment(
                                [
                                    pos2(xf_rect.min.x, xf_rect.max.y),
                                    pos2(xf_rect.max.x, xf_rect.min.y),
                                ],
                                Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 80)),
                            );

                            let xf_label = match next_clip.crossfade_type {
                                stem_core::project::CrossfadeType::EqualPower => "✕ EqPower",
                                stem_core::project::CrossfadeType::Linear => "✕ Linear",
                                stem_core::project::CrossfadeType::Exponential => "✕ Exp",
                            };
                            lane_painter.text(
                                pos2(xf_rect.center().x, xf_rect.min.y + 4.0),
                                egui::Align2::CENTER_TOP,
                                xf_label,
                                egui::FontId::monospace(9.0),
                                Color32::from_rgb(220, 240, 255),
                            );
                        }
                    }
                }

                // Only render if visible within canvas bounds
                if clip_rect.max.x < canvas_rect.min.x
                    || clip_rect.min.x > canvas_rect.max.x
                    || clip_rect.max.y < tracks_top
                    || clip_rect.min.y > canvas_rect.max.y - scrollbar_h
                {
                    continue;
                }

                let is_clip_selected = app.timeline_state.selected_clip_id == Some(clip.id);

                // Clip box background
                let clip_bg = if is_clip_selected {
                    Color32::from_rgb(32, 54, 84)
                } else {
                    Color32::from_rgb(24, 38, 56)
                };
                lane_painter.rect_filled(clip_rect, Rounding::same(4.0), clip_bg);

                // Clip border stroke
                let stroke_color = if is_clip_selected {
                    Color32::from_rgb(56, 189, 248)
                } else {
                    Color32::from_rgb(50, 75, 105)
                };
                let stroke_width = if is_clip_selected { 1.5_f32 } else { 1.0_f32 };
                lane_painter.rect_stroke(
                    clip_rect,
                    Rounding::same(4.0),
                    Stroke::new(stroke_width, stroke_color),
                );

                // Clip titlebar header strip
                let titlebar_h = 16.0_f32.min(clip_rect.height());
                let titlebar_rect =
                    Rect::from_min_size(clip_rect.min, vec2(clip_rect.width(), titlebar_h));
                lane_painter.rect_filled(
                    titlebar_rect,
                    Rounding {
                        nw: 4.0,
                        ne: 4.0,
                        sw: 0.0,
                        se: 0.0,
                    },
                    Color32::from_rgba_premultiplied(14, 18, 26, 200),
                );
                lane_painter.line_segment(
                    [
                        pos2(clip_rect.min.x, clip_rect.min.y + titlebar_h),
                        pos2(clip_rect.max.x, clip_rect.min.y + titlebar_h),
                    ],
                    Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 20)),
                );

                // Clip title text inside titlebar
                lane_painter.text(
                    clip_rect.min + vec2(6.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    &clip.name,
                    egui::FontId::proportional(10.5),
                    if is_clip_selected {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(226, 232, 240)
                    },
                );

                // Draw Waveform using multi-resolution PeakPyramid below titlebar
                if let Some(pyramid) = app.peak_pyramids.get(&clip.source_id) {
                    let wave_rect = Rect::from_min_max(
                        pos2(clip_rect.min.x, clip_rect.min.y + titlebar_h),
                        clip_rect.max,
                    );
                    render_clip_waveform(
                        &lane_painter,
                        wave_rect,
                        clip,
                        pyramid,
                        sample_rate,
                        is_clip_selected,
                    );
                }

                // Render Fade In & Fade Out curves
                let fade_in_w =
                    (clip.fade_in_frames as f32 / sample_rate as f32 * pps).min(clip_rect.width());
                let fade_out_w =
                    (clip.fade_out_frames as f32 / sample_rate as f32 * pps).min(clip_rect.width());

                if clip.fade_in_frames > 0 {
                    lane_painter.line_segment(
                        [
                            pos2(clip_rect.min.x, clip_rect.max.y),
                            pos2(clip_rect.min.x + fade_in_w, clip_rect.min.y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(255, 200, 80)),
                    );
                }
                if clip.fade_out_frames > 0 {
                    lane_painter.line_segment(
                        [
                            pos2(clip_rect.max.x - fade_out_w, clip_rect.min.y),
                            pos2(clip_rect.max.x, clip_rect.max.y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(255, 200, 80)),
                    );
                }

                // Visual handles and hit-testing for Trim and Fades
                let fade_size = 14.0_f32;
                let max_fade_x = (clip_rect.max.x - fade_size).max(clip_rect.min.x);
                let fade_in_handle_x = (clip_rect.min.x + fade_in_w - fade_size * 0.5)
                    .clamp(clip_rect.min.x, max_fade_x);
                let fade_in_handle_rect = Rect::from_min_size(
                    pos2(fade_in_handle_x, clip_rect.min.y),
                    vec2(fade_size, fade_size),
                );

                let fade_out_handle_x = (clip_rect.max.x - fade_out_w - fade_size * 0.5)
                    .clamp(clip_rect.min.x, max_fade_x);
                let fade_out_handle_rect = Rect::from_min_size(
                    pos2(fade_out_handle_x, clip_rect.min.y),
                    vec2(fade_size, fade_size),
                );

                let trim_w = 8.0_f32;
                let left_handle_rect = Rect::from_min_size(
                    pos2(clip_rect.min.x, clip_rect.min.y + fade_size),
                    vec2(trim_w, (clip_rect.height() - fade_size).max(4.0)),
                );
                let right_handle_rect = Rect::from_min_size(
                    pos2(clip_rect.max.x - trim_w, clip_rect.min.y + fade_size),
                    vec2(trim_w, (clip_rect.height() - fade_size).max(4.0)),
                );

                let can_draw_fade_handles = clip_rect.width() >= fade_size * 2.0;

                let is_left_trim_hovered =
                    mouse_pos.is_some_and(|mp| left_handle_rect.contains(mp));
                let is_right_trim_hovered =
                    mouse_pos.is_some_and(|mp| right_handle_rect.contains(mp));
                let is_fade_in_hovered = can_draw_fade_handles
                    && mouse_pos.is_some_and(|mp| fade_in_handle_rect.contains(mp));
                let is_fade_out_hovered = can_draw_fade_handles
                    && mouse_pos.is_some_and(|mp| fade_out_handle_rect.contains(mp));

                if can_draw_fade_handles {
                    let fade_in_color = if is_fade_in_hovered {
                        Color32::from_rgb(255, 230, 90)
                    } else {
                        Color32::from_rgba_premultiplied(255, 190, 60, 180)
                    };
                    let fade_out_color = if is_fade_out_hovered {
                        Color32::from_rgb(255, 230, 90)
                    } else {
                        Color32::from_rgba_premultiplied(255, 190, 60, 180)
                    };

                    lane_painter.rect_filled(
                        fade_in_handle_rect,
                        Rounding::same(2.0),
                        fade_in_color,
                    );
                    lane_painter.rect_filled(
                        fade_out_handle_rect,
                        Rounding::same(2.0),
                        fade_out_color,
                    );
                }

                // Highlight trim edge on hover
                if is_left_trim_hovered {
                    lane_painter.rect_filled(
                        left_handle_rect,
                        Rounding::same(2.0),
                        Color32::from_rgba_premultiplied(120, 200, 255, 160),
                    );
                }
                if is_right_trim_hovered {
                    lane_painter.rect_filled(
                        right_handle_rect,
                        Rounding::same(2.0),
                        Color32::from_rgba_premultiplied(120, 200, 255, 160),
                    );
                }

                // Check for click / selection / drag start
                if let Some(mp) = mouse_pos {
                    if clip_rect.contains(mp) && app.timeline_state.drag_op.is_none() {
                        let primary_pressed =
                            ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                        let primary_double = ui.input(|i| {
                            i.pointer
                                .button_double_clicked(egui::PointerButton::Primary)
                        });

                        // Prioritize edge trim handles over fade handles
                        if is_left_trim_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if primary_pressed {
                                app.timeline_state.drag_op = Some(DragOp::TrimLeft {
                                    clip_id: clip.id,
                                    track_id,
                                    orig_source_start: clip.source_start_frame,
                                    orig_track_offset: clip.track_offset_frames,
                                    orig_length: clip.length_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if is_right_trim_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if primary_pressed {
                                app.timeline_state.drag_op = Some(DragOp::TrimRight {
                                    clip_id: clip.id,
                                    track_id,
                                    orig_length: clip.length_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if is_fade_in_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            if primary_pressed {
                                app.timeline_state.drag_op = Some(DragOp::FadeIn {
                                    clip_id: clip.id,
                                    track_id,
                                    orig_fade_in: clip.fade_in_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if is_fade_out_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            if primary_pressed {
                                app.timeline_state.drag_op = Some(DragOp::FadeOut {
                                    clip_id: clip.id,
                                    track_id,
                                    orig_fade_out: clip.fade_out_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if primary_double {
                            open_clip_in_waveform = Some(clip.id);
                        } else if primary_pressed {
                            if ui.input(|i| i.modifiers.shift) {
                                let frame_at_mouse =
                                    ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                                        * sample_rate as f32)
                                        .round() as u64;
                                app.timeline_state.drag_op = Some(DragOp::SelectTimeRange {
                                    anchor_frame: frame_at_mouse,
                                });
                                app.timeline_state.time_selection =
                                    Some((frame_at_mouse, frame_at_mouse));
                                app.action_seek(frame_at_mouse);
                            } else {
                                app.timeline_state.selected_clip_id = Some(clip.id);
                                app.timeline_state.selected_track_id = Some(track_id);

                                // Start Move Clip drag operation
                                app.timeline_state.drag_op = Some(DragOp::MoveClip {
                                    clip_id: clip.id,
                                    track_id,
                                    original_track_offset: clip.track_offset_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        }
                    }
                }
            }

            // Render Automation Lane if expanded
            if is_auto_expanded {
                let lane_rect = Rect::from_min_size(
                    pos2(canvas_rect.min.x, track_top + lane_h),
                    vec2(canvas_width, 50.0),
                );
                let active_target = app.timeline_state.active_automation_target;
                if let Some(lane) = app.project.tracks[t_idx]
                    .automation_lanes
                    .iter_mut()
                    .find(|l| l.target == active_target)
                {
                    let ctx = AutomationLaneRenderCtx {
                        lane_rect,
                        target: active_target,
                        sample_rate,
                        pps,
                        scroll_x,
                        mouse_pos,
                        is_mouse_down,
                        is_primary_clicked,
                        is_secondary_clicked,
                        drag_point: &mut app.timeline_state.drag_automation_point,
                        track_id,
                    };
                    if render_automation_lane(&lane_painter, lane, ctx) {
                        timeline_needs_sync = true;
                    }
                }
            }
        }

        if !is_mouse_down {
            app.timeline_state.drag_automation_point = None;
        }
        if timeline_needs_sync {
            app.sync_timeline_to_audio_engine();
        }

        if let Some(clip_id) = open_clip_in_waveform {
            app.action_open_clip_in_waveform_editor(clip_id);
            return;
        }

        // ---------------- Canvas Click / Range Selection / Selection Edge Drag ----------------
        let primary_pressed = ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if let Some(mp) = mouse_pos {
            let in_track_area = mp.x >= canvas_rect.min.x
                && mp.x <= canvas_rect.max.x
                && mp.y >= tracks_top
                && mp.y <= canvas_rect.max.y - scrollbar_h;

            if in_track_area {
                let frame_at_mouse = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                    * sample_rate as f32)
                    .round() as u64;

                // 1. Check if hovering / clicking existing selection edges
                if let Some((s_frame, e_frame)) = app.timeline_state.time_selection {
                    let s_x =
                        canvas_rect.min.x + (s_frame as f32 / sample_rate as f32 * pps) - scroll_x;
                    let e_x =
                        canvas_rect.min.x + (e_frame as f32 / sample_rate as f32 * pps) - scroll_x;
                    let left_x = s_x.min(e_x);
                    let right_x = s_x.max(e_x);

                    if (mp.x - left_x).abs() <= 6.0 {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        if primary_pressed && app.timeline_state.drag_op.is_none() {
                            app.timeline_state.drag_op = Some(DragOp::DragSelectionLeft);
                        }
                    } else if (mp.x - right_x).abs() <= 6.0 {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        if primary_pressed && app.timeline_state.drag_op.is_none() {
                            app.timeline_state.drag_op = Some(DragOp::DragSelectionRight);
                        }
                    }
                }

                // 2. Click on empty space: start time range selection & seek playhead
                if primary_pressed && app.timeline_state.drag_op.is_none() {
                    app.timeline_state.selected_clip_id = None;
                    app.timeline_state.drag_op = Some(DragOp::SelectTimeRange {
                        anchor_frame: frame_at_mouse,
                    });
                    app.timeline_state.time_selection = Some((frame_at_mouse, frame_at_mouse));
                    app.action_seek(frame_at_mouse);
                }
            }
        }

        // ---------------- Musical Grid Lines Overlay ----------------
        let tempo = app.project.tempo_bpm.max(20.0);
        let beats_per_bar = app.project.time_signature.0.max(1) as f32;
        let sec_per_bar = (60.0 / tempo as f32) * beats_per_bar;

        let first_bar = (start_time_sec / sec_per_bar).floor() as u64;
        let last_bar = (end_time_sec / sec_per_bar).ceil() as u64;

        for bar in first_bar..=last_bar {
            let bar_sec = bar as f32 * sec_per_bar;
            let bar_x = canvas_rect.min.x + (bar_sec * pps) - scroll_x;
            if bar_x >= canvas_rect.min.x && bar_x <= canvas_rect.max.x {
                painter.line_segment(
                    [pos2(bar_x, tracks_top), pos2(bar_x, canvas_rect.max.y)],
                    Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(80, 90, 110, 45)),
                );
            }
        }

        // ---------------- Punch-In Region Rendering ----------------
        if app.project.punch_in_enabled {
            if let Some((punch_in, punch_out)) = app.project.punch_region {
                let in_x =
                    canvas_rect.min.x + (punch_in as f32 / sample_rate as f32 * pps) - scroll_x;
                let out_x =
                    canvas_rect.min.x + (punch_out as f32 / sample_rate as f32 * pps) - scroll_x;

                let punch_rect = Rect::from_min_max(
                    pos2(in_x.max(canvas_rect.min.x), canvas_rect.min.y),
                    pos2(out_x.min(canvas_rect.max.x), canvas_rect.max.y),
                );

                if punch_rect.width() > 0.0 {
                    painter.rect_filled(
                        punch_rect,
                        Rounding::ZERO,
                        Color32::from_rgba_premultiplied(255, 90, 40, 35),
                    );
                }

                // Punch In line and flag
                if in_x >= canvas_rect.min.x && in_x <= canvas_rect.max.x {
                    painter.line_segment(
                        [pos2(in_x, canvas_rect.min.y), pos2(in_x, canvas_rect.max.y)],
                        Stroke::new(1.5_f32, Color32::from_rgb(255, 120, 50)),
                    );
                    painter.text(
                        pos2(in_x + 3.0, ruler_rect.min.y + 1.0),
                        egui::Align2::LEFT_TOP,
                        "▼ PUNCH IN",
                        egui::FontId::monospace(9.0),
                        Color32::from_rgb(255, 140, 60),
                    );
                }

                // Punch Out line and flag
                if out_x >= canvas_rect.min.x && out_x <= canvas_rect.max.x {
                    painter.line_segment(
                        [
                            pos2(out_x, canvas_rect.min.y),
                            pos2(out_x, canvas_rect.max.y),
                        ],
                        Stroke::new(1.5_f32, Color32::from_rgb(255, 120, 50)),
                    );
                    painter.text(
                        pos2(out_x - 3.0, ruler_rect.min.y + 1.0),
                        egui::Align2::RIGHT_TOP,
                        "▲ PUNCH OUT",
                        egui::FontId::monospace(9.0),
                        Color32::from_rgb(255, 140, 60),
                    );
                }
            }
        }

        // ---------------- Timeline Markers Rendering ----------------
        let mut clicked_marker = None;
        let mut double_clicked_marker = None;

        for marker in &app.project.markers {
            let m_x =
                canvas_rect.min.x + (marker.frame as f32 / sample_rate as f32 * pps) - scroll_x;
            if m_x >= canvas_rect.min.x && m_x <= canvas_rect.max.x {
                let marker_color = if marker.is_cd_track {
                    Color32::from_rgb(255, 200, 60)
                } else {
                    Color32::from_rgb(marker.color_rgb.0, marker.color_rgb.1, marker.color_rgb.2)
                };

                // Vertical marker line across all track lanes
                painter.line_segment(
                    [pos2(m_x, ruler_rect.max.y), pos2(m_x, canvas_rect.max.y)],
                    Stroke::new(1.0_f32, marker_color.linear_multiply(0.6)),
                );

                // Marker flag shape on ruler
                let flag_pts = [
                    pos2(m_x, ruler_rect.min.y + 1.0),
                    pos2(m_x + 12.0, ruler_rect.min.y + 1.0),
                    pos2(m_x + 12.0, ruler_rect.min.y + 13.0),
                    pos2(m_x + 4.0, ruler_rect.min.y + 13.0),
                    pos2(m_x, ruler_rect.max.y),
                ];
                painter.add(egui::Shape::convex_polygon(
                    flag_pts.to_vec(),
                    marker_color,
                    Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 25)),
                ));

                // Marker name label on ruler
                let label_text = if marker.is_cd_track {
                    format!("[CD] {}", marker.name)
                } else {
                    marker.name.clone()
                };
                painter.text(
                    pos2(m_x + 14.0, ruler_rect.min.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    label_text,
                    egui::FontId::proportional(11.0),
                    marker_color,
                );

                // Interactive click/double-click on marker flag
                let hit_rect = Rect::from_min_size(
                    pos2(m_x - 3.0, ruler_rect.min.y),
                    vec2(40.0, ruler_height),
                );
                if let Some(mp) = mouse_pos {
                    if hit_rect.contains(mp) {
                        if response.double_clicked() {
                            double_clicked_marker = Some(marker.clone());
                        } else if is_primary_clicked {
                            clicked_marker = Some(marker.frame);
                        }
                    }
                }
            }
        }

        if let Some(frame) = clicked_marker {
            app.action_seek(frame);
        }
        if let Some(m) = double_clicked_marker {
            app.named_marker_state.open_for_edit(&m);
        }

        // ---------------- Time Range Selection Overlay ----------------
        if let Some((s_frame, e_frame)) = app.timeline_state.time_selection {
            let s_x = canvas_rect.min.x + (s_frame as f32 / sample_rate as f32 * pps) - scroll_x;
            let e_x = canvas_rect.min.x + (e_frame as f32 / sample_rate as f32 * pps) - scroll_x;
            let left_x = s_x.min(e_x);
            let right_x = s_x.max(e_x);

            let sel_rect = Rect::from_min_max(
                pos2(left_x.max(canvas_rect.min.x), tracks_top),
                pos2(
                    right_x.min(canvas_rect.max.x),
                    canvas_rect.max.y - scrollbar_h,
                ),
            );

            if sel_rect.width() > 0.0 {
                // Shaded selection box across all tracks (Audacity blue tint)
                painter.rect_filled(
                    sel_rect,
                    Rounding::ZERO,
                    Color32::from_rgba_premultiplied(70, 150, 255, 45),
                );

                // Left boundary line
                if left_x >= canvas_rect.min.x && left_x <= canvas_rect.max.x {
                    painter.line_segment(
                        [
                            pos2(left_x, tracks_top),
                            pos2(left_x, canvas_rect.max.y - scrollbar_h),
                        ],
                        Stroke::new(1.5_f32, Color32::from_rgb(100, 190, 255)),
                    );
                }

                // Right boundary line
                if right_x >= canvas_rect.min.x && right_x <= canvas_rect.max.x {
                    painter.line_segment(
                        [
                            pos2(right_x, tracks_top),
                            pos2(right_x, canvas_rect.max.y - scrollbar_h),
                        ],
                        Stroke::new(1.5_f32, Color32::from_rgb(100, 190, 255)),
                    );
                }

                // Ruler selection highlight
                let ruler_sel = Rect::from_min_max(
                    pos2(left_x.max(canvas_rect.min.x), ruler_rect.min.y),
                    pos2(right_x.min(canvas_rect.max.x), ruler_rect.max.y),
                );
                painter.rect_filled(
                    ruler_sel,
                    Rounding::ZERO,
                    Color32::from_rgba_premultiplied(100, 190, 255, 65),
                );
            }
        }

        // ---------------- Playhead Rendering ----------------
        let playhead_frame = app.telemetry.playhead_frame;
        let playhead_x =
            canvas_rect.min.x + (playhead_frame as f32 / sample_rate as f32 * pps) - scroll_x;

        if playhead_x >= canvas_rect.min.x && playhead_x <= canvas_rect.max.x {
            // Vertical playhead line
            painter.line_segment(
                [
                    pos2(playhead_x, canvas_rect.min.y),
                    pos2(playhead_x, canvas_rect.max.y),
                ],
                Stroke::new(2.0_f32, Color32::from_rgb(255, 80, 80)),
            );

            // Playhead ruler cursor handle
            let head_pts = [
                pos2(playhead_x - 6.0, ruler_rect.min.y),
                pos2(playhead_x + 6.0, ruler_rect.min.y),
                pos2(playhead_x + 6.0, ruler_rect.min.y + 12.0),
                pos2(playhead_x, ruler_rect.max.y),
                pos2(playhead_x - 6.0, ruler_rect.min.y + 12.0),
            ];
            painter.add(egui::Shape::convex_polygon(
                head_pts.to_vec(),
                Color32::from_rgb(255, 80, 80),
                Stroke::NONE,
            ));
        }

        // ---------------- Count-in HUD Countdown Overlay ----------------
        if app.recording_state.count_in_active {
            let center = canvas_rect.center();
            let hud_rect = Rect::from_center_size(center, vec2(240.0, 70.0));
            painter.rect_filled(
                hud_rect,
                Rounding::same(8.0),
                Color32::from_rgba_premultiplied(18, 20, 26, 230),
            );
            painter.rect_stroke(
                hud_rect,
                Rounding::same(8.0),
                Stroke::new(2.0_f32, Color32::from_rgb(255, 170, 40)),
            );

            let count_num = (app.recording_state.count_in_seconds_remaining.ceil() as u32).max(1);
            painter.text(
                pos2(center.x, center.y - 12.0),
                egui::Align2::CENTER_CENTER,
                format!("COUNT-IN: {count_num}"),
                egui::FontId::proportional(22.0),
                Color32::from_rgb(255, 200, 70),
            );

            let progress = (app.recording_state.count_in_seconds_remaining
                / app.recording_state.count_in_total_seconds.max(0.1))
            .clamp(0.0, 1.0);
            let bar_rect = Rect::from_center_size(
                pos2(center.x, center.y + 16.0),
                vec2(200.0 * progress, 6.0),
            );
            painter.rect_filled(
                bar_rect,
                Rounding::same(3.0),
                Color32::from_rgb(255, 140, 30),
            );
        }

        // ---------------- Interactive Playhead Scrubbing / Seeking ----------------
        if let Some(mp) = mouse_pos {
            if ruler_rect.contains(mp) && is_mouse_down {
                let target_sec = ((mp.x - canvas_rect.min.x + scroll_x) / pps).max(0.0);
                let target_frame = (target_sec * sample_rate as f32).round() as u64;
                app.action_seek(target_frame);
            }
        }

        // ---------------- Drag Operations (Move, Trim, Fade) ----------------
        if let Some(drag_op) = app.timeline_state.drag_op {
            if is_mouse_down {
                ui.ctx().request_repaint();
                if let Some(mp) = mouse_pos {
                    match drag_op {
                        DragOp::MoveClip {
                            clip_id,
                            track_id,
                            original_track_offset,
                            drag_start_x,
                        } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;
                            let new_offset =
                                (original_track_offset as i64 + delta_frames).max(0) as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.track_offset_frames = new_offset;
                                }
                            }
                        }
                        DragOp::TrimLeft {
                            clip_id,
                            track_id,
                            orig_source_start,
                            orig_track_offset,
                            orig_length,
                            drag_start_x,
                        } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;

                            let min_clip_len = (sample_rate as u64 / 20).max(500);
                            let max_trim = (orig_length.saturating_sub(min_clip_len)) as i64;
                            let min_trim = -(orig_track_offset.min(orig_source_start) as i64);
                            let max_trim = max_trim.max(min_trim);
                            let clamped_delta = delta_frames.clamp(min_trim, max_trim);

                            let new_start =
                                (orig_source_start as i64 + clamped_delta).max(0) as u64;
                            let new_offset =
                                (orig_track_offset as i64 + clamped_delta).max(0) as u64;
                            let new_length = (orig_length as i64 - clamped_delta)
                                .max(min_clip_len as i64)
                                as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.source_start_frame = new_start;
                                    clip.track_offset_frames = new_offset;
                                    clip.length_frames = new_length;
                                    clip.fade_in_frames = clip.fade_in_frames.min(new_length);
                                    clip.fade_out_frames = clip.fade_out_frames.min(new_length);
                                }
                            }
                        }
                        DragOp::TrimRight {
                            clip_id,
                            track_id,
                            orig_length,
                            drag_start_x,
                        } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;

                            let min_clip_len = (sample_rate as u64 / 20).max(500);
                            let source_info = app.project.get_track(track_id).and_then(|t| {
                                t.clips
                                    .iter()
                                    .find(|c| c.id == clip_id)
                                    .map(|c| (c.source_id, c.source_start_frame))
                            });
                            let max_source_len = source_info
                                .and_then(|(sid, start)| {
                                    app.project
                                        .sources
                                        .get(&sid)
                                        .map(|s| s.duration_frames.saturating_sub(start))
                                })
                                .unwrap_or(u64::MAX);

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    let new_length = (orig_length as i64 + delta_frames)
                                        .max(min_clip_len as i64)
                                        as u64;
                                    clip.length_frames =
                                        new_length.min(max_source_len).max(min_clip_len);
                                    clip.fade_in_frames =
                                        clip.fade_in_frames.min(clip.length_frames);
                                    clip.fade_out_frames =
                                        clip.fade_out_frames.min(clip.length_frames);
                                }
                            }
                        }
                        DragOp::FadeIn {
                            clip_id,
                            track_id,
                            orig_fade_in,
                            drag_start_x,
                        } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;
                            let new_fade = (orig_fade_in as i64 + delta_frames).max(0) as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.fade_in_frames = new_fade.min(clip.length_frames);
                                }
                            }
                        }
                        DragOp::FadeOut {
                            clip_id,
                            track_id,
                            orig_fade_out,
                            drag_start_x,
                        } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            let delta_x = drag_start_x - mp.x; // Dragging inward increases fade
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;
                            let new_fade = (orig_fade_out as i64 + delta_frames).max(0) as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.fade_out_frames = new_fade.min(clip.length_frames);
                                }
                            }
                        }
                        DragOp::SelectTimeRange { anchor_frame } => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            let curr_frame = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                                * sample_rate as f32)
                                .round() as u64;
                            let s = anchor_frame.min(curr_frame);
                            let e = anchor_frame.max(curr_frame);
                            app.timeline_state.time_selection = Some((s, e));
                            app.action_seek(curr_frame);
                        }
                        DragOp::DragSelectionLeft => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if let Some((_, e)) = app.timeline_state.time_selection {
                                let curr_frame =
                                    ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                                        * sample_rate as f32)
                                        .round() as u64;
                                let s = curr_frame.min(e);
                                app.timeline_state.time_selection = Some((s, e));
                                app.action_seek(s);
                            }
                        }
                        DragOp::DragSelectionRight => {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if let Some((s, _)) = app.timeline_state.time_selection {
                                let curr_frame =
                                    ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                                        * sample_rate as f32)
                                        .round() as u64;
                                let e = curr_frame.max(s);
                                app.timeline_state.time_selection = Some((s, e));
                                app.action_seek(e);
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // Mouse released, end drag
                if let Some(DragOp::SelectTimeRange { .. }) = app.timeline_state.drag_op {
                    if let Some((s, e)) = app.timeline_state.time_selection {
                        // If it was just a click without moving (< 150 frames / ~3ms), clear the range selection
                        if e.saturating_sub(s) < 150 {
                            app.timeline_state.time_selection = None;
                        }
                    }
                }
                app.timeline_state.drag_op = None;
                app.sync_timeline_to_audio_engine();
            }
        }

        // ---------------- Horizontal Scrollbar ----------------
        let h_scrollbar_rect = Rect::from_min_size(
            pos2(canvas_rect.min.x, canvas_rect.max.y - scrollbar_h),
            vec2(canvas_rect.width(), scrollbar_h),
        );
        painter.rect_filled(
            h_scrollbar_rect,
            Rounding::ZERO,
            Color32::from_rgb(22, 24, 28),
        );
        painter.line_segment(
            [
                pos2(h_scrollbar_rect.min.x, h_scrollbar_rect.min.y),
                pos2(h_scrollbar_rect.max.x, h_scrollbar_rect.min.y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55)),
        );

        if total_timeline_w > canvas_width {
            let thumb_ratio = (canvas_width / total_timeline_w).clamp(0.05, 1.0);
            let thumb_width = (canvas_width * thumb_ratio).max(30.0);
            let scroll_ratio =
                (app.timeline_state.scroll_x / max_scroll_x.max(1.0)).clamp(0.0, 1.0);
            let thumb_x = h_scrollbar_rect.min.x + scroll_ratio * (canvas_width - thumb_width);
            let thumb_rect = Rect::from_min_size(
                pos2(thumb_x, h_scrollbar_rect.min.y + 2.0),
                vec2(thumb_width, scrollbar_h - 4.0),
            );

            let is_h_hover = mouse_pos.is_some_and(|mp| h_scrollbar_rect.contains(mp));
            let thumb_color = if is_h_hover {
                Color32::from_rgb(100, 105, 120)
            } else {
                Color32::from_rgb(60, 64, 75)
            };
            painter.rect_filled(thumb_rect, Rounding::same(3.0), thumb_color);

            if is_mouse_down && is_h_hover {
                if let Some(mp) = mouse_pos {
                    let click_rel = (mp.x - h_scrollbar_rect.min.x - thumb_width * 0.5)
                        / (canvas_width - thumb_width).max(1.0);
                    app.timeline_state.scroll_x =
                        (click_rel.clamp(0.0, 1.0) * max_scroll_x).clamp(0.0, max_scroll_x);
                }
            }
        }

        // ---------------- Vertical Scrollbar ----------------
        if max_scroll_y > 0.0 {
            let v_scrollbar_w = 12.0_f32;
            let v_scrollbar_rect = Rect::from_min_size(
                pos2(canvas_rect.max.x - v_scrollbar_w, tracks_top),
                vec2(v_scrollbar_w, visible_tracks_h),
            );
            painter.rect_filled(
                v_scrollbar_rect,
                Rounding::ZERO,
                Color32::from_rgb(22, 24, 28),
            );

            let thumb_ratio = (visible_tracks_h / total_tracks_h.max(1.0)).clamp(0.08, 1.0);
            let thumb_h = (visible_tracks_h * thumb_ratio).max(24.0);
            let scroll_ratio =
                (app.timeline_state.scroll_y / max_scroll_y.max(1.0)).clamp(0.0, 1.0);
            let thumb_y = v_scrollbar_rect.min.y + scroll_ratio * (visible_tracks_h - thumb_h);
            let thumb_rect = Rect::from_min_size(
                pos2(v_scrollbar_rect.min.x + 2.0, thumb_y),
                vec2(v_scrollbar_w - 4.0, thumb_h),
            );

            let is_v_hover = mouse_pos.is_some_and(|mp| v_scrollbar_rect.contains(mp));
            let thumb_color = if is_v_hover {
                Color32::from_rgb(100, 105, 120)
            } else {
                Color32::from_rgb(60, 64, 75)
            };
            painter.rect_filled(thumb_rect, Rounding::same(3.0), thumb_color);

            if is_mouse_down && is_v_hover {
                if let Some(mp) = mouse_pos {
                    let click_rel = (mp.y - v_scrollbar_rect.min.y - thumb_h * 0.5)
                        / (visible_tracks_h - thumb_h).max(1.0);
                    app.timeline_state.scroll_y =
                        (click_rel.clamp(0.0, 1.0) * max_scroll_y).clamp(0.0, max_scroll_y);
                }
            }
        }
    });
}

/// Renders waveform peaks for a clip using the multi-resolution PeakPyramid.
fn render_clip_waveform(
    painter: &egui::Painter,
    wave_rect: Rect,
    clip: &Clip,
    pyramid: &PeakPyramid,
    _sample_rate: u32,
    is_selected: bool,
) {
    if wave_rect.height() < 4.0 || wave_rect.width() < 2.0 {
        return;
    }
    let target_points = (wave_rect.width() as usize).clamp(10, 2000);
    let peaks = pyramid.query_channel_peaks(
        0, // Left channel or mono
        clip.source_start_frame,
        clip.source_start_frame + clip.length_frames,
        target_points,
    );

    if peaks.is_empty() {
        return;
    }

    let mid_y = wave_rect.center().y;
    let half_h = (wave_rect.height() * 0.44).max(2.0);

    // Subtle zero line
    painter.line_segment(
        [pos2(wave_rect.min.x, mid_y), pos2(wave_rect.max.x, mid_y)],
        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
    );

    let waveform_color = if is_selected {
        Color32::from_rgb(125, 211, 252) // bright cyan highlight
    } else {
        Color32::from_rgb(56, 189, 248) // studio cyan
    };

    let step_x = wave_rect.width() / peaks.len() as f32;

    for (i, p) in peaks.iter().enumerate() {
        let x = wave_rect.min.x + i as f32 * step_x;
        let top_y = mid_y - (p.max.clamp(-1.0, 1.0) * half_h);
        let bot_y = mid_y - (p.min.clamp(-1.0, 1.0) * half_h);

        painter.line_segment(
            [pos2(x, top_y), pos2(x, bot_y.max(top_y + 1.0))],
            Stroke::new(1.0_f32, waveform_color),
        );
    }
}

/// Context parameters for rendering an automation lane.
struct AutomationLaneRenderCtx<'a> {
    pub lane_rect: Rect,
    pub target: stem_core::project::AutomationTarget,
    pub sample_rate: u32,
    pub pps: f32,
    pub scroll_x: f32,
    pub mouse_pos: Option<egui::Pos2>,
    pub is_mouse_down: bool,
    pub is_primary_clicked: bool,
    pub is_secondary_clicked: bool,
    pub drag_point: &'a mut Option<(uuid::Uuid, usize)>,
    pub track_id: uuid::Uuid,
}

/// Renders interactive breakpoint automation lane on the timeline canvas.
fn render_automation_lane(
    painter: &egui::Painter,
    lane: &mut stem_core::project::AutomationLane,
    ctx: AutomationLaneRenderCtx<'_>,
) -> bool {
    let mut modified = false;
    let lane_rect = ctx.lane_rect;
    let sample_rate = ctx.sample_rate;
    let pps = ctx.pps;
    let scroll_x = ctx.scroll_x;
    let mouse_pos = ctx.mouse_pos;
    let is_mouse_down = ctx.is_mouse_down;
    let is_primary_clicked = ctx.is_primary_clicked;
    let is_secondary_clicked = ctx.is_secondary_clicked;
    let drag_point = ctx.drag_point;
    let track_id = ctx.track_id;

    // Background
    painter.rect_filled(lane_rect, Rounding::ZERO, Color32::from_rgb(18, 20, 24));
    painter.line_segment(
        [
            pos2(lane_rect.min.x, lane_rect.max.y),
            pos2(lane_rect.max.x, lane_rect.max.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(35, 38, 45)),
    );

    let (val_min, val_max, val_zero) = match ctx.target {
        stem_core::project::AutomationTarget::Volume => (-60.0f32, 12.0f32, 0.0f32),
        stem_core::project::AutomationTarget::Pan => (-1.0f32, 1.0f32, 0.0f32),
    };

    let val_to_y = |val: f32| -> f32 {
        let norm = (val.clamp(val_min, val_max) - val_min) / (val_max - val_min);
        lane_rect.max.y - 4.0 - norm * (lane_rect.height() - 8.0)
    };

    let y_to_val = |y: f32| -> f32 {
        let norm = (lane_rect.max.y - 4.0 - y) / (lane_rect.height() - 8.0).max(1.0);
        val_min + norm.clamp(0.0, 1.0) * (val_max - val_min)
    };

    let frame_to_x = |frame: u64| -> f32 {
        lane_rect.min.x + (frame as f32 / sample_rate as f32 * pps) - scroll_x
    };

    let x_to_frame = |x: f32| -> u64 {
        let rel_x = (x - lane_rect.min.x + scroll_x).max(0.0);
        (rel_x / pps * sample_rate as f32).round() as u64
    };

    // Draw reference line at zero
    let zero_y = val_to_y(val_zero);
    painter.line_segment(
        [pos2(lane_rect.min.x, zero_y), pos2(lane_rect.max.x, zero_y)],
        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(100, 100, 120, 80)),
    );

    let stroke_color = match ctx.target {
        stem_core::project::AutomationTarget::Volume => Color32::from_rgb(80, 220, 120),
        stem_core::project::AutomationTarget::Pan => Color32::from_rgb(100, 190, 255),
    };

    // Draw segments between points
    if !lane.points.is_empty() {
        for i in 0..(lane.points.len() - 1) {
            let p1 = &lane.points[i];
            let p2 = &lane.points[i + 1];
            let x1 = frame_to_x(p1.frame);
            let y1 = val_to_y(p1.value);
            let x2 = frame_to_x(p2.frame);
            let y2 = val_to_y(p2.value);

            match p1.curve {
                stem_core::project::AutomationCurve::Step => {
                    painter.line_segment(
                        [pos2(x1, y1), pos2(x2, y1)],
                        Stroke::new(1.5_f32, stroke_color),
                    );
                    painter.line_segment(
                        [pos2(x2, y1), pos2(x2, y2)],
                        Stroke::new(1.5_f32, stroke_color),
                    );
                }
                stem_core::project::AutomationCurve::Linear => {
                    painter.line_segment(
                        [pos2(x1, y1), pos2(x2, y2)],
                        Stroke::new(1.5_f32, stroke_color),
                    );
                }
                stem_core::project::AutomationCurve::Bezier => {
                    let steps = 16;
                    let mut prev = pos2(x1, y1);
                    for s in 1..=steps {
                        let t = s as f32 / steps as f32;
                        let frame = p1.frame + ((p2.frame - p1.frame) as f32 * t) as u64;
                        let val = p1.curve.interpolate(t, p1.value, p2.value);
                        let next = pos2(frame_to_x(frame), val_to_y(val));
                        painter.line_segment([prev, next], Stroke::new(1.5_f32, stroke_color));
                        prev = next;
                    }
                }
            }
        }
    }

    // Dragging logic
    if is_mouse_down {
        if let Some((d_track, d_idx)) = *drag_point {
            if d_track == track_id && d_idx < lane.points.len() {
                if let Some(mp) = mouse_pos {
                    let new_frame = x_to_frame(mp.x);
                    let new_val = y_to_val(mp.y);
                    lane.points[d_idx].frame = new_frame;
                    lane.points[d_idx].value = new_val;
                    lane.points.sort_by_key(|p| p.frame);
                    modified = true;
                }
            }
        }
    }

    // Points interaction and drawing
    let mut point_to_remove: Option<usize> = None;
    let mut clicked_on_existing_point = false;

    for (idx, pt) in lane.points.iter().enumerate() {
        let x = frame_to_x(pt.frame);
        let y = val_to_y(pt.value);
        let pt_pos = pos2(x, y);

        let is_hovered = mouse_pos
            .map(|mp| mp.distance(pt_pos) <= 6.0)
            .unwrap_or(false);
        if is_hovered {
            clicked_on_existing_point = true;
            if is_secondary_clicked {
                point_to_remove = Some(idx);
            } else if is_mouse_down && drag_point.is_none() {
                *drag_point = Some((track_id, idx));
            }
        }

        let radius = if is_hovered { 5.0 } else { 3.5 };
        let fill = if is_hovered {
            Color32::WHITE
        } else {
            stroke_color
        };
        painter.circle_filled(pt_pos, radius, fill);
        painter.circle_stroke(pt_pos, radius, Stroke::new(1.0_f32, Color32::BLACK));
    }

    if let Some(idx) = point_to_remove {
        lane.points.remove(idx);
        modified = true;
    }

    // Click on empty lane space to add point
    if is_primary_clicked && !clicked_on_existing_point && drag_point.is_none() {
        if let Some(mp) = mouse_pos {
            if lane_rect.contains(mp) {
                let frame = x_to_frame(mp.x);
                let val = y_to_val(mp.y);
                lane.add_point(frame, val, stem_core::project::AutomationCurve::Linear);
                lane.enabled = true;
                modified = true;
            }
        }
    }

    modified
}
