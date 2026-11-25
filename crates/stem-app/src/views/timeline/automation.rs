use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};

pub(super) struct AutomationLaneRenderCtx<'a> {
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
    pub default_curve: stem_core::project::AutomationCurve,
}

/// Renders interactive breakpoint automation lane on the timeline canvas.
pub(super) fn render_automation_lane(
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

    let (val_min, val_max, val_zero) = ctx.target.value_range();

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
        stem_core::project::AutomationTarget::PluginParam { .. } => {
            Color32::from_rgb(190, 140, 255)
        }
        stem_core::project::AutomationTarget::SendLevel(_) => Color32::from_rgb(255, 190, 80),
        stem_core::project::AutomationTarget::EqFreq(_)
        | stem_core::project::AutomationTarget::EqGain(_)
        | stem_core::project::AutomationTarget::EqQ(_) => Color32::from_rgb(80, 230, 220),
        stem_core::project::AutomationTarget::CompressorThreshold
        | stem_core::project::AutomationTarget::CompressorRatio => Color32::from_rgb(255, 130, 110),
        stem_core::project::AutomationTarget::ReverbMix
        | stem_core::project::AutomationTarget::ReverbRoomSize => Color32::from_rgb(220, 130, 255),
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
                    let steps = 24;
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
        match pt.curve {
            stem_core::project::AutomationCurve::Linear => {
                painter.circle_filled(pt_pos, radius, fill);
                painter.circle_stroke(pt_pos, radius, Stroke::new(1.0_f32, Color32::BLACK));
            }
            stem_core::project::AutomationCurve::Bezier => {
                let diamond = vec![
                    pos2(pt_pos.x, pt_pos.y - radius),
                    pos2(pt_pos.x + radius, pt_pos.y),
                    pos2(pt_pos.x, pt_pos.y + radius),
                    pos2(pt_pos.x - radius, pt_pos.y),
                ];
                painter.add(egui::Shape::convex_polygon(
                    diamond,
                    fill,
                    Stroke::new(1.0_f32, Color32::BLACK),
                ));
            }
            stem_core::project::AutomationCurve::Step => {
                let sq = Rect::from_center_size(pt_pos, vec2(radius * 1.8, radius * 1.8));
                painter.rect_filled(sq, Rounding::ZERO, fill);
                painter.rect_stroke(sq, Rounding::ZERO, Stroke::new(1.0_f32, Color32::BLACK));
            }
        }
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
                lane.add_point(frame, val, ctx.default_curve);
                lane.enabled = true;
                modified = true;
            }
        }
    }

    modified
}
