//! Localization and internationalization for steM.
//!
//! Provides compile-time embedded Fluent bundles for English and Turkish,
//! runtime locale switching, audio-specific number/time formatting, and
//! the canonical application display name.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use unic_langid::LanguageIdentifier;

/// The single source of truth for the application's user-facing display name.
pub const APP_DISPLAY_NAME: &str = "steM.";

pub const EN_FTL: &str = include_str!("../../../locales/en.ftl");
pub const TR_FTL: &str = include_str!("../../../locales/tr.ftl");

/// Supported locales in steM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Locale {
    #[default]
    En,
    Tr,
}

impl Locale {
    pub const ALL: &'static [Locale] = &[Locale::En, Locale::Tr];

    pub fn code(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Tr => "tr",
        }
    }

    pub fn native_name(&self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Tr => "Türkçe",
        }
    }

    /// Detect locale from system settings, falling back to English.
    pub fn from_system() -> Self {
        if let Some(locale_str) = sys_locale::get_locale() {
            let lower = locale_str.to_ascii_lowercase();
            if lower.starts_with("tr") {
                return Locale::Tr;
            }
        }
        Locale::En
    }
}

/// Central localization manager containing bundles for all supported languages.
#[derive(Clone)]
pub struct I18n {
    current_locale: Locale,
    en_bundle: Rc<FluentBundle<FluentResource>>,
    tr_bundle: Rc<FluentBundle<FluentResource>>,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Locale::from_system())
    }
}

impl I18n {
    /// Creates a new I18n instance initialized with the given or system locale.
    pub fn new(locale: Locale) -> Self {
        let en_lang: LanguageIdentifier = "en-US".parse().expect("valid en-US langid");
        let mut en_bundle = FluentBundle::new(vec![en_lang]);
        let en_res = FluentResource::try_new(EN_FTL.to_string())
            .expect("Failed to parse embedded English FTL");
        en_bundle
            .add_resource(en_res)
            .expect("Failed to add English resource");

        let tr_lang: LanguageIdentifier = "tr-TR".parse().expect("valid tr-TR langid");
        let mut tr_bundle = FluentBundle::new(vec![tr_lang]);
        let tr_res = FluentResource::try_new(TR_FTL.to_string())
            .expect("Failed to parse embedded Turkish FTL");
        tr_bundle
            .add_resource(tr_res)
            .expect("Failed to add Turkish resource");

        Self {
            current_locale: locale,
            en_bundle: Rc::new(en_bundle),
            tr_bundle: Rc::new(tr_bundle),
        }
    }

    /// Sets the active locale. Changes take effect immediately in the UI.
    pub fn set_locale(&mut self, locale: Locale) {
        self.current_locale = locale;
    }

    /// Gets the current active locale.
    pub fn current_locale(&self) -> Locale {
        self.current_locale
    }

    fn bundle_for(&self, locale: Locale) -> &FluentBundle<FluentResource> {
        match locale {
            Locale::En => &self.en_bundle,
            Locale::Tr => &self.tr_bundle,
        }
    }

    /// Translates a key using the current locale, falling back to English if missing.
    pub fn t(&self, key: &str) -> String {
        self.t_args(key, &HashMap::new())
    }

    /// Translates a key with arguments.
    pub fn t_args<'a>(&self, key: &str, args: &HashMap<&str, FluentValue<'a>>) -> String {
        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, v.clone());
        }

        // Try active locale
        let bundle = self.bundle_for(self.current_locale);
        if let Some(msg) = bundle.get_message(key) {
            if let Some(pattern) = msg.value() {
                let mut errors = vec![];
                let formatted = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
                return formatted.into_owned();
            }
        }

        // Fallback to English if active wasn't English
        if self.current_locale != Locale::En {
            let en_bundle = &self.en_bundle;
            if let Some(msg) = en_bundle.get_message(key) {
                if let Some(pattern) = msg.value() {
                    let mut errors = vec![];
                    let formatted =
                        en_bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
                    return formatted.into_owned();
                }
            }
        }

        // Fallback to raw key if missing in all bundles
        key.to_string()
    }

    /// Helper translating with a single numerical or text argument
    pub fn t_arg<'a>(
        &self,
        key: &str,
        arg_key: &'a str,
        arg_val: impl Into<FluentValue<'a>>,
    ) -> String {
        let mut map = HashMap::new();
        map.insert(arg_key, arg_val.into());
        self.t_args(key, &map)
    }
}

// -----------------------------------------------------------------------------
// Audio Formatting Helpers
// -----------------------------------------------------------------------------

/// Formats seconds into HH:MM:SS.mmm or MM:SS.mmm format.
pub fn format_time_code(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "00:00.000".to_string();
    }
    let total_ms = (seconds * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_secs = total_ms / 1000;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;

    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}.{ms:03}")
    } else {
        format!("{m:02}:{s:02}.{ms:03}")
    }
}

/// Formats sample count with thousands separators (e.g. 1,440,000).
pub fn format_samples(samples: u64) -> String {
    let s = samples.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}

/// Formats decibels with standard "-inf dB" threshold.
pub fn format_db(db: f32) -> String {
    if !db.is_finite() || db <= -90.0 {
        "-inf dB".to_string()
    } else if db > 0.0 {
        format!("+{:.1} dB", db)
    } else {
        format!("{:.1} dB", db)
    }
}

/// Formats loudness in LUFS (Loudness Units Full Scale).
pub fn format_lufs(lufs: f32) -> String {
    if !lufs.is_finite() || lufs <= -90.0 {
        "-inf LUFS".to_string()
    } else {
        format!("{:.1} LUFS", lufs)
    }
}

/// Formats sample rate nicely (e.g. 44100 -> "44.1 kHz", 48000 -> "48.0 kHz").
pub fn format_sample_rate(hz: u32) -> String {
    if hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{:.1} kHz", hz as f64 / 1000.0)
    }
}

/// Formats frequency in Hz or kHz.
pub fn format_frequency(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}

/// Timecode display mode options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimecodeMode {
    #[default]
    TimeCode,
    Smpte,
    BarsBeats,
    Samples,
}

/// Formats seconds as SMPTE timecode: HH:MM:SS:FF.
pub fn format_smpte(seconds: f64, fps: f32) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "00:00:00:00".to_string();
    }
    let fps = if fps <= 0.0 { 30.0 } else { fps };
    let total_frames = (seconds * fps as f64).round() as u64;
    let frames = total_frames % (fps as u64);
    let total_secs = total_frames / (fps as u64);
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;

    format!("{h:02}:{m:02}:{s:02}:{frames:02}")
}

/// Formats seconds as Musical Bar.Beat.Tick (based on tempo in BPM and time signature).
pub fn format_bars_beats(seconds: f64, tempo_bpm: f64, time_sig: (u8, u8)) -> String {
    if !seconds.is_finite() || seconds < 0.0 || tempo_bpm <= 0.0 {
        return "1.1.000".to_string();
    }
    let beats_per_sec = tempo_bpm / 60.0;
    let total_beats = seconds * beats_per_sec;
    let beats_per_bar = time_sig.0.max(1) as f64;

    let bar = (total_beats / beats_per_bar).floor() as u64 + 1;
    let beat_in_bar = (total_beats % beats_per_bar).floor() as u64 + 1;
    let fractional_beat = total_beats % 1.0;
    let ticks = (fractional_beat * 960.0).round() as u64; // Standard 960 PPQN

    format!("{bar}.{beat_in_bar}.{ticks:03}")
}
