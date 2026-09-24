"""
steM. - Startup Welcome View & Drag-and-Drop Area
Features brand identity, drag-and-drop drop zone, and hardware status.
"""

from pathlib import Path
from typing import Callable, List, Optional
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, Gtk

from stem.config import APP_AUTHOR, APP_NAME, APP_SUBTITLE
from stem.core.hardware import get_hardware_info
from stem.i18n import add_language_listener, t


class WelcomeView(Gtk.Box):
    """Initial landing view with branding and drag-and-drop audio import."""

    def __init__(self, on_files_selected: Callable[[List[Path]], None]) -> None:
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=16)
        self.on_files_selected = on_files_selected
        self.set_vexpand(True)
        self.set_hexpand(True)
        self.set_halign(Gtk.Align.CENTER)
        self.set_valign(Gtk.Align.CENTER)
        self.set_margin_top(24)
        self.set_margin_bottom(24)
        self.set_margin_start(24)
        self.set_margin_end(24)

        self._build_ui()
        self._setup_drop_target()
        add_language_listener(self.update_locale)

    def _build_ui(self) -> None:
        # Brand Container
        welcome_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        welcome_box.set_css_classes(["welcome-box"])
        welcome_box.set_halign(Gtk.Align.CENTER)

        # Brand Title
        title_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0)
        title_box.set_halign(Gtk.Align.CENTER)

        stem_label = Gtk.Label(label="ste")
        stem_label.set_css_classes(["brand-title"])
        m_label = Gtk.Label(label="M.")
        m_label.set_css_classes(["brand-title", "brand-title-accent"])

        title_box.append(stem_label)
        title_box.append(m_label)
        welcome_box.append(title_box)

        # Creator Attribution
        self.author_label = Gtk.Label(label=t("by_author", author=APP_AUTHOR))
        self.author_label.set_css_classes(["brand-author"])
        welcome_box.append(self.author_label)

        # Subtitle / Studio description
        self.subtitle_label = Gtk.Label(label=t("app_subtitle"))
        self.subtitle_label.set_css_classes(["brand-tagline"])
        welcome_box.append(self.subtitle_label)

        self.append(welcome_box)

        # Drag and Drop Drop-Zone Card
        self.drop_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self.drop_box.set_css_classes(["drop-zone"])
        self.drop_box.set_size_request(460, 200)

        drop_icon = Gtk.Image.new_from_icon_name("folder-music-symbolic")
        drop_icon.set_pixel_size(48)
        drop_icon.set_css_classes(["drop-zone-icon"])
        self.drop_box.append(drop_icon)

        self.drop_hint = Gtk.Label(label=t("drag_drop_title"))
        self.drop_hint.set_css_classes(["heading"])
        self.drop_box.append(self.drop_hint)

        self.format_hint = Gtk.Label(label=t("supported_formats"))
        self.format_hint.set_css_classes(["dim-label"])
        self.drop_box.append(self.format_hint)

        self.browse_btn = Gtk.Button(label=t("browse_files"))
        self.browse_btn.set_css_classes(["accent-button", "pill"])
        self.browse_btn.set_halign(Gtk.Align.CENTER)
        self.browse_btn.connect("clicked", self._on_browse_clicked)
        self.drop_box.append(self.browse_btn)

        self.append(self.drop_box)

        # Hardware Info Status Pill
        self.hw = get_hardware_info()
        hw_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        hw_box.set_halign(Gtk.Align.CENTER)

        self.hw_pill = Gtk.Label()
        self._update_hw_label()
        hw_box.append(self.hw_pill)
        self.append(hw_box)

    def _update_hw_label(self) -> None:
        if self.hw.cuda_available:
            self.hw_pill.set_label(t("hw_cuda", name=self.hw.name, vram=self.hw.vram_total_mb))
            self.hw_pill.set_css_classes(["hw-pill-cuda"])
        else:
            self.hw_pill.set_label(t("hw_cpu"))
            self.hw_pill.set_css_classes(["hw-pill-cpu"])

    def update_locale(self, lang: Optional[str] = None) -> None:
        """Refreshes all displayed strings when language changes."""
        self.author_label.set_label(t("by_author", author=APP_AUTHOR))
        self.subtitle_label.set_label(t("app_subtitle"))
        self.drop_hint.set_label(t("drag_drop_title"))
        self.format_hint.set_label(t("supported_formats"))
        self.browse_btn.set_label(t("browse_files"))
        self._update_hw_label()

    def _setup_drop_target(self) -> None:
        """Sets up GTK4 drop target to handle files dragged from file manager."""
        target = Gtk.DropTarget.new(Gdk.FileList, Gdk.DragAction.COPY)
        target.connect("enter", self._on_drag_enter)
        target.connect("leave", self._on_drag_leave)
        target.connect("drop", self._on_drop)
        self.drop_box.add_controller(target)

    def _on_drag_enter(self, target, x, y) -> Gdk.DragAction:
        self.drop_box.add_css_class("drag-hover")
        return Gdk.DragAction.COPY

    def _on_drag_leave(self, target) -> None:
        self.drop_box.remove_css_class("drag-hover")

    def _on_drop(self, target, value, x, y) -> bool:
        self.drop_box.remove_css_class("drag-hover")
        if isinstance(value, Gdk.FileList):
            files = [Path(f.get_path()) for f in value.get_files() if f.get_path()]
            if files:
                self.on_files_selected(files)
                return True
        return False

    def _on_browse_clicked(self, btn) -> None:
        """Opens GTK4 FileDialog to pick audio files."""
        dialog = Gtk.FileDialog.new()
        dialog.set_title(t("select_audio_dialog_title"))

        # Audio file filter
        filters = Gio.ListStore.new(Gtk.FileFilter)
        audio_filter = Gtk.FileFilter()
        audio_filter.set_name(t("audio_files_filter"))
        audio_filter.add_mime_type("audio/*")
        audio_filter.add_pattern("*.mp3")
        audio_filter.add_pattern("*.wav")
        audio_filter.add_pattern("*.flac")
        audio_filter.add_pattern("*.ogg")
        audio_filter.add_pattern("*.m4a")
        audio_filter.add_pattern("*.aac")
        filters.append(audio_filter)
        dialog.set_filters(filters)

        root = self.get_root()
        dialog.open_multiple(root, None, self._on_files_chosen)

    def _on_files_chosen(self, dialog, result) -> None:
        try:
            file_list = dialog.open_multiple_finish(result)
            if file_list:
                paths = [Path(file_list.get_item(i).get_path()) for i in range(file_list.get_n_items())]
                if paths:
                    self.on_files_selected(paths)
        except Exception:
            pass
