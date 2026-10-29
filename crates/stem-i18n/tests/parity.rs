use fluent_bundle::FluentResource;
use std::collections::BTreeSet;
use stem_i18n::{EN_FTL, TR_FTL};

fn extract_keys(ftl_content: &str) -> BTreeSet<String> {
    // 1. Ensure the content is 100% valid Fluent syntax
    let _res = FluentResource::try_new(ftl_content.to_string()).expect("Valid Fluent resource");

    // 2. Extract message identifiers
    let mut keys = BTreeSet::new();
    for line in ftl_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            keys.insert(key.trim().to_string());
        }
    }
    keys
}

#[test]
fn test_en_and_tr_exact_key_parity() {
    let en_keys = extract_keys(EN_FTL);
    let tr_keys = extract_keys(TR_FTL);

    let missing_in_tr: Vec<_> = en_keys.difference(&tr_keys).collect();
    let extra_in_tr: Vec<_> = tr_keys.difference(&en_keys).collect();

    assert!(
        missing_in_tr.is_empty(),
        "The following keys exist in en.ftl but are missing in tr.ftl: {:?}",
        missing_in_tr
    );

    assert!(
        extra_in_tr.is_empty(),
        "The following keys exist in tr.ftl but are missing in en.ftl: {:?}",
        extra_in_tr
    );

    assert!(!en_keys.is_empty(), "Translation files must not be empty");
    println!(
        "Parity verified: {} identical keys in EN and TR.",
        en_keys.len()
    );
}

#[test]
fn test_turkish_characters_preserved() {
    let tr_keys = extract_keys(TR_FTL);
    assert!(tr_keys.contains("menu-track-add-audio"));

    let i18n = stem_i18n::I18n::new(stem_i18n::Locale::Tr);
    let s = i18n.t("menu-track-add-audio");
    // Verify Turkish characters İ, z, etc. are properly returned
    assert_eq!(s, "Ses İzi Ekle");
    let s_zoom = i18n.t("menu-view-reset-zoom");
    assert_eq!(s_zoom, "Yakınlaştırmayı Sıfırla");
}

#[test]
fn test_app_display_name_constant() {
    assert_eq!(stem_i18n::APP_DISPLAY_NAME, "steM.");
}
