//! Transport bar with playback controls, timecode display, and master VU meters

use crate::app::StemApp;
use egui::{Color32, Rect, Rounding, Stroke, Vec2};
use stem_i18n::{
    format_bars_beats, format_db, format_samples, format_smpte, format_time_code, TimecodeMode,
};

pub fn render_transport_bar(app: &mut StemApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

        // ----------------- Transport Buttons -----------------
        let is_playing = app.is_playing;

        // Play / Pause
        let play_text = if is_playing { "⏸" } else { "▶" };
        let play_btn = egui::Button::new(egui::RichText::new(play_text).size(16.0));
        if ui.add(play_btn).clicked() {
            app.action_toggle_play();
        }

        // Stop
        if ui.button(egui::RichText::new("⏹").size(16.0)).clicked() {
            app.action_stop();
        }

        // Record
        let is_recording = app.is_recording;
        let rec_color = if is_recording {
            Color32::from_rgb(255, 30, 30)
        } else {
            Color32::from_rgb(180, 50, 50)
        };
        let rec_btn = egui::Button::new(egui::RichText::new("⏺").size(16.0).color(rec_color));
        if ui.add(rec_btn).clicked() {
            app.action_toggle_record();
        }

        // Loop
        let mut loop_enabled = app.project.loop_enabled;
        if ui
            .selectable_label(loop_enabled, egui::RichText::new("🔁").size(14.0))
            .clicked()
        {
            loop_enabled = !loop_enabled;
            app.action_set_loop(loop_enabled);
        }

        ui.separator();

        // ----------------- Timecode Display -----------------
        let current_frame = app.telemetry.playhead_frame;
        let seconds = if app.project.sample_rate > 0 {
            current_frame as f64 / app.project.sample_rate as f64
        } else {
            0.0
        };

        let primary_timecode = match app.timecode_mode {
            TimecodeMode::TimeCode => format_time_code(seconds),
            TimecodeMode::Smpte => format_smpte(seconds, 30.0),
            TimecodeMode::BarsBeats => {
                format_bars_beats(seconds, app.project.tempo_bpm, app.project.time_signature)
            }
            TimecodeMode::Samples => format_samples(current_frame),
        };

        let mode_label = match app.timecode_mode {
            TimecodeMode::TimeCode => app.i18n.t("transport-mode-timecode"),
            TimecodeMode::Smpte => app.i18n.t("transport-mode-smpte"),
            TimecodeMode::BarsBeats => app.i18n.t("transport-mode-bars"),
            TimecodeMode::Samples => app.i18n.t("transport-mode-samples"),
        };

        if ui
            .group(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                ui.label(
                    egui::RichText::new(&primary_timecode)
                        .monospace()
                        .strong()
                        .size(16.0)
                        .color(Color32::from_rgb(100, 220, 100)),
                );
                ui.label(
                    egui::RichText::new(mode_label)
                        .monospace()
                        .size(11.0)
                        .color(Color32::GRAY),
                );
            })
            .response
            .clicked()
        {
            // Cycle timecode modes on click
            app.timecode_mode = match app.timecode_mode {
                TimecodeMode::TimeCode => TimecodeMode::BarsBeats,
                TimecodeMode::BarsBeats => TimecodeMode::Smpte,
                TimecodeMode::Smpte => TimecodeMode::Samples,
                TimecodeMode::Samples => TimecodeMode::TimeCode,
            };
        }

        ui.separator();

        // ----------------- Tempo & Signature -----------------
        ui.label(app.i18n.t("transport-tempo"));
        let mut tempo = app.project.tempo_bpm;
        if ui
            .add(
                egui::DragValue::new(&mut tempo)
                    .range(20.0..=300.0)
                    .speed(1.0),
            )
            .changed()
        {
            app.project.tempo_bpm = tempo;
        }

        ui.label(app.i18n.t("transport-time-signature"));
        ui.label(format!(
            "{}/{}",
            app.project.time_signature.0, app.project.time_signature.1
        ));

        // Effects Rack Quick Button
        if ui
            .button(
                egui::RichText::new(format!("🎛 {}", app.i18n.t("track-effects-rack"))).size(12.0),
            )
            .clicked()
        {
            app.show_effects_dialog = !app.show_effects_dialog;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ----------------- Master VU Meter -----------------
            render_master_vu_meter(app, ui);

            // Master Volume Slider
            ui.label(format_db(app.project.master_gain_db));
            let mut master_db = app.project.master_gain_db;
            if ui
                .add(
                    egui::Slider::new(&mut master_db, -60.0..=12.0)
                        .show_value(false)
                        .text(app.i18n.t("transport-master-volume")),
                )
                .changed()
            {
                app.action_set_master_gain(master_db);
            }
        });
    });
}

fn render_master_vu_meter(app: &StemApp, ui: &mut egui::Ui) {
    let meter_size = Vec2::new(120.0, 18.0);
    let (rect, _response) = ui.allocate_exact_size(meter_size, egui::Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(25, 25, 25));

    let half_h = rect.height() * 0.45;
    let gap = rect.height() * 0.1;

    let left_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), half_h));
    let right_rect = Rect::from_min_size(
        rect.min + Vec2::new(0.0, half_h + gap),
        Vec2::new(rect.width(), half_h),
    );

    // Render bar for Left Channel
    render_single_meter_bar(painter, left_rect, app.telemetry.peak_left);
    // Render bar for Right Channel
    render_single_meter_bar(painter, right_rect, app.telemetry.peak_right);
}

fn render_single_meter_bar(painter: &egui::Painter, rect: Rect, peak: f32) {
    let clamped_peak = peak.clamp(0.0, 1.2);
    let fill_w = rect.width() * (clamped_peak / 1.0).min(1.0);

    let green_w = fill_w.min(rect.width() * 0.75);
    let yellow_w = (fill_w - green_w).min(rect.width() * 0.20);
    let red_w = fill_w - green_w - yellow_w;

    let mut current_x = rect.min.x;
    if green_w > 0.0 {
        let r = Rect::from_min_size(
            egui::pos2(current_x, rect.min.y),
            Vec2::new(green_w, rect.height()),
        );
        painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(60, 200, 60));
        current_x += green_w;
    }
    if yellow_w > 0.0 {
        let r = Rect::from_min_size(
            egui::pos2(current_x, rect.min.y),
            Vec2::new(yellow_w, rect.height()),
        );
        painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(230, 200, 40));
        current_x += yellow_w;
    }
    if red_w > 0.0 {
        let r = Rect::from_min_size(
            egui::pos2(current_x, rect.min.y),
            Vec2::new(red_w, rect.height()),
        );
        painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(240, 50, 50));
    }

    // Border
    painter.rect_stroke(
        rect,
        Rounding::same(1.0),
        Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50)),
    );
}
