//! Real-time FFT spectral analyzer and 2D rolling waterfall spectrogram view.

use crate::app::StemApp;
use egui::{pos2, vec2, Color32, Rect, Rounding, Stroke};
use stem_dsp::{SpectrogramPalette, SpectrumAnalyzer};
use stem_i18n::format_db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrogramDisplayMode {
    #[default]
    Both,
    Waterfall,
    Spectrum,
}

pub struct SpectrogramState {
    pub analyzer: SpectrumAnalyzer,
    pub spectrum_history: Vec<f32>,
    pub fft_size: usize,
    pub palette: SpectrogramPalette,
    pub display_mode: SpectrogramDisplayMode,
    /// 2D rolling waterfall buffer storing recent frequency columns: [column_idx][bin_idx]
    pub waterfall: Vec<Vec<f32>>,
    pub max_waterfall_columns: usize,
    pub min_db: f32,
    pub max_db: f32,
}

impl Default for SpectrogramState {
    fn default() -> Self {
        let fft_size = 1024;
        let num_bins = fft_size / 2 + 1;
        Self {
            analyzer: SpectrumAnalyzer::new(fft_size),
            spectrum_history: vec![-90.0; num_bins],
            fft_size,
            palette: SpectrogramPalette::Viridis,
            display_mode: SpectrogramDisplayMode::Both,
            waterfall: Vec::with_capacity(120),
            max_waterfall_columns: 120,
            min_db: -90.0,
            max_db: 0.0,
        }
    }
}

impl SpectrogramState {
    pub fn set_fft_size(&mut self, size: usize) {
        if size != self.fft_size && size.is_power_of_two() {
            self.fft_size = size;
            self.analyzer.set_fft_size(size);
            let num_bins = size / 2 + 1;
            self.spectrum_history = vec![-90.0; num_bins];
            self.waterfall.clear();
        }
    }
}

pub fn render_spectrogram_panel(app: &mut StemApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        // ----------------- Header Controls Bar -----------------
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app.i18n.t("view-spectrogram")).strong());

            ui.separator();

            // Mode Selector
            ui.label(app.i18n.t("spectrogram-mode"));
            let mode = app.spectrogram_state.display_mode;
            let mode_str = match mode {
                SpectrogramDisplayMode::Both => app.i18n.t("spectrogram-mode-both"),
                SpectrogramDisplayMode::Waterfall => app.i18n.t("spectrogram-mode-waterfall"),
                SpectrogramDisplayMode::Spectrum => app.i18n.t("spectrogram-mode-curve"),
            };
            egui::ComboBox::from_id_salt("spec_mode_combo")
                .selected_text(mode_str)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            mode == SpectrogramDisplayMode::Both,
                            app.i18n.t("spectrogram-mode-both"),
                        )
                        .clicked()
                    {
                        app.spectrogram_state.display_mode = SpectrogramDisplayMode::Both;
                    }
                    if ui
                        .selectable_label(
                            mode == SpectrogramDisplayMode::Waterfall,
                            app.i18n.t("spectrogram-mode-waterfall"),
                        )
                        .clicked()
                    {
                        app.spectrogram_state.display_mode = SpectrogramDisplayMode::Waterfall;
                    }
                    if ui
                        .selectable_label(
                            mode == SpectrogramDisplayMode::Spectrum,
                            app.i18n.t("spectrogram-mode-curve"),
                        )
                        .clicked()
                    {
                        app.spectrogram_state.display_mode = SpectrogramDisplayMode::Spectrum;
                    }
                });

            ui.separator();

            // FFT Size Selector
            ui.label(app.i18n.t("spectrogram-fft-size"));
            let current_fft = app.spectrogram_state.fft_size;
            egui::ComboBox::from_id_salt("spec_fft_combo")
                .selected_text(current_fft.to_string())
                .show_ui(ui, |ui| {
                    for &size in &[256usize, 512, 1024, 2048, 4096] {
                        if ui
                            .selectable_label(size == current_fft, size.to_string())
                            .clicked()
                        {
                            app.spectrogram_state.set_fft_size(size);
                        }
                    }
                });

            ui.separator();

            // Color Scale / Palette Selector
            ui.label(app.i18n.t("spectrogram-palette"));
            let palette = app.spectrogram_state.palette;
            let palette_name = match palette {
                SpectrogramPalette::Viridis => "Viridis",
                SpectrogramPalette::Magma => "Magma",
                SpectrogramPalette::Inferno => "Inferno",
                SpectrogramPalette::ClassicHeat => "Classic Heat",
                SpectrogramPalette::Cyberpunk => "Cyberpunk",
            };
            egui::ComboBox::from_id_salt("spec_palette_combo")
                .selected_text(palette_name)
                .show_ui(ui, |ui| {
                    let palettes = [
                        (SpectrogramPalette::Viridis, "Viridis"),
                        (SpectrogramPalette::Magma, "Magma"),
                        (SpectrogramPalette::Inferno, "Inferno"),
                        (SpectrogramPalette::ClassicHeat, "Classic Heat"),
                        (SpectrogramPalette::Cyberpunk, "Cyberpunk"),
                    ];
                    for (pal, name) in palettes {
                        if ui.selectable_label(palette == pal, name).clicked() {
                            app.spectrogram_state.palette = pal;
                        }
                    }
                });

            // Live Telemetry Readout (Peak & RMS)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let peak_max = app.telemetry.peak_left.max(app.telemetry.peak_right);
                let rms_max = app.telemetry.rms_left.max(app.telemetry.rms_right);

                let peak_db = if peak_max > 1e-5 {
                    20.0 * peak_max.log10()
                } else {
                    -90.0
                };
                let rms_db = if rms_max > 1e-5 {
                    20.0 * rms_max.log10()
                } else {
                    -90.0
                };

                let p_color = if peak_db > 0.0 {
                    Color32::from_rgb(255, 60, 60)
                } else {
                    Color32::from_rgb(80, 220, 120)
                };

                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}  |  {}: {}",
                        app.i18n.t("meter-peak"),
                        format_db(peak_db),
                        app.i18n.t("meter-rms"),
                        format_db(rms_db)
                    ))
                    .monospace()
                    .size(11.0)
                    .color(p_color),
                );
            });
        });

        // ----------------- Extract Audio Slice for FFT -----------------
        let fft_size = app.spectrogram_state.fft_size;
        let mut audio_slice = vec![0.0f32; fft_size];

        let playhead = if app.view_mode == crate::state::AppViewMode::Waveform {
            app.waveform_state.playhead_frame as usize
        } else {
            app.telemetry.playhead_frame as usize
        };

        if app.view_mode == crate::state::AppViewMode::Waveform {
            if let Some(ref doc) = app.waveform_state.document {
                let planar = app
                    .waveform_state
                    .cached_planar
                    .get_or_insert_with(|| doc.render_planar(&app.source_buffers));
                if let Some(ch) = planar.first() {
                    let total = ch.len();
                    for (i, sample) in audio_slice.iter_mut().enumerate().take(fft_size) {
                        let frame = playhead + i;
                        if frame < total {
                            *sample = ch[frame];
                        }
                    }
                }
            }
        } else if let Some(ref timeline) = app.active_timeline {
            let total = timeline.total_frames as usize;
            if let Some(ch) = timeline.channels.first() {
                for (i, sample) in audio_slice.iter_mut().enumerate().take(fft_size) {
                    let frame = playhead + i;
                    if frame < total && frame < ch.len() {
                        *sample = ch[frame];
                    }
                }
            }
        }

        let mag_db = app
            .spectrogram_state
            .analyzer
            .compute_magnitude_db(&audio_slice);

        // Update spectrum history with smoothing
        for (hist, &curr) in app
            .spectrogram_state
            .spectrum_history
            .iter_mut()
            .zip(mag_db.iter())
        {
            if curr > *hist {
                *hist = curr;
            } else {
                *hist = *hist * 0.82 + curr * 0.18;
            }
        }

        // Push column into rolling 2D waterfall buffer during playback or updates
        if app.is_playing || app.spectrogram_state.waterfall.is_empty() {
            if app.spectrogram_state.waterfall.len() >= app.spectrogram_state.max_waterfall_columns
            {
                app.spectrogram_state.waterfall.remove(0);
            }
            app.spectrogram_state.waterfall.push(mag_db);
        }

        // ----------------- Display Panel Canvas -----------------
        let available_size = vec2(ui.available_width(), 160.0);
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(12, 14, 18));
        painter.rect_stroke(
            rect,
            Rounding::same(4.0),
            Stroke::new(1.0_f32, Color32::from_rgb(40, 42, 50)),
        );

        match app.spectrogram_state.display_mode {
            SpectrogramDisplayMode::Waterfall => {
                render_waterfall_canvas(painter, rect, &app.spectrogram_state);
            }
            SpectrogramDisplayMode::Spectrum => {
                render_spectrum_canvas(
                    painter,
                    rect,
                    &app.spectrogram_state,
                    app.project.sample_rate,
                );
            }
            SpectrogramDisplayMode::Both => {
                let half_h = rect.height() * 0.5;
                let waterfall_rect = Rect::from_min_size(rect.min, vec2(rect.width(), half_h));
                let spectrum_rect = Rect::from_min_size(
                    pos2(rect.min.x, rect.min.y + half_h),
                    vec2(rect.width(), half_h),
                );

                render_waterfall_canvas(painter, waterfall_rect, &app.spectrogram_state);
                render_spectrum_canvas(
                    painter,
                    spectrum_rect,
                    &app.spectrogram_state,
                    app.project.sample_rate,
                );
            }
        }
    });
}

fn render_waterfall_canvas(painter: &egui::Painter, rect: Rect, state: &SpectrogramState) {
    let num_cols = state.waterfall.len();
    if num_cols == 0 {
        return;
    }

    let col_w = rect.width() / state.max_waterfall_columns as f32;
    let min_db = state.min_db;
    let max_db = state.max_db;

    // Render columns from left (oldest) to right (newest)
    for (c_idx, col) in state.waterfall.iter().enumerate() {
        let x = rect.min.x + c_idx as f32 * col_w;
        let num_bins = col.len();
        if num_bins == 0 {
            continue;
        }

        // Draw vertical stripes grouped into chunks for render efficiency
        let chunk_size = (num_bins / 40).max(1);
        let num_chunks = num_bins.div_ceil(chunk_size);
        let chunk_h = rect.height() / num_chunks as f32;

        for chunk in 0..num_chunks {
            let start_bin = chunk * chunk_size;
            let end_bin = (start_bin + chunk_size).min(num_bins);

            let mut max_chunk_db = -120.0f32;
            for &bin_db in &col[start_bin..end_bin] {
                if bin_db > max_chunk_db {
                    max_chunk_db = bin_db;
                }
            }

            let norm = ((max_chunk_db - min_db) / (max_db - min_db)).clamp(0.0, 1.0);
            let (r, g, b) = state.palette.color_rgb(norm);
            let color = Color32::from_rgb(r, g, b);

            // Frequency increases from bottom to top
            let y = rect.max.y - (chunk + 1) as f32 * chunk_h;
            let chunk_rect = Rect::from_min_size(pos2(x, y), vec2(col_w + 0.5, chunk_h + 0.5));
            painter.rect_filled(chunk_rect, Rounding::ZERO, color);
        }
    }
}

fn render_spectrum_canvas(
    painter: &egui::Painter,
    rect: Rect,
    state: &SpectrogramState,
    sample_rate: u32,
) {
    let num_bins = state.spectrum_history.len();
    if num_bins < 2 {
        return;
    }

    let min_db = state.min_db;
    let max_db = state.max_db;

    // Draw reference frequency grid lines
    let grid_freqs = [100.0f32, 1000.0, 5000.0, 10000.0];
    let nyquist = sample_rate as f32 * 0.5;

    for &freq in &grid_freqs {
        if freq < nyquist {
            let bin_idx = (freq / nyquist * (num_bins - 1) as f32).round() as usize;
            let norm_x = bin_idx as f32 / (num_bins - 1) as f32;
            let x = rect.min.x + norm_x * rect.width();

            painter.line_segment(
                [pos2(x, rect.min.y), pos2(x, rect.max.y)],
                Stroke::new(1.0_f32, Color32::from_rgb(28, 30, 36)),
            );

            let label = if freq >= 1000.0 {
                format!("{}k", (freq / 1000.0).round() as i32)
            } else {
                format!("{}Hz", freq as i32)
            };
            painter.text(
                pos2(x + 2.0, rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(9.0),
                Color32::from_rgb(90, 95, 110),
            );
        }
    }

    // Draw reference dB horizontal grid lines
    let db_levels = [-60.0f32, -40.0, -20.0, 0.0];
    for &db in &db_levels {
        let norm_y = ((db - min_db) / (max_db - min_db)).clamp(0.0, 1.0);
        let y = rect.max.y - norm_y * rect.height();

        painter.line_segment(
            [pos2(rect.min.x, y), pos2(rect.max.x, y)],
            Stroke::new(1.0_f32, Color32::from_rgb(25, 27, 32)),
        );

        painter.text(
            pos2(rect.max.x - 30.0, y - 6.0),
            egui::Align2::LEFT_BOTTOM,
            format_db(db),
            egui::FontId::monospace(8.0),
            Color32::from_rgb(70, 75, 90),
        );
    }

    // Draw smooth frequency response curve
    let step_x = rect.width() / (num_bins - 1) as f32;
    let mut prev_pt: Option<egui::Pos2> = None;

    for (i, &db) in state.spectrum_history.iter().enumerate() {
        let norm = ((db - min_db) / (max_db - min_db)).clamp(0.0, 1.0);
        let x = rect.min.x + i as f32 * step_x;
        let y = rect.max.y - (norm * rect.height());
        let current_pt = pos2(x, y);

        if let Some(prev) = prev_pt {
            painter.line_segment(
                [prev, current_pt],
                Stroke::new(1.5_f32, Color32::from_rgb(80, 220, 150)),
            );
        }
        prev_pt = Some(current_pt);
    }
}
