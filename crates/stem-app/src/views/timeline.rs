//! Multi-track timeline view with smooth multi-resolution waveform visualization

use crate::app::StemApp;
use crate::state::DragOp;
use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_core::peaks::PeakPyramid;
use stem_core::project::Clip;

pub fn render_timeline_view(app: &mut StemApp, ui: &mut egui::Ui) {
    let available_size = ui.available_size();
    let header_width = 220.0;
    let ruler_height = 24.0;

    let sample_rate = app.project.sample_rate;
    let pps = app.timeline_state.pixels_per_second;

    // Handle horizontal / vertical mouse wheel zooming and scrolling
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
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
    } else if ui.input(|i| i.modifiers.shift) && scroll_delta.y != 0.0 {
        app.timeline_state.scroll_x = (app.timeline_state.scroll_x - scroll_delta.y).max(0.0);
    }

    ui.horizontal(|ui| {
        // ================= Left Column: Track Headers =================
        ui.allocate_ui_with_layout(
            vec2(header_width, available_size.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // Ruler spacer
                ui.add_space(ruler_height);

                let track_height = app.timeline_state.track_height;
                let num_tracks = app.project.tracks.len();

                for t_idx in 0..num_tracks {
                    let track = &app.project.tracks[t_idx];
                    let track_id = track.id;

                    let is_auto_expanded =
                        app.timeline_state.expanded_automation_track_id == Some(track_id);
                    let header_h = if is_auto_expanded {
                        track_height + 50.0
                    } else {
                        track_height
                    };

                    let (rect, _response) = ui.allocate_exact_size(
                        vec2(header_width - 8.0, header_h),
                        egui::Sense::click(),
                    );

                    let painter = ui.painter();
                    let is_selected = app.timeline_state.selected_track_id == Some(track_id);
                    let bg_color = if is_selected {
                        Color32::from_rgb(45, 55, 75)
                    } else {
                        Color32::from_rgb(32, 34, 38)
                    };
                    painter.rect_filled(rect, Rounding::same(4.0), bg_color);
                    painter.rect_stroke(
                        rect,
                        Rounding::same(4.0),
                        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 55)),
                    );

                    // Track Header Controls
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
                        ui.horizontal(|ui| {
                            let track_name = app.project.tracks[t_idx].name.clone();
                            ui.label(egui::RichText::new(&track_name).strong());

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let auto_active = is_auto_expanded;
                                    let auto_color = if auto_active {
                                        Color32::from_rgb(100, 220, 150)
                                    } else {
                                        Color32::GRAY
                                    };
                                    if ui
                                        .selectable_label(
                                            auto_active,
                                            egui::RichText::new("Auto")
                                                .size(10.0)
                                                .color(auto_color),
                                        )
                                        .clicked()
                                    {
                                        if auto_active {
                                            app.timeline_state.expanded_automation_track_id = None;
                                        } else {
                                            app.timeline_state.expanded_automation_track_id =
                                                Some(track_id);
                                        }
                                    }
                                },
                            );
                        });

                        ui.horizontal(|ui| {
                            // Mute button
                            let mut muted = app.project.tracks[t_idx].muted;
                            let mute_color = if muted {
                                Color32::from_rgb(230, 80, 80)
                            } else {
                                Color32::GRAY
                            };
                            if ui
                                .selectable_label(
                                    muted,
                                    egui::RichText::new(app.i18n.t("track-header-mute"))
                                        .color(mute_color),
                                )
                                .clicked()
                            {
                                muted = !muted;
                                app.action_set_track_mute(t_idx, muted);
                            }

                            // Solo button
                            let mut soloed = app.project.tracks[t_idx].soloed;
                            let solo_color = if soloed {
                                Color32::from_rgb(220, 200, 50)
                            } else {
                                Color32::GRAY
                            };
                            if ui
                                .selectable_label(
                                    soloed,
                                    egui::RichText::new(app.i18n.t("track-header-solo"))
                                        .color(solo_color),
                                )
                                .clicked()
                            {
                                soloed = !soloed;
                                app.action_set_track_solo(t_idx, soloed);
                            }

                            // Arm button
                            let mut armed = app.project.tracks[t_idx].armed;
                            let arm_color = if armed {
                                Color32::from_rgb(240, 50, 50)
                            } else {
                                Color32::GRAY
                            };
                            if ui
                                .selectable_label(
                                    armed,
                                    egui::RichText::new(app.i18n.t("track-header-arm"))
                                        .color(arm_color),
                                )
                                .clicked()
                            {
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
                                .selected_text(egui::RichText::new(routing_label).size(10.0))
                                .width(70.0)
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

                        // Gain Slider
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(app.i18n.t("track-header-volume")).size(11.0),
                            );
                            let mut gain_db = app.project.tracks[t_idx].gain_db;
                            if ui
                                .add(
                                    egui::Slider::new(&mut gain_db, -48.0..=12.0).show_value(false),
                                )
                                .changed()
                            {
                                app.action_set_track_gain(t_idx, gain_db);
                            }
                        });

                        // Pan Slider
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(app.i18n.t("track-header-pan")).size(11.0),
                            );
                            let mut pan = app.project.tracks[t_idx].pan;
                            if ui
                                .add(egui::Slider::new(&mut pan, -1.0..=1.0).show_value(false))
                                .changed()
                            {
                                app.action_set_track_pan(t_idx, pan);
                            }
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
                                    ui.checkbox(
                                        &mut lane.enabled,
                                        egui::RichText::new("On").size(9.0),
                                    );
                                }
                            });
                        }
                    });

                    ui.add_space(4.0);
                }
            },
        );

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
        let track_height = app.timeline_state.track_height;

        let mouse_pos = response.hover_pos();
        let is_mouse_down = ui.input(|i| i.pointer.primary_down());
        let is_primary_clicked = ui.input(|i| i.pointer.primary_clicked());
        let is_secondary_clicked = ui.input(|i| i.pointer.secondary_clicked());
        let mut open_clip_in_waveform: Option<uuid::Uuid> = None;
        let mut curr_track_top = tracks_top;
        let mut timeline_needs_sync = false;

        for t_idx in 0..app.project.tracks.len() {
            let track_id = app.project.tracks[t_idx].id;
            let is_auto_expanded =
                app.timeline_state.expanded_automation_track_id == Some(track_id);
            let track_top = curr_track_top;
            let track_rect = Rect::from_min_size(
                pos2(canvas_rect.min.x, track_top),
                vec2(canvas_rect.width(), track_height),
            );

            // Alternate track background
            let track_bg = if t_idx % 2 == 0 {
                Color32::from_rgb(24, 26, 30)
            } else {
                Color32::from_rgb(28, 30, 35)
            };
            painter.rect_filled(track_rect, Rounding::ZERO, track_bg);
            painter.line_segment(
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
                    vec2(clip_width, track_height - 8.0),
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
                            pos2(x2.min(canvas_rect.max.x), track_top + track_height - 4.0),
                        );
                        if xf_rect.width() > 2.0 {
                            // Crossfade region overlay
                            painter.rect_filled(
                                xf_rect,
                                Rounding::ZERO,
                                Color32::from_rgba_premultiplied(40, 150, 220, 50),
                            );
                            painter.line_segment(
                                [
                                    pos2(xf_rect.min.x, xf_rect.min.y),
                                    pos2(xf_rect.max.x, xf_rect.max.y),
                                ],
                                Stroke::new(1.5_f32, Color32::from_rgb(100, 220, 255)),
                            );
                            painter.line_segment(
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
                            painter.text(
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
                if clip_rect.max.x < canvas_rect.min.x || clip_rect.min.x > canvas_rect.max.x {
                    continue;
                }

                let is_clip_selected = app.timeline_state.selected_clip_id == Some(clip.id);

                // Clip box
                let clip_bg = if is_clip_selected {
                    Color32::from_rgb(40, 70, 110)
                } else {
                    Color32::from_rgb(35, 55, 80)
                };
                painter.rect_filled(clip_rect, Rounding::same(3.0), clip_bg);

                let stroke_color = if is_clip_selected {
                    Color32::from_rgb(100, 180, 255)
                } else {
                    Color32::from_rgb(60, 100, 150)
                };
                painter.rect_stroke(
                    clip_rect,
                    Rounding::same(3.0),
                    Stroke::new(1.5_f32, stroke_color),
                );

                // Clip title
                painter.text(
                    clip_rect.min + vec2(6.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &clip.name,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );

                // Draw Waveform using multi-resolution PeakPyramid
                if let Some(pyramid) = app.peak_pyramids.get(&clip.source_id) {
                    render_clip_waveform(&painter, clip_rect, clip, pyramid, sample_rate);
                }

                // Render Fade In & Fade Out curves
                if clip.fade_in_frames > 0 {
                    let fade_in_w = (clip.fade_in_frames as f32 / sample_rate as f32 * pps)
                        .min(clip_rect.width());
                    painter.line_segment(
                        [
                            pos2(clip_rect.min.x, clip_rect.max.y),
                            pos2(clip_rect.min.x + fade_in_w, clip_rect.min.y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(255, 200, 80)),
                    );
                }
                if clip.fade_out_frames > 0 {
                    let fade_out_w = (clip.fade_out_frames as f32 / sample_rate as f32 * pps)
                        .min(clip_rect.width());
                    painter.line_segment(
                        [
                            pos2(clip_rect.max.x - fade_out_w, clip_rect.min.y),
                            pos2(clip_rect.max.x, clip_rect.max.y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(255, 200, 80)),
                    );
                }

                // Visual handles and hit-testing for Trim and Fades
                let fade_size = 14.0_f32;
                let fade_in_handle_rect =
                    Rect::from_min_size(clip_rect.min, vec2(fade_size, fade_size));
                let fade_out_handle_rect = Rect::from_min_size(
                    pos2(clip_rect.max.x - fade_size, clip_rect.min.y),
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

                // Draw subtle fade corner handles
                let is_fade_in_hovered =
                    mouse_pos.is_some_and(|mp| fade_in_handle_rect.contains(mp));
                let is_fade_out_hovered =
                    mouse_pos.is_some_and(|mp| fade_out_handle_rect.contains(mp));
                let is_left_trim_hovered =
                    mouse_pos.is_some_and(|mp| left_handle_rect.contains(mp));
                let is_right_trim_hovered =
                    mouse_pos.is_some_and(|mp| right_handle_rect.contains(mp));

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

                painter.rect_filled(fade_in_handle_rect, Rounding::same(2.0), fade_in_color);
                painter.rect_filled(fade_out_handle_rect, Rounding::same(2.0), fade_out_color);

                // Highlight trim edge on hover
                if is_left_trim_hovered {
                    painter.rect_filled(
                        left_handle_rect,
                        Rounding::same(2.0),
                        Color32::from_rgba_premultiplied(120, 200, 255, 160),
                    );
                }
                if is_right_trim_hovered {
                    painter.rect_filled(
                        right_handle_rect,
                        Rounding::same(2.0),
                        Color32::from_rgba_premultiplied(120, 200, 255, 160),
                    );
                }

                // Check for click / selection / drag start
                if let Some(mp) = mouse_pos {
                    if clip_rect.contains(mp) {
                        let primary_pressed =
                            ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
                        let primary_double = ui.input(|i| {
                            i.pointer
                                .button_double_clicked(egui::PointerButton::Primary)
                        });

                        if is_fade_in_hovered {
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
                        } else if is_left_trim_hovered {
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
                        } else if primary_double {
                            open_clip_in_waveform = Some(clip.id);
                        } else if primary_pressed {
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

            // Render Automation Lane if expanded
            if is_auto_expanded {
                let lane_rect = Rect::from_min_size(
                    pos2(canvas_rect.min.x, curr_track_top + track_height),
                    vec2(canvas_rect.width(), 50.0),
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
                    if render_automation_lane(&painter, lane, ctx) {
                        timeline_needs_sync = true;
                    }
                }
                curr_track_top += track_height + 50.0 + 4.0;
            } else {
                curr_track_top += track_height + 4.0;
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
                if let Some(mp) = mouse_pos {
                    match drag_op {
                        DragOp::MoveClip {
                            clip_id,
                            track_id,
                            original_track_offset,
                            drag_start_x,
                        } => {
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
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;

                            let max_trim =
                                (orig_length.saturating_sub(sample_rate as u64 / 10)) as i64;
                            let clamped_delta =
                                delta_frames.clamp(-(orig_track_offset as i64), max_trim);

                            let new_start =
                                (orig_source_start as i64 + clamped_delta).max(0) as u64;
                            let new_offset =
                                (orig_track_offset as i64 + clamped_delta).max(0) as u64;
                            let new_length = (orig_length as i64 - clamped_delta).max(100) as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.source_start_frame = new_start;
                                    clip.track_offset_frames = new_offset;
                                    clip.length_frames = new_length;
                                }
                            }
                        }
                        DragOp::TrimRight {
                            clip_id,
                            track_id,
                            orig_length,
                            drag_start_x,
                        } => {
                            let delta_x = mp.x - drag_start_x;
                            let delta_frames = (delta_x / pps * sample_rate as f32).round() as i64;
                            let new_length = (orig_length as i64 + delta_frames).max(100) as u64;

                            if let Some(track) = app.project.get_track_mut(track_id) {
                                if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id)
                                {
                                    clip.length_frames = new_length;
                                }
                            }
                        }
                        DragOp::FadeIn {
                            clip_id,
                            track_id,
                            orig_fade_in,
                            drag_start_x,
                        } => {
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
                        _ => {}
                    }
                }
            } else {
                // Mouse released, end drag
                app.timeline_state.drag_op = None;
                app.sync_timeline_to_audio_engine();
            }
        }
    });
}

/// Renders waveform peaks for a clip using the multi-resolution PeakPyramid.
fn render_clip_waveform(
    painter: &egui::Painter,
    clip_rect: Rect,
    clip: &Clip,
    pyramid: &PeakPyramid,
    _sample_rate: u32,
) {
    let target_points = (clip_rect.width() as usize).clamp(10, 2000);
    let peaks = pyramid.query_channel_peaks(
        0, // Left channel or mono
        clip.source_start_frame,
        clip.source_start_frame + clip.length_frames,
        target_points,
    );

    if peaks.is_empty() {
        return;
    }

    let mid_y = clip_rect.center().y;
    let half_h = (clip_rect.height() * 0.42).max(4.0);
    let waveform_color = Color32::from_rgb(120, 200, 255);

    let step_x = clip_rect.width() / peaks.len() as f32;

    for (i, p) in peaks.iter().enumerate() {
        let x = clip_rect.min.x + i as f32 * step_x;
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
