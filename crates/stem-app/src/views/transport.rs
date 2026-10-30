//! Transport bar with playback controls, timecode display, and master VU meters

use crate::app::StemApp;
use egui::{Color32, Rect, Rounding, Stroke, Vec2};
use stem_i18n::{
    format_bars_beats, format_db, format_samples, format_smpte, format_time_code, TimecodeMode,
};

pub fn render_transport_bar(app: &mut StemApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);

        // ================= ROW 1: Transport, Input Metering & Timecode =================
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

            // Record Button
            let is_recording = app.is_recording;
            let is_count_in = app.recording_state.count_in_active;
            let rec_color = if is_recording {
                Color32::from_rgb(255, 30, 30)
            } else if is_count_in {
                Color32::from_rgb(255, 180, 30)
            } else {
                Color32::from_rgb(180, 50, 50)
            };
            let rec_btn = egui::Button::new(egui::RichText::new("⏺").size(16.0).color(rec_color));
            if ui
                .add(rec_btn)
                .on_hover_text(app.i18n.t("transport-record"))
                .clicked()
            {
                app.action_toggle_record();
            }

            // Loop
            let mut loop_enabled = app.project.loop_enabled;
            if ui
                .selectable_label(loop_enabled, egui::RichText::new("🔁").size(14.0))
                .on_hover_text("Loop Playback (L)")
                .clicked()
            {
                loop_enabled = !loop_enabled;
                app.action_set_loop(loop_enabled);
            }

            ui.separator();

            // Live Hardware Input Level Meter
            render_input_vu_meter(app, ui);

            // Software Input Monitoring
            let monitoring = app.project.input_monitoring;
            let mon_color = if monitoring {
                Color32::from_rgb(80, 230, 100)
            } else {
                Color32::GRAY
            };
            if ui
                .selectable_label(
                    monitoring,
                    egui::RichText::new("🎧 Mon").size(11.0).color(mon_color),
                )
                .on_hover_text(app.i18n.t("recording-monitoring"))
                .clicked()
            {
                app.action_toggle_input_monitoring();
            }

            // Count-In Selector
            let mut count_in = app.project.count_in_mode;
            let count_in_label = match count_in {
                stem_core::project::CountInMode::Off => app.i18n.t("recording-count-in-off"),
                stem_core::project::CountInMode::OneBar => app.i18n.t("recording-count-in-1bar"),
                stem_core::project::CountInMode::TwoBars => app.i18n.t("recording-count-in-2bars"),
                stem_core::project::CountInMode::TwoSeconds => app.i18n.t("recording-count-in-2s"),
                stem_core::project::CountInMode::FourSeconds => app.i18n.t("recording-count-in-4s"),
            };
            egui::ComboBox::from_id_salt("transport_count_in")
                .selected_text(format!("⏱ {}", count_in_label))
                .width(80.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut count_in,
                        stem_core::project::CountInMode::Off,
                        app.i18n.t("recording-count-in-off"),
                    );
                    ui.selectable_value(
                        &mut count_in,
                        stem_core::project::CountInMode::OneBar,
                        app.i18n.t("recording-count-in-1bar"),
                    );
                    ui.selectable_value(
                        &mut count_in,
                        stem_core::project::CountInMode::TwoBars,
                        app.i18n.t("recording-count-in-2bars"),
                    );
                    ui.selectable_value(
                        &mut count_in,
                        stem_core::project::CountInMode::TwoSeconds,
                        app.i18n.t("recording-count-in-2s"),
                    );
                    ui.selectable_value(
                        &mut count_in,
                        stem_core::project::CountInMode::FourSeconds,
                        app.i18n.t("recording-count-in-4s"),
                    );
                });
            app.project.count_in_mode = count_in;

            // Punch-In Recording Toggle
            let mut punch_enabled = app.project.punch_in_enabled;
            let punch_color = if punch_enabled {
                Color32::from_rgb(255, 120, 50)
            } else {
                Color32::GRAY
            };
            if ui
                .selectable_label(
                    punch_enabled,
                    egui::RichText::new("🎯 Punch")
                        .size(11.0)
                        .color(punch_color),
                )
                .on_hover_text(app.i18n.t("recording-punch-in"))
                .clicked()
            {
                punch_enabled = !punch_enabled;
                app.project.punch_in_enabled = punch_enabled;
                if punch_enabled && app.project.punch_region.is_none() {
                    if let Some(sel) = app.waveform_state.selection {
                        app.project.punch_region = Some(sel);
                    } else {
                        let start = app.telemetry.playhead_frame;
                        let len = (app.project.sample_rate as u64 * 4).max(48000);
                        app.project.punch_region = Some((start, start + len));
                    }
                }
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
                            .size(15.0)
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
        });

        // ================= ROW 2: Session Tools, Mode Tabs & Master Output =================
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);

            // ----------------- Tempo & Signature -----------------
            ui.label(egui::RichText::new(app.i18n.t("transport-tempo")).size(11.0));
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

            ui.label(egui::RichText::new(app.i18n.t("transport-time-signature")).size(11.0));
            ui.label(
                egui::RichText::new(format!(
                    "{}/{}",
                    app.project.time_signature.0, app.project.time_signature.1
                ))
                .strong()
                .size(11.0),
            );

            // Effects Rack Quick Button
            if ui
                .button(
                    egui::RichText::new(format!("[FX] {}", app.i18n.t("track-effects-rack")))
                        .size(11.0),
                )
                .clicked()
            {
                app.show_effects_dialog = !app.show_effects_dialog;
            }

            ui.separator();

            // View Mode Switcher
            let is_multi = app.view_mode == crate::state::AppViewMode::Multitrack;
            if ui
                .selectable_label(
                    is_multi,
                    format!("⊞ {}", app.i18n.t("view-mode-multitrack")),
                )
                .on_hover_text("Multitrack Session View (Key: 1)")
                .clicked()
                && !is_multi
            {
                app.action_switch_to_multitrack();
            }

            let is_wave = app.view_mode == crate::state::AppViewMode::Waveform;
            if ui
                .selectable_label(is_wave, format!("~ {}", app.i18n.t("view-mode-waveform")))
                .on_hover_text("Single-File Waveform Editor (Key: 2)")
                .clicked()
                && !is_wave
            {
                app.action_switch_to_waveform();
            }

            // Right-aligned Master controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // ----------------- Master VU Meter -----------------
                render_master_vu_meter(app, ui);

                // Master Volume Slider
                ui.label(
                    egui::RichText::new(format_db(app.project.master_gain_db))
                        .size(11.0)
                        .monospace(),
                );
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
    });
}

fn render_master_vu_meter(app: &StemApp, ui: &mut egui::Ui) {
    let meter_size = Vec2::new(130.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(meter_size, egui::Sense::hover());

    let peak_l = app.telemetry.peak_left;
    let peak_r = app.telemetry.peak_right;
    let rms_l = app.telemetry.rms_left;
    let rms_r = app.telemetry.rms_right;

    let peak_db_l = if peak_l > 1e-5 {
        20.0 * peak_l.log10()
    } else {
        -90.0
    };
    let peak_db_r = if peak_r > 1e-5 {
        20.0 * peak_r.log10()
    } else {
        -90.0
    };
    let rms_db_l = if rms_l > 1e-5 {
        20.0 * rms_l.log10()
    } else {
        -90.0
    };
    let rms_db_r = if rms_r > 1e-5 {
        20.0 * rms_r.log10()
    } else {
        -90.0
    };

    response.on_hover_text(format!(
        "L: Peak {} / RMS {}\nR: Peak {} / RMS {}",
        format_db(peak_db_l),
        format_db(rms_db_l),
        format_db(peak_db_r),
        format_db(rms_db_r)
    ));

    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(20, 22, 26));

    let half_h = rect.height() * 0.45;
    let gap = rect.height() * 0.1;

    let left_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), half_h));
    let right_rect = Rect::from_min_size(
        rect.min + Vec2::new(0.0, half_h + gap),
        Vec2::new(rect.width(), half_h),
    );

    // Render dual Peak + RMS bars
    render_single_meter_bar(painter, left_rect, peak_l, rms_l);
    render_single_meter_bar(painter, right_rect, peak_r, rms_r);
}

fn render_single_meter_bar(painter: &egui::Painter, rect: Rect, peak: f32, rms: f32) {
    let clamped_peak = peak.clamp(0.0, 1.2);
    let clamped_rms = rms.clamp(0.0, 1.2);

    let fill_w_rms = rect.width() * (clamped_rms / 1.0).min(1.0);
    let fill_w_peak = rect.width() * (clamped_peak / 1.0).min(1.0);

    // 1. Draw RMS bar (solid medium intensity)
    if fill_w_rms > 0.0 {
        let green_w = fill_w_rms.min(rect.width() * 0.75);
        let yellow_w = (fill_w_rms - green_w).min(rect.width() * 0.20);
        let red_w = fill_w_rms - green_w - yellow_w;

        let mut current_x = rect.min.x;
        if green_w > 0.0 {
            let r = Rect::from_min_size(
                egui::pos2(current_x, rect.min.y),
                Vec2::new(green_w, rect.height()),
            );
            painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(45, 150, 45));
            current_x += green_w;
        }
        if yellow_w > 0.0 {
            let r = Rect::from_min_size(
                egui::pos2(current_x, rect.min.y),
                Vec2::new(yellow_w, rect.height()),
            );
            painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(180, 150, 30));
            current_x += yellow_w;
        }
        if red_w > 0.0 {
            let r = Rect::from_min_size(
                egui::pos2(current_x, rect.min.y),
                Vec2::new(red_w, rect.height()),
            );
            painter.rect_filled(r, Rounding::ZERO, Color32::from_rgb(190, 40, 40));
        }
    }

    // 2. Draw Peak Indicator Line / Tick (bright highlight)
    if fill_w_peak > 1.0 {
        let peak_x = (rect.min.x + fill_w_peak).min(rect.max.x - 1.5);
        let peak_color = if clamped_peak > 1.0 {
            Color32::from_rgb(255, 60, 60)
        } else if clamped_peak > 0.75 {
            Color32::from_rgb(255, 230, 80)
        } else {
            Color32::from_rgb(100, 255, 100)
        };
        painter.line_segment(
            [
                egui::pos2(peak_x, rect.min.y),
                egui::pos2(peak_x, rect.max.y),
            ],
            Stroke::new(2.0_f32, peak_color),
        );
    }

    // Border
    painter.rect_stroke(
        rect,
        Rounding::same(1.0),
        Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 55)),
    );
}

/// Renders a compact dual Peak/RMS hardware input meter in the transport bar.
pub fn render_input_vu_meter(app: &StemApp, ui: &mut egui::Ui) {
    let meter_size = Vec2::new(55.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(meter_size, egui::Sense::hover());

    let telem = app.recording_state.input_telemetry;
    let peak_l = telem.peak_l;
    let peak_r = telem.peak_r;
    let rms_l = telem.rms_l;
    let rms_r = telem.rms_r;

    let peak_db_l = if peak_l > 1e-5 {
        20.0 * peak_l.log10()
    } else {
        -90.0
    };
    let peak_db_r = if peak_r > 1e-5 {
        20.0 * peak_r.log10()
    } else {
        -90.0
    };

    response.on_hover_ui(|ui| {
        ui.label(
            egui::RichText::new(format!("🎙 {}", app.i18n.t("recording-input-level"))).strong(),
        );
        ui.label(format!("L Peak: {peak_db_l:.1} dBFS  |  RMS: {rms_l:.3}"));
        ui.label(format!("R Peak: {peak_db_r:.1} dBFS  |  RMS: {rms_r:.3}"));
    });

    let painter = ui.painter_at(rect);

    // Dark background
    painter.rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(18, 20, 24));

    let bar_height = (rect.height() - 3.0) / 2.0;

    // Top Bar: Input Left
    let rect_l = Rect::from_min_size(
        egui::pos2(rect.min.x + 1.0, rect.min.y + 1.0),
        Vec2::new(rect.width() - 2.0, bar_height),
    );
    render_single_meter_bar(&painter, rect_l, peak_l, rms_l);

    // Bottom Bar: Input Right
    let rect_r = Rect::from_min_size(
        egui::pos2(rect.min.x + 1.0, rect.min.y + 2.0 + bar_height),
        Vec2::new(rect.width() - 2.0, bar_height),
    );
    render_single_meter_bar(&painter, rect_r, peak_r, rms_r);

    // Overall frame outline
    painter.rect_stroke(
        rect,
        Rounding::same(2.0),
        Stroke::new(1.0_f32, Color32::from_rgb(50, 55, 65)),
    );
}
