//! Real-time FFT spectral analyzer and spectrogram view

use crate::app::StemApp;
use egui::{pos2, vec2, Color32, Rounding, Stroke};
use stem_dsp::SpectrumAnalyzer;

pub struct SpectrogramState {
    analyzer: SpectrumAnalyzer,
    spectrum_history: Vec<f32>,
}

impl Default for SpectrogramState {
    fn default() -> Self {
        Self {
            analyzer: SpectrumAnalyzer::new(1024),
            spectrum_history: vec![-90.0; 513],
        }
    }
}

pub fn render_spectrogram_panel(app: &mut StemApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(app.i18n.t("view-spectrogram")).strong());
        });

        let available_size = vec2(ui.available_width(), 140.0);
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(15, 16, 20));
        painter.rect_stroke(
            rect,
            Rounding::same(4.0),
            Stroke::new(1.0_f32, Color32::from_rgb(45, 45, 55)),
        );

        // Extract current audio slice around playhead from timeline buffer for FFT
        let fft_size = app.spectrogram_state.analyzer.fft_size();
        let mut audio_slice = vec![0.0f32; fft_size];

        if let Some(ref timeline) = app.active_timeline {
            let playhead = app.telemetry.playhead_frame as usize;
            let total = timeline.total_frames as usize;
            let ch = &timeline.channels[0];

            for (i, sample) in audio_slice.iter_mut().enumerate().take(fft_size) {
                let frame = playhead + i;
                if frame < total && frame < ch.len() {
                    *sample = ch[frame];
                }
            }
        }

        let mag_db = app
            .spectrogram_state
            .analyzer
            .compute_magnitude_db(&audio_slice);

        // Smooth with history for visualization
        for (hist, &curr) in app
            .spectrogram_state
            .spectrum_history
            .iter_mut()
            .zip(mag_db.iter())
        {
            if curr > *hist {
                *hist = curr;
            } else {
                *hist = *hist * 0.85 + curr * 0.15;
            }
        }

        // Draw frequency response curve
        let num_bins = app.spectrogram_state.spectrum_history.len();
        if num_bins > 1 {
            let step_x = rect.width() / num_bins as f32;
            let min_db = -90.0f32;
            let max_db = 0.0f32;

            let mut prev_pt: Option<egui::Pos2> = None;

            for (i, &db) in app.spectrogram_state.spectrum_history.iter().enumerate() {
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
    });
}
