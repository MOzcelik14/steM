//! Interactive visualizers and real-time controls for Track and Master effect chains

use crate::app::StemApp;
use crate::state::AppViewMode;
use egui::{pos2, vec2, Color32, Rounding, Stroke};
use stem_core::project::{EqBandConfig, FilterType};
use stem_dsp::biquad::BiquadCoeffs;
use stem_dsp::chain::{
    CompressorEffect, DelayEffect, EffectChain, EqEffect, InsertEffect, LimiterEffect, ReverbEffect,
};
use stem_dsp::dynamics::Compressor;
use stem_i18n::format_frequency;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectTarget {
    Master,
    Track(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectSlotType {
    Equalizer,
    Compressor,
    Limiter,
    Delay,
    Reverb,
}

#[derive(Debug, Clone)]
pub struct EffectSlotState {
    pub effect_type: EffectSlotType,
    pub bypassed: bool,
}

pub struct EffectsRackState {
    pub target: EffectTarget,
    pub slots: Vec<EffectSlotState>,

    // 1. Equalizer parameters
    pub selected_band_idx: usize,
    pub bands: [EqBandConfig; 4],

    // 2. Dynamics Compressor parameters
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub comp_attack_ms: f32,
    pub comp_release_ms: f32,
    pub comp_makeup_db: f32,

    // 3. Limiter parameters
    pub limiter_threshold_db: f32,
    pub limiter_release_ms: f32,

    // 4. Stereo Delay parameters
    pub delay_time_ms: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,

    // 5. Algorithmic Reverb parameters
    pub reverb_room_size: f32,
    pub reverb_damping: f32,
    pub reverb_mix: f32,
}

impl Default for EffectsRackState {
    fn default() -> Self {
        Self {
            target: EffectTarget::Master,
            slots: vec![
                EffectSlotState {
                    effect_type: EffectSlotType::Equalizer,
                    bypassed: false,
                },
                EffectSlotState {
                    effect_type: EffectSlotType::Compressor,
                    bypassed: true,
                },
                EffectSlotState {
                    effect_type: EffectSlotType::Limiter,
                    bypassed: true,
                },
                EffectSlotState {
                    effect_type: EffectSlotType::Delay,
                    bypassed: true,
                },
                EffectSlotState {
                    effect_type: EffectSlotType::Reverb,
                    bypassed: true,
                },
            ],
            selected_band_idx: 1,
            bands: [
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::LowShelf,
                    frequency: 100.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::Peaking,
                    frequency: 800.0,
                    gain_db: 0.0,
                    q: 1.0,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::Peaking,
                    frequency: 3000.0,
                    gain_db: 0.0,
                    q: 1.0,
                },
                EqBandConfig {
                    enabled: true,
                    filter_type: FilterType::HighShelf,
                    frequency: 10000.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
            ],
            comp_threshold_db: -18.0,
            comp_ratio: 3.5,
            comp_attack_ms: 15.0,
            comp_release_ms: 120.0,
            comp_makeup_db: 0.0,
            limiter_threshold_db: -0.5,
            limiter_release_ms: 50.0,
            delay_time_ms: 350.0,
            delay_feedback: 0.3,
            delay_mix: 0.25,
            reverb_room_size: 0.65,
            reverb_damping: 0.3,
            reverb_mix: 0.2,
        }
    }
}

impl EffectsRackState {
    /// Builds an offline EffectChain reflecting the exact active slot order and parameters.
    pub fn build_dsp_chain(&self, sample_rate: f32) -> EffectChain {
        let mut chain = EffectChain::new();
        for slot in &self.slots {
            match slot.effect_type {
                EffectSlotType::Equalizer => {
                    let mut eq = EqEffect::new(sample_rate);
                    for (i, b) in self.bands.iter().enumerate() {
                        eq.set_band(i, b.clone());
                    }
                    eq.bypassed = slot.bypassed;
                    chain.add_effect(InsertEffect::Equalizer(eq));
                }
                EffectSlotType::Compressor => {
                    let mut comp = CompressorEffect::new(
                        sample_rate,
                        self.comp_threshold_db,
                        self.comp_ratio,
                        self.comp_attack_ms,
                        self.comp_release_ms,
                        self.comp_makeup_db,
                    );
                    comp.bypassed = slot.bypassed;
                    chain.add_effect(InsertEffect::Compressor(comp));
                }
                EffectSlotType::Limiter => {
                    let mut limiter = LimiterEffect::new(
                        sample_rate,
                        self.limiter_threshold_db,
                        self.limiter_release_ms,
                    );
                    limiter.bypassed = slot.bypassed;
                    chain.add_effect(InsertEffect::Limiter(limiter));
                }
                EffectSlotType::Delay => {
                    let mut delay = DelayEffect::new(
                        sample_rate,
                        self.delay_time_ms,
                        self.delay_feedback,
                        self.delay_mix,
                    );
                    delay.bypassed = slot.bypassed;
                    chain.add_effect(InsertEffect::Delay(delay));
                }
                EffectSlotType::Reverb => {
                    let mut reverb = ReverbEffect::new(
                        sample_rate,
                        self.reverb_room_size,
                        self.reverb_damping,
                        self.reverb_mix,
                    );
                    reverb.bypassed = slot.bypassed;
                    chain.add_effect(InsertEffect::Reverb(reverb));
                }
            }
        }
        chain
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
        .default_size(vec2(680.0, 560.0))
        .show(ctx, |ui| {
            // Target header & Waveform mode apply button
            ui.horizontal(|ui| {
                if app.view_mode == AppViewMode::Waveform {
                    ui.label(
                        egui::RichText::new(format!("⎌ {}", app.i18n.t("effect-target-waveform")))
                            .strong()
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(format!("⚡ {}", app.i18n.t("effect-btn-apply")))
                            .on_hover_text(
                                "Renders the effect chain into the waveform document's edit list",
                            )
                            .clicked()
                        {
                            app.action_waveform_apply_active_effects();
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new(app.i18n.t("effect-target-master"))
                            .strong()
                            .color(egui::Color32::from_rgb(255, 200, 100)),
                    );

                    egui::ComboBox::from_id_salt("effects_target_combo")
                        .selected_text(match app.effects_state.target {
                            EffectTarget::Master => app.i18n.t("effect-target-master"),
                            EffectTarget::Track(idx) => app.i18n.t_arg(
                                "effect-target-track",
                                "index",
                                (idx + 1).to_string(),
                            ),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.effects_state.target,
                                EffectTarget::Master,
                                app.i18n.t("effect-target-master"),
                            );
                            for (t_idx, track) in app.project.tracks.iter().enumerate() {
                                let label = format!("Track {}: {}", t_idx + 1, track.name);
                                ui.selectable_value(
                                    &mut app.effects_state.target,
                                    EffectTarget::Track(t_idx),
                                    label,
                                );
                            }
                        });
                }
            });

            ui.separator();

            let target_opt = match app.effects_state.target {
                EffectTarget::Master => None,
                EffectTarget::Track(idx) => Some(idx),
            };

            // Reorder actions or slots manipulation
            let mut move_up_idx: Option<usize> = None;
            let mut move_down_idx: Option<usize> = None;
            let mut reset_idx: Option<usize> = None;
            let mut toggle_bypass_idx: Option<usize> = None;

            let num_slots = app.effects_state.slots.len();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for slot_idx in 0..num_slots {
                        let slot_effect_type = app.effects_state.slots[slot_idx].effect_type;
                        let bypassed = app.effects_state.slots[slot_idx].bypassed;
                        let effect_title = match slot_effect_type {
                            EffectSlotType::Equalizer => app.i18n.t("effect-eq-title"),
                            EffectSlotType::Compressor => app.i18n.t("effect-comp-title"),
                            EffectSlotType::Limiter => app.i18n.t("effect-limiter-title"),
                            EffectSlotType::Delay => app.i18n.t("effect-delay-title"),
                            EffectSlotType::Reverb => app.i18n.t("effect-reverb-title"),
                        };

                        let header_color = if bypassed {
                            Color32::GRAY
                        } else {
                            Color32::WHITE
                        };

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}. {}",
                                        slot_idx + 1,
                                        effect_title
                                    ))
                                    .strong()
                                    .color(header_color),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_enabled(
                                                slot_idx + 1 < num_slots,
                                                egui::Button::new("⏷").min_size(vec2(22.0, 18.0)),
                                            )
                                            .on_hover_text(app.i18n.t("effect-btn-move-down"))
                                            .clicked()
                                        {
                                            move_down_idx = Some(slot_idx);
                                        }

                                        if ui
                                            .add_enabled(
                                                slot_idx > 0,
                                                egui::Button::new("⏶").min_size(vec2(22.0, 18.0)),
                                            )
                                            .on_hover_text(app.i18n.t("effect-btn-move-up"))
                                            .clicked()
                                        {
                                            move_up_idx = Some(slot_idx);
                                        }

                                        if ui
                                            .button(app.i18n.t("effect-btn-reset"))
                                            .on_hover_text("Reset parameters to default")
                                            .clicked()
                                        {
                                            reset_idx = Some(slot_idx);
                                        }

                                        let mut bypass_val = bypassed;
                                        if ui
                                            .checkbox(
                                                &mut bypass_val,
                                                app.i18n.t("effect-btn-bypass"),
                                            )
                                            .clicked()
                                        {
                                            toggle_bypass_idx = Some(slot_idx);
                                        }
                                    },
                                );
                            });

                            // Render specific effect parameter controls & visualizer
                            match slot_effect_type {
                                EffectSlotType::Equalizer => {
                                    render_parametric_eq_visualizer(app, ui, target_opt, slot_idx);
                                }
                                EffectSlotType::Compressor => {
                                    render_compressor_visualizer(app, ui, target_opt, slot_idx);
                                }
                                EffectSlotType::Limiter => {
                                    render_limiter_controls(app, ui, target_opt, slot_idx);
                                }
                                EffectSlotType::Delay => {
                                    render_delay_controls(app, ui, target_opt, slot_idx);
                                }
                                EffectSlotType::Reverb => {
                                    render_reverb_controls(app, ui, target_opt, slot_idx);
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });

            // Handle actions outside loop
            if let Some(i) = toggle_bypass_idx {
                app.effects_state.slots[i].bypassed = !app.effects_state.slots[i].bypassed;
                let b = app.effects_state.slots[i].bypassed;
                if let Some(ref mut engine) = app.audio_engine {
                    engine.set_effect_bypass(target_opt, i, b);
                }
            }

            if let Some(i) = move_up_idx {
                if i > 0 {
                    app.effects_state.slots.swap(i, i - 1);
                    if let Some(ref mut engine) = app.audio_engine {
                        engine.reorder_effect(target_opt, i, i - 1);
                    }
                }
            }

            if let Some(i) = move_down_idx {
                if i + 1 < app.effects_state.slots.len() {
                    app.effects_state.slots.swap(i, i + 1);
                    if let Some(ref mut engine) = app.audio_engine {
                        engine.reorder_effect(target_opt, i, i + 1);
                    }
                }
            }

            if let Some(i) = reset_idx {
                match app.effects_state.slots[i].effect_type {
                    EffectSlotType::Equalizer => {
                        for b in &mut app.effects_state.bands {
                            b.gain_db = 0.0;
                        }
                    }
                    EffectSlotType::Compressor => {
                        app.effects_state.comp_threshold_db = -18.0;
                        app.effects_state.comp_ratio = 3.5;
                        app.effects_state.comp_makeup_db = 0.0;
                    }
                    EffectSlotType::Limiter => {
                        app.effects_state.limiter_threshold_db = -0.5;
                        app.effects_state.limiter_release_ms = 50.0;
                    }
                    EffectSlotType::Delay => {
                        app.effects_state.delay_time_ms = 350.0;
                        app.effects_state.delay_feedback = 0.3;
                        app.effects_state.delay_mix = 0.25;
                    }
                    EffectSlotType::Reverb => {
                        app.effects_state.reverb_room_size = 0.65;
                        app.effects_state.reverb_damping = 0.3;
                        app.effects_state.reverb_mix = 0.2;
                    }
                }
                if let Some(ref mut engine) = app.audio_engine {
                    engine.reset_effect(target_opt, i);
                }
            }
        });

    if !open {
        app.show_effects_dialog = false;
    }
}

fn render_parametric_eq_visualizer(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    target_opt: Option<usize>,
    slot_idx: usize,
) {
    let canvas_size = vec2(ui.available_width(), 140.0);
    let (rect, response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(rect);

    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(18, 20, 24));
    painter.rect_stroke(
        rect,
        Rounding::same(4.0),
        Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 56)),
    );

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

    let zero_y = db_to_y(0.0);
    painter.line_segment(
        [pos2(rect.min.x, zero_y), pos2(rect.max.x, zero_y)],
        Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 75)),
    );

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

    let mouse_pos = response.hover_pos();
    let is_dragging = response.dragged();

    for (b_idx, band) in app.effects_state.bands.iter_mut().enumerate() {
        let node_pt = pos2(freq_to_x(band.frequency), db_to_y(band.gain_db));

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

                if let Some(ref mut engine) = app.audio_engine {
                    engine.set_effect_param(
                        target_opt,
                        slot_idx,
                        (b_idx as u8) + 4,
                        band.frequency,
                    );
                    engine.set_effect_param(target_opt, slot_idx, b_idx as u8, band.gain_db);
                }
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

    // Selected band controls
    let sel_idx = app.effects_state.selected_band_idx;
    if sel_idx < app.effects_state.bands.len() {
        let band = &mut app.effects_state.bands[sel_idx];
        ui.horizontal(|ui| {
            ui.label(format!("Band {}", sel_idx + 1));
            if ui.checkbox(&mut band.enabled, "Enabled").changed() {
                if let Some(ref mut engine) = app.audio_engine {
                    let val = if band.enabled { 1.0 } else { 0.0 };
                    engine.set_effect_param(target_opt, slot_idx, (sel_idx as u8) + 12, val);
                }
            }
            ui.label("Freq:");
            if ui
                .add(
                    egui::DragValue::new(&mut band.frequency)
                        .range(20.0..=20000.0)
                        .speed(5.0),
                )
                .changed()
            {
                if let Some(ref mut engine) = app.audio_engine {
                    engine.set_effect_param(
                        target_opt,
                        slot_idx,
                        (sel_idx as u8) + 4,
                        band.frequency,
                    );
                }
            }
            ui.label("Gain (dB):");
            if ui
                .add(
                    egui::DragValue::new(&mut band.gain_db)
                        .range(-18.0..=18.0)
                        .speed(0.1),
                )
                .changed()
            {
                if let Some(ref mut engine) = app.audio_engine {
                    engine.set_effect_param(target_opt, slot_idx, sel_idx as u8, band.gain_db);
                }
            }
            ui.label("Q:");
            if ui
                .add(
                    egui::DragValue::new(&mut band.q)
                        .range(0.1..=10.0)
                        .speed(0.05),
                )
                .changed()
            {
                if let Some(ref mut engine) = app.audio_engine {
                    engine.set_effect_param(target_opt, slot_idx, (sel_idx as u8) + 8, band.q);
                }
            }
        });
    }
}

fn render_compressor_visualizer(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    target_opt: Option<usize>,
    slot_idx: usize,
) {
    let canvas_size = vec2(ui.available_width(), 120.0);
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

    // Static compression curve
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

    let thresh_pt = pos2(
        in_to_x(app.effects_state.comp_threshold_db),
        out_to_y(app.effects_state.comp_threshold_db),
    );
    painter.circle_filled(thresh_pt, 5.0, Color32::from_rgb(255, 200, 50));

    // Sliders
    ui.horizontal(|ui| {
        ui.label(app.i18n.t("effect-threshold"));
        if ui
            .add(
                egui::Slider::new(&mut app.effects_state.comp_threshold_db, -60.0..=0.0)
                    .suffix(" dB"),
            )
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(
                    target_opt,
                    slot_idx,
                    0,
                    app.effects_state.comp_threshold_db,
                );
            }
        }

        ui.label(app.i18n.t("effect-ratio"));
        if ui
            .add(egui::Slider::new(&mut app.effects_state.comp_ratio, 1.0..=20.0).suffix(":1"))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 1, app.effects_state.comp_ratio);
            }
        }

        ui.label(app.i18n.t("effect-makeup-gain"));
        if ui
            .add(egui::Slider::new(&mut app.effects_state.comp_makeup_db, 0.0..=24.0).suffix(" dB"))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 4, app.effects_state.comp_makeup_db);
            }
        }
    });
}

fn render_limiter_controls(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    target_opt: Option<usize>,
    slot_idx: usize,
) {
    ui.horizontal(|ui| {
        ui.label(app.i18n.t("effect-threshold"));
        if ui
            .add(
                egui::Slider::new(&mut app.effects_state.limiter_threshold_db, -30.0..=0.0)
                    .suffix(" dBFS"),
            )
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(
                    target_opt,
                    slot_idx,
                    0,
                    app.effects_state.limiter_threshold_db,
                );
            }
        }

        ui.label(app.i18n.t("effect-release"));
        if ui
            .add(
                egui::Slider::new(&mut app.effects_state.limiter_release_ms, 1.0..=500.0)
                    .suffix(" ms"),
            )
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(
                    target_opt,
                    slot_idx,
                    1,
                    app.effects_state.limiter_release_ms,
                );
            }
        }
    });
}

fn render_delay_controls(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    target_opt: Option<usize>,
    slot_idx: usize,
) {
    ui.horizontal(|ui| {
        ui.label(app.i18n.t("effect-delay-time"));
        if ui
            .add(
                egui::Slider::new(&mut app.effects_state.delay_time_ms, 10.0..=1500.0)
                    .suffix(" ms"),
            )
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 0, app.effects_state.delay_time_ms);
            }
        }

        ui.label(app.i18n.t("effect-feedback"));
        if ui
            .add(egui::Slider::new(
                &mut app.effects_state.delay_feedback,
                0.0..=0.95,
            ))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 1, app.effects_state.delay_feedback);
            }
        }

        ui.label(app.i18n.t("effect-mix"));
        if ui
            .add(egui::Slider::new(
                &mut app.effects_state.delay_mix,
                0.0..=1.0,
            ))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 2, app.effects_state.delay_mix);
            }
        }
    });
}

fn render_reverb_controls(
    app: &mut StemApp,
    ui: &mut egui::Ui,
    target_opt: Option<usize>,
    slot_idx: usize,
) {
    ui.horizontal(|ui| {
        ui.label(app.i18n.t("effect-room-size"));
        if ui
            .add(egui::Slider::new(
                &mut app.effects_state.reverb_room_size,
                0.0..=0.98,
            ))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(
                    target_opt,
                    slot_idx,
                    0,
                    app.effects_state.reverb_room_size,
                );
            }
        }

        ui.label(app.i18n.t("effect-damping"));
        if ui
            .add(egui::Slider::new(
                &mut app.effects_state.reverb_damping,
                0.0..=1.0,
            ))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 1, app.effects_state.reverb_damping);
            }
        }

        ui.label(app.i18n.t("effect-mix"));
        if ui
            .add(egui::Slider::new(
                &mut app.effects_state.reverb_mix,
                0.0..=1.0,
            ))
            .changed()
        {
            if let Some(ref mut engine) = app.audio_engine {
                engine.set_effect_param(target_opt, slot_idx, 2, app.effects_state.reverb_mix);
            }
        }
    });
}
