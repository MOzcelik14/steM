//! Interactive visualizers for Parametric EQ and Dynamics Compressor

use crate::app::StemApp;
use egui::{pos2, vec2, Color32, Rounding, Stroke};
use stem_core::project::{EqBandConfig, FilterType};
use stem_dsp::biquad::BiquadCoeffs;
use stem_dsp::dynamics::Compressor;
use stem_i18n::format_frequency;

pub struct EffectsRackState {
    pub selected_band_idx: usize,
    pub bands: [EqBandConfig; 4],
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub comp_attack_ms: f32,
    pub comp_release_ms: f32,
    pub comp_makeup_db: f32,
}

impl Default for EffectsRackState {
    fn default() -> Self {
        Self {
            selected_band_idx: 1,
            bands: [
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::LowShelf,
                    frequency: 100.0,
                    gain_db: 2.0,
                    q: 0.707,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::Peaking,
                    frequency: 800.0,
                    gain_db: -3.0,
                    q: 1.2,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::Peaking,
                    frequency: 3200.0,
                    gain_db: 4.0,
                    q: 1.5,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::HighShelf,
                    frequency: 9000.0,
                    gain_db: 1.5,
                    q: 0.707,
                },
            ],
            comp_threshold_db: -18.0,
            comp_ratio: 3.5,
            comp_attack_ms: 15.0,
            comp_release_ms: 120.0,
            comp_makeup_db: 3.0,
        }
    }
}

pub fn render_effects_dialog(app: &mut StemApp, ctx: &egui::Context) {
    if !app.show_effects_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new(app.i18n.t("track-effects-rack"))
        .open(&mut open)
        .resizable(true)
        .default_size(vec2(640.0, 480.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Tabs or Sections: Parametric EQ & Dynamics Compressor
                ui.horizontal(|ui| {
                    ui.heading(app.i18n.t("effect-eq-title"));
                });

                // ================= 1. Parametric EQ Visualizer =================
                render_parametric_eq_visualizer(app, ui);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // ================= 2. Dynamics Compressor Visualizer =================
                ui.horizontal(|ui| {
                    ui.heading(app.i18n.t("effect-comp-title"));
                });

                render_compressor_visualizer(app, ui);
            });
        });

    if !open {
        app.show_effects_dialog = false;
    }
}

fn render_parametric_eq_visualizer(app: &mut StemApp, ui: &mut egui::Ui) {
    let canvas_size = vec2(ui.available_width(), 160.0);
    let (rect, response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(rect);

    // Background & border
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(
        rect,
        Rounding::same(4.0),
        Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56)),
    );

    // Logarithmic frequency bounds (20 Hz to 20,000 Hz)
    let min_freq = 20.0f32;
    let max_freq = 20000.0f32;
    let min_log = min_freq.log10();
    let max_log = max_freq.log10();

    let min_db = -18.0f32;
    let max_db = 18.0f32;

    let freq_to_x = |f: f32| -> f32 {
        let f_clamped = f.clamp(min_freq, max_freq);
        let norm = (f_clamped.log10() - min_log) / (max_log - min_log);
        rect.min.x + norm * rect.width()
    };

    let x_to_freq = |x: f32| -> f32 {
        let norm = ((x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        10.0f32.powf(min_log + norm * (max_log - min_log))
    };

    let db_to_y = |db: f32| -> f32 {
        let norm = (db.clamp(min_db, max_db) - min_db) / (max_db - min_db);
        rect.max.y - norm * rect.height()
    };

    let y_to_db = |y: f32| -> f32 {
        let norm = ((rect.max.y - y) / rect.height()).clamp(0.0, 1.0);
        min_db + norm * (max_db - min_db)
    };

    // Grid lines for standard frequencies: 50, 100, 250, 500, 1k, 2.5k, 5k, 10k, 20k Hz
    let grid_freqs = [
        50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 20000.0,
    ];
    for &gf in &grid_freqs {
        let x = freq_to_x(gf);
        painter.line_segment(
            [pos2(x, rect.min.y), pos2(x, rect.max.y)],
            Stroke::new(1.0_f32, Color32::from_rgb(28, 30, 36)),
        );
        painter.text(
            pos2(x + 2.0, rect.max.y - 12.0),
            egui::Align2::LEFT_BOTTOM,
            format_frequency(gf),
            egui::FontId::proportional(9.0),
            Color32::from_rgb(90, 95, 105),
        );
    }

    // Horizontal 0 dB center reference line
    let zero_y = db_to_y(0.0);
    painter.line_segment(
        [pos2(rect.min.x, zero_y), pos2(rect.max.x, zero_y)],
        Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75)),
    );

    // Compute combined magnitude curve across 200 screen points
    let sample_rate = app.project.sample_rate as f32;
    let biquad_coeffs: Vec<BiquadCoeffs> = app
        .effects_state
        .bands
        .iter()
        .filter(|b| b.enabled)
        .map(|b| BiquadCoeffs::design(b.filter_type, sample_rate, b.frequency, b.q, b.gain_db))
        .collect();

    let num_points = 200;
    let mut prev_pt: Option<egui::Pos2> = None;

    for i in 0..=num_points {
        let t = i as f32 / num_points as f32;
        let freq = 10.0f32.powf(min_log + t * (max_log - min_log));

        let mut total_db = 0.0f32;
        for c in &biquad_coeffs {
            total_db += c.magnitude_response_db(freq, sample_rate);
        }

        let pt = pos2(rect.min.x + t * rect.width(), db_to_y(total_db));
        if let Some(prev) = prev_pt {
            painter.line_segment(
                [prev, pt],
                Stroke::new(2.0_f32, Color32::from_rgb(90, 180, 255)),
            );
        }
        prev_pt = Some(pt);
    }

    // Render interactive draggable band control nodes
    let mouse_pos = response.hover_pos();
    let is_dragging = response.dragged();

    for (b_idx, band) in app.effects_state.bands.iter_mut().enumerate() {
        let node_pt = pos2(freq_to_x(band.frequency), db_to_y(band.gain_db));

        // Drag node interaction
        if is_dragging && response.drag_started() {
            if let Some(mp) = mouse_pos {
                if (mp - node_pt).length() < 14.0 {
                    app.effects_state.selected_band_idx = b_idx;
                }
            }
        }

        if is_dragging && app.effects_state.selected_band_idx == b_idx {
            if let Some(mp) = mouse_pos {
                band.frequency = x_to_freq(mp.x);
                band.gain_db = y_to_db(mp.y);
            }
        }

        let is_selected = app.effects_state.selected_band_idx == b_idx;
        let node_color = if is_selected {
            Color32::from_rgb(255, 200, 50)
        } else {
            Color32::from_rgb(90, 180, 255)
        };

        painter.circle_filled(node_pt, 6.0, node_color);
        painter.circle_stroke(node_pt, 6.0, Stroke::new(1.5_f32, Color32::WHITE));
    }

    // Controls for selected band
    let sel_idx = app.effects_state.selected_band_idx;
    if sel_idx < app.effects_state.bands.len() {
        let band = &mut app.effects_state.bands[sel_idx];
        ui.horizontal(|ui| {
            ui.label(format!("Band {}", sel_idx + 1));
            ui.checkbox(&mut band.enabled, "Enabled");
            ui.label("Freq:");
            ui.add(
                egui::DragValue::new(&mut band.frequency)
                    .range(20.0..=20000.0)
                    .speed(5.0),
            );
            ui.label("Gain (dB):");
            ui.add(
                egui::DragValue::new(&mut band.gain_db)
                    .range(-18.0..=18.0)
                    .speed(0.1),
            );
            ui.label("Q:");
            ui.add(
                egui::DragValue::new(&mut band.q)
                    .range(0.1..=10.0)
                    .speed(0.05),
            );
        });
    }
}

fn render_compressor_visualizer(app: &mut StemApp, ui: &mut egui::Ui) {
    let canvas_size = vec2(ui.available_width(), 140.0);
    let (rect, _response) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);

    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(
        rect,
        Rounding::same(4.0),
        Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56)),
    );

    let comp = Compressor::new(
        app.project.sample_rate as f32,
        app.effects_state.comp_threshold_db,
        app.effects_state.comp_ratio,
        app.effects_state.comp_attack_ms,
        app.effects_state.comp_release_ms,
        app.effects_state.comp_makeup_db,
    );

    let min_db = -60.0f32;
    let max_db = 0.0f32;

    let in_to_x = |db: f32| -> f32 {
        let norm = (db.clamp(min_db, max_db) - min_db) / (max_db - min_db);
        rect.min.x + norm * rect.width()
    };

    let out_to_y = |db: f32| -> f32 {
        let norm = (db.clamp(min_db, max_db) - min_db) / (max_db - min_db);
        rect.max.y - norm * rect.height()
    };

    // 1:1 Unity Line (Dotted)
    painter.line_segment(
        [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.min.y)],
        Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75)),
    );

    // Dynamic compression curve
    let mut prev_pt: Option<egui::Pos2> = None;
    for i in 0..=100 {
        let in_db = min_db + (i as f32 / 100.0) * (max_db - min_db);
        let out_db = comp.compute_static_transfer_db(in_db);
        let pt = pos2(in_to_x(in_db), out_to_y(out_db));

        if let Some(prev) = prev_pt {
            painter.line_segment(
                [prev, pt],
                Stroke::new(2.0_f32, Color32::from_rgb(255, 120, 80)),
            );
        }
        prev_pt = Some(pt);
    }

    // Threshold indicator point
    let thresh_pt = pos2(
        in_to_x(app.effects_state.comp_threshold_db),
        out_to_y(app.effects_state.comp_threshold_db),
    );
    painter.circle_filled(thresh_pt, 5.0, Color32::from_rgb(255, 200, 50));

    // Compressor parameter sliders
    ui.horizontal(|ui| {
        ui.label(app.i18n.t("effect-threshold"));
        ui.add(
            egui::Slider::new(&mut app.effects_state.comp_threshold_db, -60.0..=0.0).suffix(" dB"),
        );

        ui.label(app.i18n.t("effect-ratio"));
        ui.add(egui::Slider::new(&mut app.effects_state.comp_ratio, 1.0..=20.0).suffix(":1"));

        ui.label(app.i18n.t("effect-makeup-gain"));
        ui.add(egui::Slider::new(&mut app.effects_state.comp_makeup_db, 0.0..=24.0).suffix(" dB"));
    });
}
