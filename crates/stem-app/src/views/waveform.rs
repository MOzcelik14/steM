//! Single-file Waveform Editor Canvas View
//!
//! Provides sample-accurate non-destructive editing, time-range selection with edge handles,
//! zero-crossing snapping, channel-split waveform rendering with peak pyramid caching ($O(1)$ query),
//! numeric selection timecode readouts, and playback.

use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_core::peaks::PeakPyramid;
use stem_core::waveform::find_nearest_zero_crossing;

use crate::app::StemApp;
use crate::state::WaveformDragOp;

pub fn render_waveform_editor(app: &mut StemApp, ui: &mut egui::Ui) {
    let sample_rate = app.project.sample_rate;

    // -------------------------------------------------------------------------
    // Top Waveform Editor Toolbar
    // -------------------------------------------------------------------------
    ui.horizontal(|ui| {
        if ui
            .button(format!("⮜ {}", app.i18n.t("view-mode-multitrack")))
            .on_hover_text("Return to Multitrack timeline (Key: 1)")
            .clicked()
        {
            app.action_switch_to_multitrack();
        }

        ui.separator();

        if let Some(doc) = &app.waveform_state.document {
            ui.label(
                egui::RichText::new(&app.waveform_state.file_name)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 180, 255)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "({} ch, {} frames)",
                    doc.channels,
                    doc.total_frames()
                ))
                .size(11.0)
                .color(egui::Color32::GRAY),
            );
        } else {
            ui.label(
                egui::RichText::new(app.i18n.t("waveform-title"))
                    .strong()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.separator();

        // Editing Action Buttons
        let has_doc = app.waveform_state.document.is_some();
        let has_sel = app.waveform_state.selection.is_some();
        let has_clip = app.clipboard.is_some();
        let can_undo = app.waveform_state.history.can_undo();
        let can_redo = app.waveform_state.history.can_redo();

        if ui
            .add_enabled(
                has_doc && can_undo,
                egui::Button::new(format!("↶ {}", app.i18n.t("menu-edit-undo"))),
            )
            .on_hover_text("Undo (Ctrl+Z)")
            .clicked()
        {
            app.action_waveform_undo();
        }

        if ui
            .add_enabled(
                has_doc && can_redo,
                egui::Button::new(format!("↷ {}", app.i18n.t("menu-edit-redo"))),
            )
            .on_hover_text("Redo (Ctrl+Y)")
            .clicked()
        {
            app.action_waveform_redo();
        }

        ui.separator();

        if ui
            .add_enabled(
                has_doc && has_sel,
                egui::Button::new(app.i18n.t("waveform-btn-cut")),
            )
            .on_hover_text("Cut selection to clipboard (Ctrl+X)")
            .clicked()
        {
            app.action_waveform_cut();
        }

        if ui
            .add_enabled(
                has_doc && has_sel,
                egui::Button::new(app.i18n.t("waveform-btn-copy")),
            )
            .on_hover_text("Copy selection to clipboard (Ctrl+C)")
            .clicked()
        {
            app.action_waveform_copy();
        }

        if ui
            .add_enabled(
                has_doc && has_clip,
                egui::Button::new(app.i18n.t("waveform-btn-paste")),
            )
            .on_hover_text("Paste from clipboard (Ctrl+V)")
            .clicked()
        {
            app.action_waveform_paste();
        }

        if ui
            .add_enabled(
                has_doc && has_sel,
                egui::Button::new(app.i18n.t("waveform-btn-delete")),
            )
            .on_hover_text("Delete selection (Delete)")
            .clicked()
        {
            app.action_waveform_delete();
        }

        if ui
            .add_enabled(
                has_doc && has_sel,
                egui::Button::new(app.i18n.t("waveform-btn-trim")),
            )
            .on_hover_text("Trim to selection (Ctrl+T)")
            .clicked()
        {
            app.action_waveform_trim();
        }

        if ui
            .add_enabled(
                has_doc,
                egui::Button::new(app.i18n.t("waveform-btn-silence")),
            )
            .on_hover_text("Insert 1 second of silence at cursor or selection (Ctrl+I)")
            .clicked()
        {
            app.action_waveform_insert_silence();
        }

        if ui
            .add_enabled(
                has_doc,
                egui::Button::new(app.i18n.t("waveform-btn-reverse")),
            )
            .on_hover_text("Reverse selected range or entire document")
            .clicked()
        {
            app.action_waveform_reverse();
        }

        if ui
            .add_enabled(
                has_doc,
                egui::Button::new(app.i18n.t("waveform-btn-invert")),
            )
            .on_hover_text("Invert phase of selected range or entire document")
            .clicked()
        {
            app.action_waveform_invert_phase();
        }

        ui.separator();

        // Fade In with Curve Submenu
        ui.add_enabled_ui(has_doc, |ui| {
            ui.menu_button(format!("◢ {}", app.i18n.t("waveform-btn-fade-in")), |ui| {
                if ui.button(app.i18n.t("waveform-fade-linear")).clicked() {
                    app.action_waveform_fade_in(stem_core::waveform::FadeCurve::Linear);
                    ui.close_menu();
                }
                if ui.button(app.i18n.t("waveform-fade-log")).clicked() {
                    app.action_waveform_fade_in(stem_core::waveform::FadeCurve::Logarithmic);
                    ui.close_menu();
                }
                if ui.button(app.i18n.t("waveform-fade-exp")).clicked() {
                    app.action_waveform_fade_in(stem_core::waveform::FadeCurve::Exponential);
                    ui.close_menu();
                }
                if ui.button(app.i18n.t("waveform-fade-scurve")).clicked() {
                    app.action_waveform_fade_in(stem_core::waveform::FadeCurve::SCurve);
                    ui.close_menu();
                }
            });
        });

        // Fade Out with Curve Submenu
        ui.add_enabled_ui(has_doc, |ui| {
            ui.menu_button(
                format!("◣ {}", app.i18n.t("waveform-btn-fade-out")),
                |ui| {
                    if ui.button(app.i18n.t("waveform-fade-linear")).clicked() {
                        app.action_waveform_fade_out(stem_core::waveform::FadeCurve::Linear);
                        ui.close_menu();
                    }
                    if ui.button(app.i18n.t("waveform-fade-log")).clicked() {
                        app.action_waveform_fade_out(stem_core::waveform::FadeCurve::Logarithmic);
                        ui.close_menu();
                    }
                    if ui.button(app.i18n.t("waveform-fade-exp")).clicked() {
                        app.action_waveform_fade_out(stem_core::waveform::FadeCurve::Exponential);
                        ui.close_menu();
                    }
                    if ui.button(app.i18n.t("waveform-fade-scurve")).clicked() {
                        app.action_waveform_fade_out(stem_core::waveform::FadeCurve::SCurve);
                        ui.close_menu();
                    }
                },
            );
        });

        // Normalize button
        if ui
            .add_enabled(
                has_doc,
                egui::Button::new(format!("⇪ {}", app.i18n.t("waveform-btn-normalize"))),
            )
            .on_hover_text("Peak normalize audio range to target dBFS")
            .clicked()
        {
            app.waveform_state.show_normalize_dialog = true;
        }

        // Effects Rack button
        if ui
            .button(format!("🎛 {}", app.i18n.t("track-effects-rack")))
            .on_hover_text("Open Insert Effects Rack with live preview")
            .clicked()
        {
            app.show_effects_dialog = true;
        }

        ui.separator();

        // Snap to Zero-Crossing Toggle
        let mut snap_zc = app.waveform_state.snap_to_zero_crossing;
        if ui
            .selectable_label(
                snap_zc,
                format!("∿ {}", app.i18n.t("waveform-snap-zero-crossing")),
            )
            .on_hover_text("Automatically snap selection boundaries to the nearest zero-crossing")
            .clicked()
        {
            snap_zc = !snap_zc;
            app.waveform_state.snap_to_zero_crossing = snap_zc;
        }

        // Loop Selection Toggle
        let mut loop_sel = app.waveform_state.loop_selection;
        if ui
            .selectable_label(
                loop_sel,
                format!("🔁 {}", app.i18n.t("waveform-loop-selection")),
            )
            .on_hover_text("Loop playback within the active time selection")
            .clicked()
        {
            loop_sel = !loop_sel;
            app.waveform_state.loop_selection = loop_sel;
            if app.is_playing {
                app.sync_waveform_playback_loop();
            }
        }

        // Apply to Project Button (if opened from clip)
        if app.waveform_state.active_clip_info.is_some() {
            ui.separator();
            if ui
                .button(format!(
                    "💾 {}",
                    app.i18n.t("waveform-btn-apply-to-project")
                ))
                .on_hover_text("Save edited waveform back to project clip")
                .clicked()
            {
                app.action_waveform_apply_to_project();
            }
        }
    });

    ui.separator();

    // -------------------------------------------------------------------------
    // Numeric Selection Info Bar
    // -------------------------------------------------------------------------
    ui.horizontal(|ui| {
        if let Some((s_frame, e_frame)) = app.waveform_state.selection {
            let dur_frame = e_frame.saturating_sub(s_frame);
            let s_str = app
                .timecode_mode
                .format_time(s_frame as f64 / sample_rate as f64, sample_rate);
            let e_str = app
                .timecode_mode
                .format_time(e_frame as f64 / sample_rate as f64, sample_rate);
            let d_str = app
                .timecode_mode
                .format_time(dur_frame as f64 / sample_rate as f64, sample_rate);

            ui.label(
                egui::RichText::new(app.i18n.t_arg("waveform-selection-start", "time", s_str))
                    .monospace()
                    .size(12.0)
                    .color(egui::Color32::from_rgb(180, 220, 255)),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(app.i18n.t_arg("waveform-selection-end", "time", e_str))
                    .monospace()
                    .size(12.0)
                    .color(egui::Color32::from_rgb(180, 220, 255)),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(app.i18n.t_arg("waveform-selection-duration", "time", d_str))
                    .monospace()
                    .size(12.0)
                    .color(egui::Color32::from_rgb(255, 215, 120)),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(format!("({} spl)", dur_frame))
                    .size(11.0)
                    .color(egui::Color32::GRAY),
            );

            if ui
                .small_button("✕")
                .on_hover_text("Clear selection")
                .clicked()
            {
                app.waveform_state.selection = None;
            }
        } else {
            ui.label(
                egui::RichText::new(
                    "No active selection (Click and drag on waveform to select a time range)",
                )
                .size(11.0)
                .italics()
                .color(egui::Color32::DARK_GRAY),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("🔍-")
                .on_hover_text("Zoom Out (Ctrl+Minus)")
                .clicked()
            {
                app.waveform_state.pixels_per_second =
                    (app.waveform_state.pixels_per_second * 0.75).max(5.0);
            }
            if ui
                .button("🔍+")
                .on_hover_text("Zoom In (Ctrl+Plus)")
                .clicked()
            {
                app.waveform_state.pixels_per_second =
                    (app.waveform_state.pixels_per_second * 1.33).min(5000.0);
            }
            if ui.button("1:1").on_hover_text("Reset Zoom").clicked() {
                app.waveform_state.pixels_per_second = 120.0;
                app.waveform_state.scroll_x = 0.0;
            }
        });
    });

    ui.separator();

    // -------------------------------------------------------------------------
    // Main Waveform Canvas
    // -------------------------------------------------------------------------
    let doc = match &app.waveform_state.document {
        Some(d) => d.clone(),
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    egui::RichText::new(app.i18n.t("waveform-no-file"))
                        .size(14.0)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(16.0);
                if ui
                    .button(format!("📂 {}", app.i18n.t("menu-file-import-audio")))
                    .clicked()
                {
                    app.action_open_file_in_waveform_editor();
                }
            });
            return;
        }
    };

    let total_frames = doc.total_frames();
    let num_channels = doc.channels;

    let available_size = ui.available_size();
    let (response, painter) = ui.allocate_painter(available_size, egui::Sense::click_and_drag());
    let canvas_rect = response.rect;

    let pps = app.waveform_state.pixels_per_second;
    let scroll_x = app.waveform_state.scroll_x;

    // Handle mouse wheel zoom & scroll
    let scroll_delta = ui.input(|i| i.raw_scroll_delta);
    if response.hovered() && scroll_delta != egui::Vec2::ZERO {
        if ui.input(|i| i.modifiers.command || i.modifiers.ctrl) {
            // Zoom centered around mouse
            let mouse_x = ui
                .input(|i| i.pointer.hover_pos())
                .unwrap_or(canvas_rect.center())
                .x;
            let time_under_mouse = (mouse_x - canvas_rect.min.x + scroll_x) / pps;
            let zoom_factor = if scroll_delta.y > 0.0 { 1.15 } else { 0.85 };
            let new_pps = (pps * zoom_factor).clamp(5.0, 5000.0);
            let new_scroll_x =
                (time_under_mouse * new_pps - (mouse_x - canvas_rect.min.x)).max(0.0);
            app.waveform_state.pixels_per_second = new_pps;
            app.waveform_state.scroll_x = new_scroll_x;
        } else {
            // Horizontal scroll
            let dx = scroll_delta.x + scroll_delta.y;
            app.waveform_state.scroll_x = (scroll_x - dx * 2.0).max(0.0);
        }
    }

    // -------------------------------------------------------------------------
    // Time Ruler
    // -------------------------------------------------------------------------
    let ruler_height = 24.0;
    let ruler_rect = Rect::from_min_size(canvas_rect.min, vec2(canvas_rect.width(), ruler_height));
    painter.rect_filled(ruler_rect, Rounding::ZERO, Color32::from_rgb(20, 22, 26));
    painter.line_segment(
        [
            pos2(ruler_rect.min.x, ruler_rect.max.y),
            pos2(ruler_rect.max.x, ruler_rect.max.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 52, 60)),
    );

    let start_sec = (scroll_x / pps).max(0.0);
    let visible_sec = canvas_rect.width() / pps;
    let end_sec = start_sec + visible_sec;

    let step_sec = if pps > 200.0 {
        0.1
    } else if pps > 80.0 {
        0.5
    } else if pps > 30.0 {
        1.0
    } else if pps > 10.0 {
        5.0
    } else {
        10.0
    };

    let first_tick = (start_sec / step_sec).floor() * step_sec;
    let mut t = first_tick;
    while t <= end_sec + step_sec {
        let x = canvas_rect.min.x + (t * pps) - scroll_x;
        if x >= canvas_rect.min.x && x <= canvas_rect.max.x {
            painter.line_segment(
                [pos2(x, ruler_rect.max.y - 8.0), pos2(x, ruler_rect.max.y)],
                Stroke::new(1.0_f32, Color32::from_rgb(110, 115, 125)),
            );
            let time_str = app.timecode_mode.format_time(t as f64, sample_rate);
            painter.text(
                pos2(x + 3.0, ruler_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                time_str,
                egui::FontId::monospace(10.0),
                Color32::from_rgb(170, 175, 185),
            );
        }
        t += step_sec;
    }

    // -------------------------------------------------------------------------
    // Channel-Split Waveform Lanes
    // -------------------------------------------------------------------------
    let wave_top = ruler_rect.max.y;
    let wave_height = (canvas_rect.height() - ruler_height).max(40.0);
    let channel_height = wave_height / num_channels.max(1) as f32;

    let channel_colors = [
        Color32::from_rgb(100, 190, 255), // Channel 0 (Left / Mono)
        Color32::from_rgb(80, 220, 180),  // Channel 1 (Right)
        Color32::from_rgb(220, 160, 255), // Channel 2
        Color32::from_rgb(255, 180, 100), // Channel 3
    ];

    for ch in 0..num_channels {
        let ch_top = wave_top + ch as f32 * channel_height;
        let ch_rect = Rect::from_min_size(
            pos2(canvas_rect.min.x, ch_top),
            vec2(canvas_rect.width(), channel_height),
        );

        // Lane background
        let bg_color = if ch % 2 == 0 {
            Color32::from_rgb(16, 18, 22)
        } else {
            Color32::from_rgb(20, 22, 26)
        };
        painter.rect_filled(ch_rect, Rounding::ZERO, bg_color);

        // Center line (0.0 V)
        let mid_y = ch_rect.center().y;
        painter.line_segment(
            [pos2(ch_rect.min.x, mid_y), pos2(ch_rect.max.x, mid_y)],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55)),
        );

        // Channel boundary line
        painter.line_segment(
            [
                pos2(ch_rect.min.x, ch_rect.max.y),
                pos2(ch_rect.max.x, ch_rect.max.y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(35, 38, 45)),
        );

        // Channel label (e.g. L / R)
        let ch_label = match (num_channels, ch) {
            (1, 0) => "Mono",
            (2, 0) => "L (Left)",
            (2, 1) => "R (Right)",
            _ => "Ch",
        };
        painter.text(
            pos2(ch_rect.min.x + 8.0, ch_rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            ch_label,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(140, 145, 155),
        );

        // Draw Waveform Peaks
        if let Some(pyramid) = &app.waveform_state.pyramid {
            render_channel_peaks(
                &painter,
                ch_rect,
                ch,
                pyramid,
                scroll_x,
                pps,
                sample_rate,
                channel_colors[ch % channel_colors.len()],
            );
        }
    }

    // -------------------------------------------------------------------------
    // Selection Region & Drag Handles
    // -------------------------------------------------------------------------
    let mouse_pos = response.hover_pos();
    let is_mouse_down = ui.input(|i| i.pointer.primary_down());

    if let Some((s_frame, e_frame)) = app.waveform_state.selection {
        let s_x = canvas_rect.min.x + (s_frame as f32 / sample_rate as f32 * pps) - scroll_x;
        let e_x = canvas_rect.min.x + (e_frame as f32 / sample_rate as f32 * pps) - scroll_x;

        let left_x = s_x.min(e_x);
        let right_x = s_x.max(e_x);

        let sel_rect = Rect::from_min_max(
            pos2(left_x.max(canvas_rect.min.x), wave_top),
            pos2(right_x.min(canvas_rect.max.x), canvas_rect.max.y),
        );

        if sel_rect.width() > 0.0 {
            // Shaded selection box
            painter.rect_filled(
                sel_rect,
                Rounding::ZERO,
                Color32::from_rgba_premultiplied(100, 180, 255, 45),
            );

            // Left boundary line & handle
            let left_handle_rect =
                Rect::from_min_size(pos2(left_x - 4.0, wave_top), vec2(8.0, wave_height));
            painter.line_segment(
                [pos2(left_x, wave_top), pos2(left_x, canvas_rect.max.y)],
                Stroke::new(1.5_f32, Color32::from_rgb(120, 200, 255)),
            );

            // Right boundary line & handle
            let right_handle_rect =
                Rect::from_min_size(pos2(right_x - 4.0, wave_top), vec2(8.0, wave_height));
            painter.line_segment(
                [pos2(right_x, wave_top), pos2(right_x, canvas_rect.max.y)],
                Stroke::new(1.5_f32, Color32::from_rgb(120, 200, 255)),
            );

            // Check handle hover cursor
            if let Some(mp) = mouse_pos {
                if left_handle_rect.contains(mp) || right_handle_rect.contains(mp) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Playhead Line
    // -------------------------------------------------------------------------
    let playhead = if app.is_playing {
        app.telemetry.playhead_frame
    } else {
        app.waveform_state.playhead_frame
    };

    let playhead_x = canvas_rect.min.x + (playhead as f32 / sample_rate as f32 * pps) - scroll_x;
    if playhead_x >= canvas_rect.min.x && playhead_x <= canvas_rect.max.x {
        painter.line_segment(
            [
                pos2(playhead_x, ruler_rect.min.y),
                pos2(playhead_x, canvas_rect.max.y),
            ],
            Stroke::new(1.5_f32, Color32::from_rgb(255, 80, 80)),
        );
        // Playhead head icon
        painter.circle_filled(
            pos2(playhead_x, ruler_rect.min.y + 4.0),
            4.0,
            Color32::from_rgb(255, 80, 80),
        );
    }

    // -------------------------------------------------------------------------
    // Mouse Interaction (Click-drag Selection, Edge Resize, Playhead Seek)
    // -------------------------------------------------------------------------
    if is_mouse_down {
        if let Some(mp) = mouse_pos {
            if canvas_rect.contains(mp) {
                let frame_at_mouse = ((mp.x - canvas_rect.min.x + scroll_x).max(0.0) / pps
                    * sample_rate as f32)
                    .round() as u64;
                let frame_at_mouse = frame_at_mouse.min(total_frames);

                let snap = app.waveform_state.snap_to_zero_crossing;
                let snapped_frame = if snap {
                    if let Some(planar) = &app.waveform_state.cached_planar {
                        if !planar.is_empty() {
                            find_nearest_zero_crossing(&planar[0], frame_at_mouse, 256)
                        } else {
                            frame_at_mouse
                        }
                    } else {
                        frame_at_mouse
                    }
                } else {
                    frame_at_mouse
                };

                if app.waveform_state.drag_op.is_none() {
                    // Start drag operation
                    if mp.y < wave_top {
                        // Click in ruler -> Scrub Playhead
                        app.waveform_state.drag_op = Some(WaveformDragOp::ScrubPlayhead);
                        app.action_seek_waveform(snapped_frame);
                    } else if let Some((s_frame, e_frame)) = app.waveform_state.selection {
                        let s_x = canvas_rect.min.x + (s_frame as f32 / sample_rate as f32 * pps)
                            - scroll_x;
                        let e_x = canvas_rect.min.x + (e_frame as f32 / sample_rate as f32 * pps)
                            - scroll_x;
                        let left_x = s_x.min(e_x);
                        let right_x = s_x.max(e_x);

                        if (mp.x - left_x).abs() <= 6.0 {
                            app.waveform_state.drag_op = Some(WaveformDragOp::DragLeftEdge);
                        } else if (mp.x - right_x).abs() <= 6.0 {
                            app.waveform_state.drag_op = Some(WaveformDragOp::DragRightEdge);
                        } else {
                            // Start new selection
                            app.waveform_state.drag_op = Some(WaveformDragOp::Selecting {
                                anchor_frame: snapped_frame,
                            });
                            app.waveform_state.selection = Some((snapped_frame, snapped_frame));
                            app.action_seek_waveform(snapped_frame);
                        }
                    } else {
                        // Start new selection
                        app.waveform_state.drag_op = Some(WaveformDragOp::Selecting {
                            anchor_frame: snapped_frame,
                        });
                        app.waveform_state.selection = Some((snapped_frame, snapped_frame));
                        app.action_seek_waveform(snapped_frame);
                    }
                } else {
                    // Continue active drag operation
                    match app.waveform_state.drag_op {
                        Some(WaveformDragOp::Selecting { anchor_frame }) => {
                            let s = anchor_frame.min(snapped_frame);
                            let e = anchor_frame.max(snapped_frame);
                            app.waveform_state.selection = Some((s, e));
                        }
                        Some(WaveformDragOp::DragLeftEdge) => {
                            if let Some((_, e_frame)) = app.waveform_state.selection {
                                let new_s = snapped_frame.min(e_frame);
                                app.waveform_state.selection = Some((new_s, e_frame));
                            }
                        }
                        Some(WaveformDragOp::DragRightEdge) => {
                            if let Some((s_frame, _)) = app.waveform_state.selection {
                                let new_e = snapped_frame.max(s_frame).min(total_frames);
                                app.waveform_state.selection = Some((s_frame, new_e));
                            }
                        }
                        Some(WaveformDragOp::ScrubPlayhead) => {
                            app.action_seek_waveform(snapped_frame);
                        }
                        None => {}
                    }
                }
            }
        }
    } else {
        // Mouse released
        if let Some(op) = app.waveform_state.drag_op {
            if let WaveformDragOp::Selecting { anchor_frame: _ } = op {
                if let Some((s, e)) = app.waveform_state.selection {
                    if s == e {
                        app.waveform_state.selection = None;
                    }
                }
            }
            app.waveform_state.drag_op = None;
        }
    }

    render_normalize_dialog(app, ui.ctx());
}

/// Renders channel peaks for a given channel using the multi-resolution PeakPyramid.
#[allow(clippy::too_many_arguments)]
fn render_channel_peaks(
    painter: &egui::Painter,
    ch_rect: Rect,
    channel_idx: usize,
    pyramid: &PeakPyramid,
    scroll_x: f32,
    pps: f32,
    sample_rate: u32,
    color: Color32,
) {
    let visible_start_sec = (scroll_x / pps).max(0.0);
    let visible_duration = ch_rect.width() / pps;
    let visible_end_sec = visible_start_sec + visible_duration;

    let start_frame = (visible_start_sec * sample_rate as f32) as u64;
    let end_frame = (visible_end_sec * sample_rate as f32).ceil() as u64;

    let target_points = (ch_rect.width() as usize).clamp(10, 3000);
    let peaks = pyramid.query_channel_peaks(channel_idx, start_frame, end_frame, target_points);

    if peaks.is_empty() {
        return;
    }

    let mid_y = ch_rect.center().y;
    let half_h = (ch_rect.height() * 0.44).max(4.0);
    let step_x = ch_rect.width() / peaks.len() as f32;

    for (i, p) in peaks.iter().enumerate() {
        let x = ch_rect.min.x + i as f32 * step_x;
        let top_y = mid_y - (p.max.clamp(-1.0, 1.0) * half_h);
        let bot_y = mid_y - (p.min.clamp(-1.0, 1.0) * half_h);

        painter.line_segment(
            [pos2(x, top_y), pos2(x, bot_y.max(top_y + 1.0))],
            Stroke::new(1.0_f32, color),
        );
    }
}

fn render_normalize_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.waveform_state.show_normalize_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("dialog-normalize-title"))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(app.i18n.t("dialog-normalize-target"));
                    ui.add(
                        egui::DragValue::new(&mut app.waveform_state.normalize_target_db)
                            .range(-30.0..=0.0)
                            .speed(0.1)
                            .suffix(" dBFS"),
                    );
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(app.i18n.t("dialog-normalize-btn-apply"))
                        .clicked()
                    {
                        let target = app.waveform_state.normalize_target_db;
                        app.action_waveform_normalize(target);
                        app.waveform_state.show_normalize_dialog = false;
                    }

                    if ui.button(app.i18n.t("export-btn-cancel")).clicked() {
                        app.waveform_state.show_normalize_dialog = false;
                    }
                });
            });
        });

    if !open {
        app.waveform_state.show_normalize_dialog = false;
    }
}
