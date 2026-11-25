use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect};

/// Renders a vertical peak VU meter bar with green/amber/red studio thresholds.
pub fn render_vu_meter_bar(
    painter: &egui::Painter,
    rect: Rect,
    peak: f32,
    inset_x: f32,
    rounding: f32,
) {
    let radius = rounding.round().clamp(0.0, 255.0) as u8;
    painter.rect_filled(
        rect,
        CornerRadius::same(radius),
        Color32::from_rgb(18, 22, 30),
    );
    if peak <= 1e-5 {
        return;
    }

    let norm = ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0);
    if norm <= 0.0 {
        return;
    }

    let h = rect.height() * norm;
    let fill_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + inset_x, rect.max.y - h),
        Pos2::new(rect.max.x - inset_x, rect.max.y),
    );
    let col = if norm > 0.92 {
        Color32::from_rgb(235, 70, 70)
    } else if norm > 0.72 {
        Color32::from_rgb(235, 195, 65)
    } else {
        Color32::from_rgb(70, 210, 125)
    };
    painter.rect_filled(fill_rect, CornerRadius::same(radius), col);
}
