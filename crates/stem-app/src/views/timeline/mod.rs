//! Multi-track timeline view with smooth multi-resolution waveform visualization

mod automation;
mod canvas;
mod interactions;
mod track_headers;

use crate::app::StemApp;
use egui::{pos2, vec2, Color32, CornerRadius, Rect, Stroke};

pub(super) struct TimelineGeom {
    pub available_size: egui::Vec2,
    pub header_width: f32,
    pub ruler_height: f32,
    pub scrollbar_h: f32,
    pub sample_rate: u32,
    pub pps: f32,
    pub base_track_height: f32,
    pub track_spacing: f32,
    pub track_heights: Vec<f32>,
    pub track_y_offsets: Vec<f32>,
}

pub(super) struct CanvasFrame {
    pub canvas_rect: Rect,
    pub response: egui::Response,
    pub painter: egui::Painter,
    pub ruler_rect: Rect,
    pub tracks_top: f32,
    pub mouse_pos: Option<egui::Pos2>,
    pub is_mouse_down: bool,
    pub is_primary_clicked: bool,
    pub scroll_x: f32,
    pub start_time_sec: f32,
    pub end_time_sec: f32,
}

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

    let mut geom = TimelineGeom {
        available_size,
        header_width,
        ruler_height,
        scrollbar_h,
        sample_rate,
        pps,
        base_track_height,
        track_spacing,
        track_heights,
        track_y_offsets,
    };

    ui.horizontal(|ui| {
        let delete_track_id = track_headers::render_track_headers(app, ui, &geom);
        let Some(frame) = canvas::render_timeline_canvas(app, ui, &mut geom, delete_track_id)
        else {
            return;
        };
        interactions::render_overlays_and_interactions(app, ui, &geom, &frame);

        let canvas_rect = frame.canvas_rect;
        let painter = &frame.painter;
        let tracks_top = frame.tracks_top;
        let mouse_pos = frame.mouse_pos;
        let is_mouse_down = frame.is_mouse_down;
        // ---------------- Horizontal Scrollbar ----------------
        let h_scrollbar_rect = Rect::from_min_size(
            pos2(canvas_rect.min.x, canvas_rect.max.y - scrollbar_h),
            vec2(canvas_rect.width(), scrollbar_h),
        );
        painter.rect_filled(
            h_scrollbar_rect,
            CornerRadius::ZERO,
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
            painter.rect_filled(thumb_rect, CornerRadius::same(3), thumb_color);

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
                CornerRadius::ZERO,
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
            painter.rect_filled(thumb_rect, CornerRadius::same(3), thumb_color);

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
