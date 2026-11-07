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
    let icon_data = egui::IconData {
        rgba: include_bytes!("../../../assets/icon.rgba").to_vec(),
        width: 256,
        height: 256,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(stem_i18n::APP_DISPLAY_NAME)
            .with_icon(icon_data)
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

/// Configures modern, clean audio workstation visual styles with generous breathing room.
fn configure_app_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(18, 20, 25);
    visuals.window_fill = egui::Color32::from_rgb(22, 24, 30);
    visuals.extreme_bg_color = egui::Color32::from_rgb(12, 13, 16);
    visuals.selection.bg_fill = egui::Color32::from_rgb(40, 110, 210);

    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(24, 26, 33);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 42, 52));

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 31, 40);
    visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 50, 62));

    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 45, 58);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(65, 74, 92));

    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(48, 56, 74);
    visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(56, 189, 248));

    visuals.window_rounding = egui::Rounding::same(8.0);
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(5.0);
    visuals.widgets.inactive.rounding = egui::Rounding::same(5.0);
    visuals.widgets.hovered.rounding = egui::Rounding::same(5.0);
    visuals.widgets.active.rounding = egui::Rounding::same(5.0);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(10.0);
    ctx.set_style(style);
}
