"""
steM. - Preferences & Settings Dialog
Configures default models, export formats, directories, and hardware options.
"""

from pathlib import Path
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from stem.config import settings
from stem.core.hardware import get_hardware_info


class SettingsDialog(Adw.PreferencesWindow):
    """Preferences window for steM."""

    def __init__(self, parent_window: Gtk.Window) -> None:
        super().__init__()
        self.set_transient_for(parent_window)
        self.set_modal(True)
        self.set_title("Preferences — steM.")
        self.set_default_size(520, 480)

        page = Adw.PreferencesPage()
        page.set_title("General")
        page.set_icon_name("preferences-system-symbolic")

        # Group 1: Audio Separation Defaults
        group_ai = Adw.PreferencesGroup()
        group_ai.set_title("Demucs Separation Defaults")

        # Default Model
        self.model_row = Adw.ComboRow()
        self.model_row.set_title("Default Model")
        self.model_row.set_model(Gtk.StringList.new([
            "HTDemucs v4 (Default)",
            "HTDemucs Fine-Tuned",
            "HTDemucs 6-Stems",
            "MDX-Net Extra",
        ]))
        model_keys = ["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"]
        current_model = settings.get("model", "htdemucs")
        if current_model in model_keys:
            self.model_row.set_selected(model_keys.index(current_model))
        self.model_row.connect("notify::selected", self._on_model_changed)
        group_ai.add(self.model_row)

        # Low VRAM Optimization Switch
        self.vram_switch = Adw.SwitchRow()
        self.vram_switch.set_title("Low VRAM Mode (4 GB RTX 3050 Optimization)")
        self.vram_switch.set_subtitle("Restricts segment size to 6s and jobs to 1 to avoid CUDA OOM")
        self.vram_switch.set_active(settings.get("low_vram_mode", True))
        self.vram_switch.connect("notify::active", lambda r, p: settings.set("low_vram_mode", r.get_active()))
        group_ai.add(self.vram_switch)

        page.add(group_ai)

        # Group 2: Output & Export
        group_export = Adw.PreferencesGroup()
        group_export.set_title("Export & File Storage")

        # Destination Folder
        self.dir_row = Adw.ActionRow()
        self.dir_row.set_title("Default Stems Destination")
        self.dir_row.set_subtitle(settings.get("output_dir"))
        pick_btn = Gtk.Button.new_from_icon_name("folder-open-symbolic")
        pick_btn.set_valign(Gtk.Align.CENTER)
        pick_btn.connect("clicked", self._on_pick_dir)
        self.dir_row.add_suffix(pick_btn)
        group_export.add(self.dir_row)

        # Default Export Format
        self.fmt_row = Adw.ComboRow()
        self.fmt_row.set_title("Default Export Format")
        self.fmt_row.set_model(Gtk.StringList.new(["WAV (24-bit PCM)", "MP3 (Compressed)"]))
        self.fmt_row.set_selected(0 if settings.get("export_format") == "wav" else 1)
        self.fmt_row.connect("notify::selected", lambda r, p: settings.set("export_format", "wav" if r.get_selected() == 0 else "mp3"))
        group_export.add(self.fmt_row)

        # MP3 Bitrate
        self.bitrate_row = Adw.ComboRow()
        self.bitrate_row.set_title("MP3 Bitrate")
        self.bitrate_row.set_model(Gtk.StringList.new(["320 kbps (Studio)", "256 kbps", "192 kbps"]))
        br_map = {320: 0, 256: 1, 192: 2}
        self.bitrate_row.set_selected(br_map.get(settings.get("mp3_bitrate", 320), 0))
        self.bitrate_row.connect("notify::selected", self._on_bitrate_changed)
        group_export.add(self.bitrate_row)

        page.add(group_export)

        # Group 3: Hardware Diagnostics
        group_hw = Adw.PreferencesGroup()
        group_hw.set_title("Hardware Diagnostics")
        hw = get_hardware_info()

        gpu_row = Adw.ActionRow()
        gpu_row.set_title("GPU Device")
        gpu_row.set_subtitle(hw.name)
        group_hw.add(gpu_row)

        vram_row = Adw.ActionRow()
        vram_row.set_title("Dedicated VRAM")
        vram_row.set_subtitle(f"{hw.vram_total_mb} MB (Free: {hw.vram_free_mb} MB)")
        group_hw.add(vram_row)

        driver_row = Adw.ActionRow()
        driver_row.set_title("NVIDIA Driver")
        driver_row.set_subtitle(hw.driver_version)
        group_hw.add(driver_row)

        page.add(group_hw)
        self.add(page)

    def _on_model_changed(self, row, param) -> None:
        model_keys = ["htdemucs", "htdemucs_ft", "htdemucs_6s", "mdx_extra"]
        idx = row.get_selected()
        if idx < len(model_keys):
            settings.set("model", model_keys[idx])

    def _on_bitrate_changed(self, row, param) -> None:
        rates = [320, 256, 192]
        idx = row.get_selected()
        if idx < len(rates):
            settings.set("mp3_bitrate", rates[idx])

    def _on_pick_dir(self, btn) -> None:
        dialog = Gtk.FileDialog.new()
        dialog.set_title("Select Default Destination Directory")
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
