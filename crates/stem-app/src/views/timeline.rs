//! Multi-track timeline view with smooth multi-resolution waveform visualization

use crate::app::StemApp;
use crate::state::DragOp;
use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_core::peaks::PeakPyramid;
use stem_core::project::Clip;
use stem_i18n::format_time_code;

pub fn render_timeline_view(app: &mut StemApp, ui: &mut egui::Ui) {
    let available_size = ui.available_size();
    let header_width = 200.0;
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

                    let (rect, _response) = ui.allocate_exact_size(
                        vec2(header_width - 8.0, track_height),
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
                                app.project.tracks[t_idx].armed = armed;
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

        // Draw time tick marks
        let scroll_x = app.timeline_state.scroll_x;
        let start_time_sec = (scroll_x / pps).max(0.0);
        let visible_duration = canvas_width / pps;
        let end_time_sec = start_time_sec + visible_duration;

        // Choose step size: 1s, 5s, 10s, 30s, 60s
        let step_sec = if pps > 100.0 {
            0.5
        } else if pps > 40.0 {
            1.0
        } else if pps > 15.0 {
            5.0
        } else {
            10.0
        };

        let first_tick = (start_time_sec / step_sec).floor() * step_sec;
        let mut t = first_tick;
        while t <= end_time_sec + step_sec {
            let x = canvas_rect.min.x + (t * pps) - scroll_x;
            if x >= canvas_rect.min.x && x <= canvas_rect.max.x {
                painter.line_segment(
                    [pos2(x, ruler_rect.max.y - 8.0), pos2(x, ruler_rect.max.y)],
                    Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 130)),
                );
                painter.text(
                    pos2(x + 3.0, ruler_rect.min.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    format_time_code(t as f64),
                    egui::FontId::monospace(10.0),
                    Color32::from_rgb(180, 180, 190),
                );
            }
            t += step_sec;
        }

        // ---------------- Track Waveform Lanes ----------------
        let tracks_top = canvas_rect.min.y + ruler_height;
        let track_height = app.timeline_state.track_height;

        let mouse_pos = response.hover_pos();
        let is_mouse_down = ui.input(|i| i.pointer.primary_down());
        let mut open_clip_in_waveform: Option<uuid::Uuid> = None;

        for (t_idx, track) in app.project.tracks.iter().enumerate() {
            let track_top = tracks_top + t_idx as f32 * (track_height + 4.0);
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
            for clip in &track.clips {
                let clip_x_start = canvas_rect.min.x
                    + (clip.track_offset_frames as f32 / sample_rate as f32 * pps)
                    - scroll_x;
                let clip_width = (clip.length_frames as f32 / sample_rate as f32 * pps).max(4.0);
                let clip_rect = Rect::from_min_size(
                    pos2(clip_x_start, track_top + 4.0),
                    vec2(clip_width, track_height - 8.0),
                );

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
                let left_handle_rect =
                    Rect::from_min_size(clip_rect.min, vec2(8.0, clip_rect.height()));
                let right_handle_rect = Rect::from_min_size(
                    pos2(clip_rect.max.x - 8.0, clip_rect.min.y),
                    vec2(8.0, clip_rect.height()),
                );
                let fade_in_handle_rect = Rect::from_min_size(clip_rect.min, vec2(12.0, 12.0));
                let fade_out_handle_rect = Rect::from_min_size(
                    pos2(clip_rect.max.x - 12.0, clip_rect.min.y),
                    vec2(12.0, 12.0),
                );

                // Draw subtle fade corner handles
                painter.rect_filled(
                    fade_in_handle_rect,
                    Rounding::same(2.0),
                    Color32::from_rgba_premultiplied(255, 200, 80, 180),
                );
                painter.rect_filled(
                    fade_out_handle_rect,
                    Rounding::same(2.0),
                    Color32::from_rgba_premultiplied(255, 200, 80, 180),
                );

                // Check for click / selection / drag start
                if let Some(mp) = mouse_pos {
                    if clip_rect.contains(mp) {
                        if fade_in_handle_rect.contains(mp) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                app.timeline_state.drag_op = Some(DragOp::FadeIn {
                                    clip_id: clip.id,
                                    track_id: track.id,
                                    orig_fade_in: clip.fade_in_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if fade_out_handle_rect.contains(mp) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                app.timeline_state.drag_op = Some(DragOp::FadeOut {
                                    clip_id: clip.id,
                                    track_id: track.id,
                                    orig_fade_out: clip.fade_out_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if left_handle_rect.contains(mp) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                app.timeline_state.drag_op = Some(DragOp::TrimLeft {
                                    clip_id: clip.id,
                                    track_id: track.id,
                                    orig_source_start: clip.source_start_frame,
                                    orig_track_offset: clip.track_offset_frames,
                                    orig_length: clip.length_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if right_handle_rect.contains(mp) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                app.timeline_state.drag_op = Some(DragOp::TrimRight {
                                    clip_id: clip.id,
                                    track_id: track.id,
                                    orig_length: clip.length_frames,
                                    drag_start_x: mp.x,
                                });
                            }
                        } else if ui.input(|i| {
                            i.pointer
                                .button_double_clicked(egui::PointerButton::Primary)
                        }) {
                            open_clip_in_waveform = Some(clip.id);
                        } else if ui.input(|i| i.pointer.primary_clicked()) {
                            app.timeline_state.selected_clip_id = Some(clip.id);
                            app.timeline_state.selected_track_id = Some(track.id);

                            // Start Move Clip drag operation
                            app.timeline_state.drag_op = Some(DragOp::MoveClip {
                                clip_id: clip.id,
                                track_id: track.id,
                                original_track_offset: clip.track_offset_frames,
                                drag_start_x: mp.x,
                            });
                        }
                    }
                }
            }
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
