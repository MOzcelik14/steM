use super::automation::{render_automation_lane, AutomationLaneRenderCtx};
use super::{CanvasFrame, TimelineGeom};
use crate::app::StemApp;
use crate::state::DragOp;
use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_core::peaks::PeakPyramid;
use stem_core::project::Clip;

pub(super) fn render_timeline_canvas(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    geom: &mut TimelineGeom,
    delete_track_id: Option<uuid::Uuid>,
) -> Option<CanvasFrame> {
    let available_size = geom.available_size;
    let header_width = geom.header_width;
    let ruler_height = geom.ruler_height;
    let scrollbar_h = geom.scrollbar_h;
    let sample_rate = geom.sample_rate;
    let pps = geom.pps;
    let base_track_height = geom.base_track_height;
    let track_spacing = geom.track_spacing;
    let track_heights = &mut geom.track_heights;
    let track_y_offsets = &mut geom.track_y_offsets;
    // ================= Right Column: Timeline Canvas =================
    let canvas_width = (available_size.x - header_width).max(100.0);
    let canvas_size = vec2(canvas_width, available_size.y);

    let (canvas_rect, response) =
        ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(canvas_rect);

    // Background
    painter.rect_filled(canvas_rect, Rounding::ZERO, Color32::from_rgb(20, 22, 26));

    // ---------------- Timeline Ruler ----------------
    let ruler_rect = Rect::from_min_size(canvas_rect.min, vec2(canvas_rect.width(), ruler_height));
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
        0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
        3600.0,
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

    let can_interact = app.timeline_state.drag_op.is_some() || response.hovered();
    let mouse_pos = if can_interact {
        response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos())
            .or_else(|| ui.input(|i| i.pointer.latest_pos()))
    } else {
        None
    };
    let is_mouse_down = ui.input(|i| i.pointer.primary_down()) && can_interact;
    let is_primary_clicked = ui.input(|i| i.pointer.primary_clicked()) && can_interact;
    let is_secondary_clicked = ui.input(|i| i.pointer.secondary_clicked()) && can_interact;
    let mut open_clip_in_waveform: Option<uuid::Uuid> = None;
    let mut timeline_needs_sync = false;

    if let Some(id) = delete_track_id {
        app.action_delete_track(id);
    }
    if track_y_offsets.len() != app.project.tracks.len() {
        track_heights.clear();
        track_y_offsets.clear();
        let mut y_acc = 0.0_f32;
        for track in &app.project.tracks {
            let is_auto_expanded =
                app.timeline_state.expanded_automation_track_id == Some(track.id);
            let h = if is_auto_expanded {
                base_track_height + 55.0
            } else {
                base_track_height
            };
            track_heights.push(h);
            track_y_offsets.push(y_acc);
            y_acc += h + track_spacing;
        }
    }

    for t_idx in 0..app.project.tracks.len() {
        let track_id = app.project.tracks[t_idx].id;
        let is_auto_expanded = app.timeline_state.expanded_automation_track_id == Some(track_id);
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
                    let x1 =
                        canvas_rect.min.x + (xf_start as f32 / sample_rate as f32 * pps) - scroll_x;
                    let x2 =
                        canvas_rect.min.x + (xf_end as f32 / sample_rate as f32 * pps) - scroll_x;
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

            let is_clip_selected = app.timeline_state.selected_clip_id == Some(clip.id)
                || app.timeline_state.selected_clip_ids.contains(&clip.id);

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

            // Clip title text clipped strictly inside clip_rect
            let clip_text_painter = lane_painter.with_clip_rect(clip_rect);
            clip_text_painter.text(
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
            let fade_in_handle_x =
                (clip_rect.min.x + fade_in_w - fade_size * 0.5).clamp(clip_rect.min.x, max_fade_x);
            let fade_in_handle_rect = Rect::from_min_size(
                pos2(fade_in_handle_x, clip_rect.min.y),
                vec2(fade_size, fade_size),
            );

            let fade_out_handle_x =
                (clip_rect.max.x - fade_out_w - fade_size * 0.5).clamp(clip_rect.min.x, max_fade_x);
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

            let is_left_trim_hovered = mouse_pos.is_some_and(|mp| left_handle_rect.contains(mp));
            let is_right_trim_hovered = mouse_pos.is_some_and(|mp| right_handle_rect.contains(mp));
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

                lane_painter.rect_filled(fade_in_handle_rect, Rounding::same(2.0), fade_in_color);
                lane_painter.rect_filled(fade_out_handle_rect, Rounding::same(2.0), fade_out_color);
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

            // Clip Gain horizontal line (-24 dB .. +12 dB mapped to clip height)
            let gain_norm = ((clip.gain_db + 24.0) / 36.0).clamp(0.0, 1.0);
            let gain_y = (clip_rect.max.y - 6.0) - gain_norm * (clip_rect.height() - 22.0).max(8.0);
            let is_gain_line_hovered = !is_left_trim_hovered
                && !is_right_trim_hovered
                && !is_fade_in_hovered
                && !is_fade_out_hovered
                && mouse_pos.is_some_and(|mp| {
                    mp.x >= clip_rect.min.x + 6.0
                        && mp.x <= clip_rect.max.x - 6.0
                        && (mp.y - gain_y).abs() <= 4.0
                });
            let gain_line_color = if is_gain_line_hovered {
                Color32::from_rgb(130, 235, 255)
            } else {
                Color32::from_rgba_premultiplied(120, 210, 240, 130)
            };
            if clip_rect.width() > 20.0 && clip_rect.height() > 28.0 {
                lane_painter.line_segment(
                    [
                        pos2(clip_rect.min.x + 2.0, gain_y),
                        pos2(clip_rect.max.x - 2.0, gain_y),
                    ],
                    Stroke::new(
                        if is_gain_line_hovered {
                            2.0_f32
                        } else {
                            1.0_f32
                        },
                        gain_line_color,
                    ),
                );
                if clip.gain_db.abs() > 0.1 || is_gain_line_hovered {
                    lane_painter.text(
                        pos2(clip_rect.min.x + 6.0, gain_y - 10.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:+.1} dB", clip.gain_db),
                        egui::FontId::proportional(9.5),
                        gain_line_color,
                    );
                }
            }

            // Check for click / selection / drag start
            if let Some(mp) = mouse_pos {
                if clip_rect.contains(mp) && app.timeline_state.drag_op.is_none() {
                    if is_secondary_clicked {
                        app.timeline_state.selected_clip_id = Some(clip.id);
                        app.timeline_state.selected_track_id = Some(track_id);
                        if !app.timeline_state.selected_clip_ids.contains(&clip.id) {
                            app.timeline_state.selected_clip_ids.clear();
                            app.timeline_state.selected_clip_ids.insert(clip.id);
                        }
                    }
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
                    } else if is_gain_line_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        if primary_pressed {
                            app.timeline_state.selected_clip_id = Some(clip.id);
                            app.timeline_state.selected_track_id = Some(track_id);
                            app.timeline_state.drag_op = Some(DragOp::AdjustClipGain {
                                clip_id: clip.id,
                                track_id,
                                orig_gain_db: clip.gain_db,
                                drag_start_y: mp.y,
                            });
                        }
                    } else if primary_double {
                        open_clip_in_waveform = Some(clip.id);
                    } else if primary_pressed {
                        if ui.input(|i| i.modifiers.shift) {
                            app.timeline_state.selected_clip_ids.clear();
                            app.timeline_state.selected_clip_id = None;
                            app.timeline_state.drag_op = Some(DragOp::BoxSelect {
                                start_pos: mp,
                                current_pos: mp,
                            });
                        } else {
                            app.timeline_state.selected_clip_ids.clear();
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
                    default_curve: app.timeline_state.automation_default_curve,
                };
                if render_automation_lane(&lane_painter, lane, ctx) {
                    timeline_needs_sync = true;
                }
            }
        }
    }

    if !is_mouse_down {
        if app.timeline_state.drag_automation_point.is_some() {
            timeline_needs_sync = true;
        }
        app.timeline_state.drag_automation_point = None;
    }
    if timeline_needs_sync && (!is_mouse_down || app.timeline_state.drag_automation_point.is_none())
    {
        app.sync_timeline_to_audio_engine();
    }

    if let Some(clip_id) = open_clip_in_waveform {
        app.action_open_clip_in_waveform_editor(clip_id);
        return None;
    }

    let scroll_x = app.timeline_state.scroll_x;
    Some(CanvasFrame {
        canvas_rect,
        response,
        painter,
        ruler_rect,
        tracks_top,
        mouse_pos,
        is_mouse_down,
        is_primary_clicked,
        scroll_x,
        start_time_sec,
        end_time_sec,
    })
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
    let clip_bounds = painter.clip_rect();
    let vis_min_x = wave_rect.min.x.max(clip_bounds.min.x);
    let vis_max_x = wave_rect.max.x.min(clip_bounds.max.x);
    let vis_width = vis_max_x - vis_min_x;
    if vis_width < 1.0 {
        return;
    }

    let frac_start = ((vis_min_x - wave_rect.min.x) / wave_rect.width()).clamp(0.0, 1.0);
    let frac_end = ((vis_max_x - wave_rect.min.x) / wave_rect.width()).clamp(0.0, 1.0);
    let query_start =
        clip.source_start_frame + (clip.length_frames as f64 * frac_start as f64).floor() as u64;
    let query_end = (clip.source_start_frame
        + (clip.length_frames as f64 * frac_end as f64).ceil() as u64)
        .min(clip.source_start_frame + clip.length_frames);
    if query_end <= query_start {
        return;
    }

    let target_points = (vis_width as usize).clamp(10, 2000);
    let peaks = pyramid.query_channel_peaks(
        0, // Left channel or mono
        query_start,
        query_end,
        target_points,
    );

    if peaks.is_empty() {
        return;
    }

    let mid_y = wave_rect.center().y;
    let half_h = (wave_rect.height() * 0.44).max(2.0);

    // Subtle zero line
    painter.line_segment(
        [pos2(vis_min_x, mid_y), pos2(vis_max_x, mid_y)],
        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
    );

    let waveform_color = if is_selected {
        Color32::from_rgb(125, 211, 252) // bright cyan highlight
    } else {
        Color32::from_rgb(56, 189, 248) // studio cyan
    };
    let stroke = Stroke::new(1.0_f32, waveform_color);
    let step_x = vis_width / peaks.len() as f32;

    let shapes: Vec<egui::Shape> = peaks
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let x = vis_min_x + i as f32 * step_x;
            let top_y = mid_y - (p.max.clamp(-1.0, 1.0) * half_h);
            let bot_y = mid_y - (p.min.clamp(-1.0, 1.0) * half_h);
            egui::Shape::line_segment([pos2(x, top_y), pos2(x, bot_y.max(top_y + 1.0))], stroke)
        })
        .collect();
    painter.extend(shapes);
}
