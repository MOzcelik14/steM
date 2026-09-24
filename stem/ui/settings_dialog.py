"""
steM. - Preferences & Settings Dialog
Configures default models, language, export formats, directories, and hardware options.
Created by M. Özçelik
"""

from pathlib import Path
from typing import Optional
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from stem.config import settings
from stem.core.hardware import get_hardware_info
from stem.i18n import add_language_listener, set_language, t


class SettingsDialog(Adw.PreferencesWindow):
    """Preferences window for steM."""

    def __init__(self, parent_window: Gtk.Window) -> None:
        super().__init__()
        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_default_size(520, 520)

        self._build_ui()
        add_language_listener(self.update_locale)

    def _build_ui(self) -> None:
        self.page = Adw.PreferencesPage()
        self.page.set_title(t("pref_page_general"))
        self.page.set_icon_name("preferences-system-symbolic")

        # Group 1: Interface Language
        self.group_lang = Adw.PreferencesGroup()
        self.group_lang.set_title(t("pref_group_language"))

        self.lang_row = Adw.ComboRow()
        self.lang_row.set_title(t("pref_language_title"))
        self.lang_row.set_subtitle(t("pref_language_sub"))
        self._refresh_lang_options()

        current_lang = settings.get("language", "auto")
        lang_idx = {"auto": 0, "tr": 1, "en": 2}.get(current_lang, 0)
        self.lang_row.set_selected(lang_idx)
        self.lang_row.connect("notify::selected", self._on_lang_changed)
        self.group_lang.add(self.lang_row)
        self.page.add(self.group_lang)

        # Group 2: Audio Separation Defaults
        self.group_ai = Adw.PreferencesGroup()
        self.group_ai.set_title(t("pref_group_ai"))

        # Default Model
        self.model_row = Adw.ComboRow()
        self.model_row.set_title(t("pref_model_title"))
        self._refresh_model_options()
        self.model_keys = ["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"]
        current_model = settings.get("model", "htdemucs")
        if current_model in self.model_keys:
            self.model_row.set_selected(self.model_keys.index(current_model))
        self.model_row.connect("notify::selected", self._on_model_changed)
        self.group_ai.add(self.model_row)

        # Low VRAM Optimization Switch
        self.vram_switch = Adw.SwitchRow()
        self.vram_switch.set_title(t("pref_vram_title"))
        self.vram_switch.set_subtitle(t("pref_vram_sub"))
        self.vram_switch.set_active(settings.get("low_vram_mode", True))
        self.vram_switch.connect("notify::active", lambda r, p: settings.set("low_vram_mode", r.get_active()))
        self.group_ai.add(self.vram_switch)
        self.page.add(self.group_ai)

        # Group 3: Output & Export
        self.group_export = Adw.PreferencesGroup()
        self.group_export.set_title(t("pref_group_export"))

        # Destination Folder
        self.dir_row = Adw.ActionRow()
        self.dir_row.set_title(t("pref_dest_title"))
        self.dir_row.set_subtitle(settings.get("output_dir"))
        pick_btn = Gtk.Button.new_from_icon_name("folder-open-symbolic")
        pick_btn.set_valign(Gtk.Align.CENTER)
        pick_btn.connect("clicked", self._on_pick_dir)
        self.dir_row.add_suffix(pick_btn)
        self.group_export.add(self.dir_row)

        # Default Export Format
        self.fmt_row = Adw.ComboRow()
        self.fmt_row.set_title(t("pref_format_title"))
        self._refresh_format_options()
        self.fmt_row.set_selected(0 if settings.get("export_format") == "wav" else 1)
        self.fmt_row.connect("notify::selected", lambda r, p: settings.set("export_format", "wav" if r.get_selected() == 0 else "mp3"))
        self.group_export.add(self.fmt_row)

        # MP3 Bitrate
        self.bitrate_row = Adw.ComboRow()
        self.bitrate_row.set_title(t("pref_bitrate_title"))
        self.bitrate_row.set_model(Gtk.StringList.new(["320 kbps", "256 kbps", "192 kbps"]))
        br_map = {320: 0, 256: 1, 192: 2}
        self.bitrate_row.set_selected(br_map.get(settings.get("mp3_bitrate", 320), 0))
        self.bitrate_row.connect("notify::selected", self._on_bitrate_changed)
        self.group_export.add(self.bitrate_row)
        self.page.add(self.group_export)

        # Group 4: Hardware Diagnostics
        self.group_hw = Adw.PreferencesGroup()
        self.group_hw.set_title(t("pref_group_hw"))
        hw = get_hardware_info()

        self.gpu_row = Adw.ActionRow()
        self.gpu_row.set_title(t("pref_gpu_device"))
        self.gpu_row.set_subtitle(hw.name)
        self.group_hw.add(self.gpu_row)

        self.vram_row = Adw.ActionRow()
        self.vram_row.set_title(t("pref_vram"))
        self.vram_row.set_subtitle(f"{hw.vram_total_mb} MB (Free: {hw.vram_free_mb} MB)")
        self.group_hw.add(self.vram_row)

        self.driver_row = Adw.ActionRow()
        self.driver_row.set_title(t("pref_driver"))
        self.driver_row.set_subtitle(hw.driver_version)
        self.group_hw.add(self.driver_row)
        self.page.add(self.group_hw)

        self.add(self.page)
        self.set_title(t("pref_title"))

    def _refresh_lang_options(self) -> None:
        self.lang_row.set_model(Gtk.StringList.new([
            t("lang_auto"),
            t("lang_tr"),
            t("lang_en"),
        ]))

    def _refresh_model_options(self) -> None:
        self.model_row.set_model(Gtk.StringList.new([
            t("model_htdemucs"),
            t("model_htdemucs_ft"),
            t("model_htdemucs_6s"),
            t("model_mdx_extra"),
        ]))

    def _refresh_format_options(self) -> None:
        self.fmt_row.set_model(Gtk.StringList.new([
            t("format_wav"),
            t("format_mp3"),
        ]))

    def update_locale(self, lang: Optional[str] = None) -> None:
        """Dynamically updates all strings in the settings dialog."""
        self.set_title(t("pref_title"))
        self.page.set_title(t("pref_page_general"))
        self.group_lang.set_title(t("pref_group_language"))
        self.lang_row.set_title(t("pref_language_title"))
        self.lang_row.set_subtitle(t("pref_language_sub"))
        self._refresh_lang_options()

        self.group_ai.set_title(t("pref_group_ai"))
        self.model_row.set_title(t("pref_model_title"))
        self._refresh_model_options()
        self.vram_switch.set_title(t("pref_vram_title"))
        self.vram_switch.set_subtitle(t("pref_vram_sub"))

        self.group_export.set_title(t("pref_group_export"))
        self.dir_row.set_title(t("pref_dest_title"))
        self.fmt_row.set_title(t("pref_format_title"))
        self._refresh_format_options()
        self.bitrate_row.set_title(t("pref_bitrate_title"))

        self.group_hw.set_title(t("pref_group_hw"))
        self.gpu_row.set_title(t("pref_gpu_device"))
        self.vram_row.set_title(t("pref_vram"))
        self.driver_row.set_title(t("pref_driver"))

    def _on_lang_changed(self, row, param) -> None:
        idx = row.get_selected()
        lang_keys = ["auto", "tr", "en"]
        if idx < len(lang_keys):
            chosen = lang_keys[idx]
            if chosen != settings.get("language"):
                set_language(chosen)

    def _on_model_changed(self, row, param) -> None:
        idx = row.get_selected()
        if idx < len(self.model_keys):
            settings.set("model", self.model_keys[idx])

    def _on_bitrate_changed(self, row, param) -> None:
        rates = [320, 256, 192]
        idx = row.get_selected()
        if idx < len(rates):
            settings.set("mp3_bitrate", rates[idx])

    def _on_pick_dir(self, btn) -> None:
        dialog = Gtk.FileDialog.new()
        dialog.set_title(t("pref_select_dest"))
        dialog.select_folder(self, None, self._on_dir_picked)

    def _on_dir_picked(self, dialog, result) -> None:
        try:
            folder = dialog.select_folder_finish(result)
            if folder:
                p = folder.get_path()
                self.dir_row.set_subtitle(p)
                settings.set("output_dir", p)
        except Exception:
            pass
