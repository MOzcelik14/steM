//! steM. - High-performance non-destructive audio editor and mixer
//!
//! Entry point initializing eframe, setting fonts with complete Latin Extended-A coverage,
//! and running the application window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod state;
mod views;

use app::StemApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(stem_i18n::APP_DISPLAY_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 550.0]),
        ..Default::default()
    };

    eframe::run_native(
        stem_i18n::APP_DISPLAY_NAME,
        native_options,
        Box::new(|cc| {
            setup_custom_fonts(&cc.egui_ctx);
            configure_app_theme(&cc.egui_ctx);
            Ok(Box::new(StemApp::new(cc)))
        }),
    )
}

/// Configures fonts ensuring complete Latin Extended-A coverage for Turkish glyphs (ç ğ ı İ ö ş ü).
fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Check system for DejaVu Sans or Noto Sans which has full Latin Extended-A coverage
    let font_candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ];

    for path in font_candidates {
        if let Ok(font_bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "extended_latin".to_owned(),
                egui::FontData::from_owned(font_bytes),
            );

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "extended_latin".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("extended_latin".to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);
}

/// Configures modern, clean audio workstation visual styles.
fn configure_app_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(22, 24, 28);
    visuals.window_fill = egui::Color32::from_rgb(26, 28, 33);
    visuals.selection.bg_fill = egui::Color32::from_rgb(90, 180, 255);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 32, 38);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 40, 48);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(50, 54, 65);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 65, 80);
    ctx.set_visuals(visuals);
}
