"""
steM. - Unit tests for internationalization (i18n) module.
"""

import unittest
from stem.i18n import (
    TRANSLATIONS,
    add_language_listener,
    get_language,
    set_language,
    t,
)


class TestI18n(unittest.TestCase):
    def setUp(self):
        # Save previous language to restore after tests
        self._initial_lang = get_language()

    def tearDown(self):
        set_language(self._initial_lang)

    def test_translations_dictionary_symmetry(self):
        """Verifies that English and Turkish have matching translation keys."""
        en_keys = set(TRANSLATIONS["en"].keys())
        tr_keys = set(TRANSLATIONS["tr"].keys())

        missing_in_tr = en_keys - tr_keys
        missing_in_en = tr_keys - en_keys

        self.assertEqual(missing_in_tr, set(), f"Keys missing in TR: {missing_in_tr}")
        self.assertEqual(missing_in_en, set(), f"Keys missing in EN: {missing_in_en}")

    def test_language_switching(self):
        """Tests that set_language changes effective translation."""
        set_language("en")
        self.assertEqual(get_language(), "en")
        self.assertEqual(t("tab_welcome"), "Welcome")
        self.assertEqual(t("tab_separation"), "Separation")
        self.assertEqual(t("tab_mixer"), "Mixer Studio")

        set_language("tr")
        self.assertEqual(get_language(), "tr")
        self.assertEqual(t("tab_welcome"), "Giriş")
        self.assertEqual(t("tab_separation"), "Ayrıştırma")
        self.assertEqual(t("tab_mixer"), "Mikser Stüdyosu")

    def test_formatted_translations(self):
        """Tests keyword replacement in translations."""
        set_language("en")
        self.assertEqual(t("by_author", author="M. Özçelik"), "by M. Özçelik")
        self.assertEqual(t("status_processing", pct=50), "Separating... 50%")

        set_language("tr")
        self.assertEqual(t("by_author", author="M. Özçelik"), "Geliştirici: M. Özçelik")
        self.assertEqual(t("status_processing", pct=50), "Ayrıştırılıyor... %50")

    def test_fallback_for_unknown_key(self):
        """Tests fallback behavior for missing translation keys."""
        self.assertEqual(t("non_existent_key_xyz"), "non_existent_key_xyz")

    def test_listener_notification(self):
        """Tests that language change callbacks are executed."""
        events = []

        def on_change(lang):
            events.append(lang)

        add_language_listener(on_change)
        set_language("en")
        self.assertIn("en", events)

        set_language("tr")
        self.assertIn("tr", events)


if __name__ == "__main__":
    unittest.main()
