use super::{CanvasFrame, TimelineGeom};
use crate::app::StemApp;
use crate::state::DragOp;
use egui::{pos2, vec2, Color32, CornerRadius, Rect, Stroke};

pub(super) fn render_overlays_and_interactions(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    geom: &TimelineGeom,
    frame: &CanvasFrame,
) {
    let ruler_height = geom.ruler_height;
    let scrollbar_h = geom.scrollbar_h;
    let sample_rate = geom.sample_rate;
    let pps = geom.pps;
    let track_heights = &geom.track_heights;
    let track_y_offsets = &geom.track_y_offsets;
    let canvas_rect = frame.canvas_rect;
    let response = &frame.response;
    let painter = &frame.painter;
    let ruler_rect = frame.ruler_rect;
    let tracks_top = frame.tracks_top;
    let mouse_pos = frame.mouse_pos;
    let is_mouse_down = frame.is_mouse_down;
    let is_primary_clicked = frame.is_primary_clicked;
    let scroll_x = frame.scroll_x;
    let start_time_sec = frame.start_time_sec;
    let end_time_sec = frame.end_time_sec;
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

            // 2. Click on empty space: Shift+Drag 2D BoxSelect or time range selection & seek playhead
            if primary_pressed && app.timeline_state.drag_op.is_none() {
                app.timeline_state.selected_clip_ids.clear();
                app.timeline_state.selected_clip_id = None;
                if ui.input(|i| i.modifiers.shift) {
                    app.timeline_state.drag_op = Some(DragOp::BoxSelect {
                        start_pos: mp,
                        current_pos: mp,
                    });
                } else {
                    app.timeline_state.drag_op = Some(DragOp::SelectTimeRange {
                        anchor_frame: frame_at_mouse,
                    });
                    app.timeline_state.time_selection = Some((frame_at_mouse, frame_at_mouse));
                    app.action_seek(frame_at_mouse);
                }
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
            let in_x = canvas_rect.min.x + (punch_in as f32 / sample_rate as f32 * pps) - scroll_x;
            let out_x =
                canvas_rect.min.x + (punch_out as f32 / sample_rate as f32 * pps) - scroll_x;

            let punch_rect = Rect::from_min_max(
                pos2(in_x.max(canvas_rect.min.x), canvas_rect.min.y),
                pos2(out_x.min(canvas_rect.max.x), canvas_rect.max.y),
            );

            if punch_rect.width() > 0.0 {
                painter.rect_filled(
                    punch_rect,
                    CornerRadius::ZERO,
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
        let m_x = canvas_rect.min.x + (marker.frame as f32 / sample_rate as f32 * pps) - scroll_x;
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
            let hit_rect =
                Rect::from_min_size(pos2(m_x - 3.0, ruler_rect.min.y), vec2(40.0, ruler_height));
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
                CornerRadius::ZERO,
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
                CornerRadius::ZERO,
                Color32::from_rgba_premultiplied(100, 190, 255, 65),
            );

            if sel_rect.width() > 36.0 && app.timeline_state.drag_op.is_none() {
                let bar_rect = Rect::from_min_size(
                    pos2(sel_rect.min.x + 4.0, tracks_top + 4.0),
                    vec2(260.0, 22.0),
                );
                let bar_ui = egui::UiBuilder::new()
                    .max_rect(bar_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                    .id_salt("time_sel_quick_bar");
                let mut action: Option<u8> = None;
                ui.allocate_new_ui(bar_ui, |ui| {
                    egui::Frame::NONE
                        .fill(Color32::from_rgba_premultiplied(20, 24, 34, 230))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(90, 170, 255)))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(4, 2))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
                            if ui
                                .small_button("✂")
                                .on_hover_text(app.i18n.t("action-split-at-selection"))
                                .clicked()
                            {
                                action = Some(1);
                            }
                            if ui
                                .small_button("🔇")
                                .on_hover_text(app.i18n.t("action-silence-selection"))
                                .clicked()
                            {
                                action = Some(2);
                            }
                            if ui
                                .small_button("⌫")
                                .on_hover_text(app.i18n.t("action-ripple-delete-selection"))
                                .clicked()
                            {
                                action = Some(3);
                            }
                            if ui
                                .small_button("📤")
                                .on_hover_text(app.i18n.t("action-export-selection"))
                                .clicked()
                            {
                                action = Some(4);
                            }
                        });
                });
                match action {
                    Some(1) => app.action_split_at_selection(),
                    Some(2) => app.action_silence_selection(),
                    Some(3) => app.action_ripple_delete_selection(),
                    Some(4) => {
                        app.export_settings.range = stem_audio::ExportRange::TimeRange {
                            start_frame: s_frame.min(e_frame),
                            end_frame: s_frame.max(e_frame),
                        };
                        app.show_export_dialog = true;
                    }
                    _ => {}
                }
            }
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
            CornerRadius::same(8),
            Color32::from_rgba_premultiplied(18, 20, 26, 230),
        );
        painter.rect_stroke(
            hud_rect,
            CornerRadius::same(8),
            Stroke::new(2.0_f32, Color32::from_rgb(255, 170, 40)),
            egui::StrokeKind::Middle,
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
        let bar_rect =
            Rect::from_center_size(pos2(center.x, center.y + 16.0), vec2(200.0 * progress, 6.0));
        painter.rect_filled(
            bar_rect,
            CornerRadius::same(3),
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
                        let raw_offset =
                            (original_track_offset as i64 + delta_frames).max(0) as u64;
                        let new_offset = stem_core::project::snap_frame(
                            raw_offset,
                            sample_rate,
                            app.project.tempo_bpm,
                            app.project.time_signature.0,
                            app.timeline_state.snap_mode,
                        );

                        if let Some(track) = app.project.get_track_mut(track_id) {
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
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

                        let new_start = (orig_source_start as i64 + clamped_delta).max(0) as u64;
                        let new_offset = (orig_track_offset as i64 + clamped_delta).max(0) as u64;
                        let new_length =
                            (orig_length as i64 - clamped_delta).max(min_clip_len as i64) as u64;

                        if let Some(track) = app.project.get_track_mut(track_id) {
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
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
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                                let new_length = (orig_length as i64 + delta_frames)
                                    .max(min_clip_len as i64)
                                    as u64;
                                clip.length_frames =
                                    new_length.min(max_source_len).max(min_clip_len);
                                clip.fade_in_frames = clip.fade_in_frames.min(clip.length_frames);
                                clip.fade_out_frames = clip.fade_out_frames.min(clip.length_frames);
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
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
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
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
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
                            let curr_frame = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
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
                            let curr_frame = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                                * sample_rate as f32)
                                .round() as u64;
                            let e = curr_frame.max(s);
                            app.timeline_state.time_selection = Some((s, e));
                            app.action_seek(e);
                        }
                    }
                    DragOp::AdjustClipGain {
                        clip_id,
                        track_id,
                        orig_gain_db,
                        drag_start_y,
                    } => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        let new_gain =
                            (orig_gain_db + (drag_start_y - mp.y) * 0.25).clamp(-24.0, 12.0);
                        if let Some(track) = app.project.get_track_mut(track_id) {
                            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                                clip.gain_db = new_gain;
                            }
                        }
                    }
                    DragOp::BoxSelect {
                        start_pos,
                        current_pos: _,
                    } => {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                        let marquee = Rect::from_two_pos(start_pos, mp);
                        painter.rect(
                            marquee,
                            CornerRadius::same(2),
                            Color32::from_rgba_premultiplied(59, 130, 246, 45),
                            Stroke::new(1.5_f32, Color32::from_rgb(96, 165, 250)),
                            egui::StrokeKind::Middle,
                        );
                        let f_start = ((marquee.min.x - canvas_rect.min.x + scroll_x).max(0.0)
                            / pps
                            * sample_rate as f32)
                            .round() as u64;
                        let f_end = ((marquee.max.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                            * sample_rate as f32)
                            .round() as u64;
                        app.timeline_state.time_selection = None;

                        app.timeline_state.selected_clip_ids.clear();
                        app.timeline_state.selected_clip_id = None;
                        let mut cur_y = tracks_top - app.timeline_state.scroll_y;
                        let lane_h = app.timeline_state.track_height;
                        for track in &app.project.tracks {
                            let t_bot = cur_y + lane_h;
                            let y_overlaps = t_bot >= marquee.min.y && cur_y <= marquee.max.y;
                            if y_overlaps {
                                for clip in &track.clips {
                                    let c_start = clip.track_offset_frames;
                                    let c_end = clip.timeline_end_frame();
                                    if c_end > f_start && c_start < f_end {
                                        app.timeline_state.selected_clip_ids.insert(clip.id);
                                        if app.timeline_state.selected_clip_id.is_none() {
                                            app.timeline_state.selected_clip_id = Some(clip.id);
                                            app.timeline_state.selected_track_id = Some(track.id);
                                        }
                                    }
                                }
                            }
                            let is_exp =
                                app.timeline_state.expanded_automation_track_id == Some(track.id);
                            cur_y += lane_h + if is_exp { 50.0 } else { 0.0 } + 4.0;
                        }
                        app.timeline_state.drag_op = Some(DragOp::BoxSelect {
                            start_pos,
                            current_pos: mp,
                        });
                    }
                    _ => {}
                }
            }
        } else {
            // Mouse released, end drag
            let mut mutated = false;
            if let Some(DragOp::MoveClip {
                clip_id,
                track_id,
                original_track_offset,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur_offset = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| c.track_offset_frames);
                if let Some(new_offset) = cur_offset {
                    if new_offset != original_track_offset {
                        app.history
                            .push_recorded(stem_core::history::Action::MoveClip {
                                track_id,
                                clip_id,
                                old_track_offset: original_track_offset,
                                new_track_offset: new_offset,
                            });
                    }
                }
                if app.timeline_state.auto_crossfade {
                    app.project
                        .apply_auto_crossfades_on_track(track_id, clip_id);
                }
                mutated = true;
            } else if let Some(DragOp::TrimLeft {
                clip_id,
                track_id,
                orig_source_start,
                orig_track_offset,
                orig_length,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| (c.source_start_frame, c.length_frames, c.track_offset_frames));
                if let Some((ns, nl, no)) = cur {
                    if ns != orig_source_start || nl != orig_length || no != orig_track_offset {
                        app.history
                            .push_recorded(stem_core::history::Action::TrimClip {
                                track_id,
                                clip_id,
                                old_source_start: orig_source_start,
                                old_length: orig_length,
                                old_track_offset: orig_track_offset,
                                new_source_start: ns,
                                new_length: nl,
                                new_track_offset: no,
                            });
                    }
                }
                mutated = true;
            } else if let Some(DragOp::TrimRight {
                clip_id,
                track_id,
                orig_length,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| (c.source_start_frame, c.length_frames, c.track_offset_frames));
                if let Some((ns, nl, no)) = cur {
                    if nl != orig_length {
                        app.history
                            .push_recorded(stem_core::history::Action::TrimClip {
                                track_id,
                                clip_id,
                                old_source_start: ns,
                                old_length: orig_length,
                                old_track_offset: no,
                                new_source_start: ns,
                                new_length: nl,
                                new_track_offset: no,
                            });
                    }
                }
                mutated = true;
            } else if let Some(DragOp::FadeIn {
                clip_id,
                track_id,
                orig_fade_in,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| c.fade_in_frames);
                if cur.is_some_and(|v| v != orig_fade_in) {
                    let mut before = app.project.tracks.clone();
                    if let Some(bt) = before.iter_mut().find(|t| t.id == track_id) {
                        if let Some(bc) = bt.clips.iter_mut().find(|c| c.id == clip_id) {
                            bc.fade_in_frames = orig_fade_in;
                        }
                    }
                    app.history
                        .push_recorded(stem_core::history::Action::ProjectTracksEdit {
                            label: "Fade In",
                            before,
                            after: app.project.tracks.clone(),
                        });
                }
                mutated = true;
            } else if let Some(DragOp::FadeOut {
                clip_id,
                track_id,
                orig_fade_out,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| c.fade_out_frames);
                if cur.is_some_and(|v| v != orig_fade_out) {
                    let mut before = app.project.tracks.clone();
                    if let Some(bt) = before.iter_mut().find(|t| t.id == track_id) {
                        if let Some(bc) = bt.clips.iter_mut().find(|c| c.id == clip_id) {
                            bc.fade_out_frames = orig_fade_out;
                        }
                    }
                    app.history
                        .push_recorded(stem_core::history::Action::ProjectTracksEdit {
                            label: "Fade Out",
                            before,
                            after: app.project.tracks.clone(),
                        });
                }
                mutated = true;
            } else if let Some(DragOp::AdjustClipGain {
                clip_id,
                track_id,
                orig_gain_db,
                ..
            }) = app.timeline_state.drag_op
            {
                let cur = app
                    .project
                    .get_track(track_id)
                    .and_then(|t| t.clips.iter().find(|c| c.id == clip_id))
                    .map(|c| c.gain_db);
                if cur.is_some_and(|v| (v - orig_gain_db).abs() > 1e-4) {
                    let mut before = app.project.tracks.clone();
                    if let Some(bt) = before.iter_mut().find(|t| t.id == track_id) {
                        if let Some(bc) = bt.clips.iter_mut().find(|c| c.id == clip_id) {
                            bc.gain_db = orig_gain_db;
                        }
                    }
                    app.history
                        .push_recorded(stem_core::history::Action::ProjectTracksEdit {
                            label: "Clip Gain",
                            before,
                            after: app.project.tracks.clone(),
                        });
                }
                mutated = true;
            }
            if let Some(DragOp::SelectTimeRange { .. }) = app.timeline_state.drag_op {
                if let Some((s, e)) = app.timeline_state.time_selection {
                    // If it was just a click without moving (< 150 frames / ~3ms), clear the range selection
                    if e.saturating_sub(s) < 150 {
                        app.timeline_state.time_selection = None;
                    }
                }
            }
            app.timeline_state.drag_op = None;
            if mutated {
                app.sync_timeline_to_audio_engine();
            }
        }
    }

    // ---------------- Right-Click Context Menu ----------------
    response.context_menu(|ui| {
        let selected_clip = app.timeline_state.selected_clip_id;
        let has_selection = app.timeline_state.time_selection.is_some();
        let has_clipboard = app.timeline_state.timeline_clip_clipboard.is_some();

        if let Some(clip_id) = selected_clip {
            if ui.button(app.i18n.t("ctx-open-in-waveform")).clicked() {
                app.action_open_clip_in_waveform_editor(clip_id);
                ui.close_menu();
            }
            ui.separator();

            if ui.button(app.i18n.t("ctx-split-at-playhead")).clicked() {
                app.action_split_selected_clip();
                ui.close_menu();
            }
            if has_selection && ui.button(app.i18n.t("ctx-split-at-selection")).clicked() {
                app.action_split_at_selection();
                ui.close_menu();
            }
            ui.separator();

            if ui.button(app.i18n.t("waveform-btn-cut")).clicked() {
                app.action_cut_selected_clip();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("waveform-btn-copy")).clicked() {
                app.action_copy_selected_clip();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("ctx-duplicate-clip")).clicked() {
                app.action_duplicate_selected_clip();
                ui.close_menu();
            }
            if has_clipboard && ui.button(app.i18n.t("waveform-btn-paste")).clicked() {
                app.action_paste_clip();
                ui.close_menu();
            }
            if ui.button(app.i18n.t("waveform-btn-delete")).clicked() {
                app.action_delete_selected_clip();
                ui.close_menu();
            }
            ui.separator();

            ui.menu_button(app.i18n.t("ctx-fade-in"), |ui| {
                for (label, ms) in [("50 ms", 50.0), ("250 ms", 250.0), ("1000 ms", 1000.0)] {
                    if ui.button(label).clicked() {
                        app.action_fade_in_selected_clip(ms);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button(app.i18n.t("ctx-fade-clear")).clicked() {
                    app.action_fade_in_selected_clip(0.0);
                    ui.close_menu();
                }
            });

            ui.menu_button(app.i18n.t("ctx-fade-out"), |ui| {
                for (label, ms) in [("50 ms", 50.0), ("250 ms", 250.0), ("1000 ms", 1000.0)] {
                    if ui.button(label).clicked() {
                        app.action_fade_out_selected_clip(ms);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button(app.i18n.t("ctx-fade-clear")).clicked() {
                    app.action_fade_out_selected_clip(0.0);
                    ui.close_menu();
                }
            });

            if has_selection {
                ui.separator();
                if ui.button(app.i18n.t("ctx-silence-selection")).clicked() {
                    app.action_silence_selection();
                    ui.close_menu();
                }
                if ui.button(app.i18n.t("ctx-ripple-delete")).clicked() {
                    app.action_ripple_delete_selection();
                    ui.close_menu();
                }
            }

            ui.separator();
            if ui.button(app.i18n.t("ctx-separate-stems")).clicked() {
                app.stem_separation_state.show = true;
                ui.close_menu();
            }
        } else {
            if has_clipboard && ui.button(app.i18n.t("waveform-btn-paste")).clicked() {
                app.action_paste_clip();
                ui.close_menu();
            }
            if has_selection {
                if ui.button(app.i18n.t("ctx-silence-selection")).clicked() {
                    app.action_silence_selection();
                    ui.close_menu();
                }
                if ui.button(app.i18n.t("ctx-ripple-delete")).clicked() {
                    app.action_ripple_delete_selection();
                    ui.close_menu();
                }
                ui.separator();
            }
            if ui.button(app.i18n.t("ctx-add-track")).clicked() {
                let count = app.project.tracks.len() + 1;
                app.action_add_track(format!("Track {}", count));
                ui.close_menu();
            }
            if ui.button(app.i18n.t("ctx-drop-marker")).clicked() {
                app.action_drop_marker(None);
                ui.close_menu();
            }
        }
    });

    // ---------------- Drag & Drop Audio Import ----------------
    let hovered_files_count = ui.input(|i| i.raw.hovered_files.len());
    if hovered_files_count > 0 && mouse_pos.is_some_and(|mp| canvas_rect.contains(mp)) {
        painter.rect_stroke(
            canvas_rect.shrink(2.0),
            CornerRadius::same(6),
            Stroke::new(2.0_f32, Color32::from_rgb(80, 190, 255)),
            egui::StrokeKind::Middle,
        );
        painter.text(
            canvas_rect.center(),
            egui::Align2::CENTER_CENTER,
            app.i18n.t("drop-audio-hint"),
            egui::FontId::proportional(15.0),
            Color32::from_rgb(160, 220, 255),
        );
    }
    let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
    if !dropped_files.is_empty() {
        let mut target_track_idx = 0usize;
        let mut target_offset_frames = 0u64;
        if let Some(mp) = mouse_pos {
            if canvas_rect.contains(mp) {
                let rel_y = mp.y - tracks_top + app.timeline_state.scroll_y;
                for (t_idx, &y_off) in track_y_offsets.iter().enumerate() {
                    if rel_y >= y_off && rel_y < y_off + track_heights[t_idx] {
                        target_track_idx = t_idx;
                        break;
                    }
                }
                let raw_frame = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                    * sample_rate as f32)
                    .round() as u64;
                target_offset_frames = stem_core::project::snap_frame(
                    raw_frame,
                    sample_rate,
                    app.project.tempo_bpm,
                    app.project.time_signature.0,
                    app.timeline_state.snap_mode,
                );
            }
        }
        for df in dropped_files {
            if let Some(path) = df.path {
                app.import_audio_path(path, Some(target_track_idx), Some(target_offset_frames));
            }
        }
    }
}
