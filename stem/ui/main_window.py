"""
steM. - Main Application Window
Coordinates Adw.ApplicationWindow views, header bar, drag-and-drop, and OOM handling.
"""

from pathlib import Path
from typing import List, Optional
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from stem.config import APP_NAME, APP_TAGLINE, settings
from stem.core.audio_metadata import AudioFileInfo, inspect_audio_file
from stem.core.exporter import AudioExporter, ExportOptions
from stem.core.hardware import get_hardware_info
from stem.core.queue_manager import QueueItem, QueueManager
from stem.ui.about_dialog import show_about_dialog
from stem.ui.mixer_view import MixerView
from stem.ui.separation_view import SeparationView
from stem.ui.settings_dialog import SettingsDialog
from stem.ui.welcome_view import WelcomeView


class MainWindow(Adw.ApplicationWindow):
    """Main window for steM. featuring ViewStack, HeaderBar, and audio drag-and-drop."""

    def __init__(self, app: Adw.Application) -> None:
        super().__init__(application=app)
        self.set_title(f"{APP_NAME} — AI Audio Separation Studio")
        self.set_default_size(980, 680)

        # Enforce dark theme for professional DAW look
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.FORCE_DARK)

        self.queue_manager = QueueManager()
        self.queue_manager.on_oom_detected = self._handle_oom_event

        self.loaded_tracks: List[AudioFileInfo] = []

        self._build_header_and_views()
        self._setup_window_drop_target()

    def _build_header_and_views(self) -> None:
        # Main layout container
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # Header Bar
        self.header_bar = Adw.HeaderBar()

        # Custom Title Box with steM. branding
        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        title_box.set_valign(Gtk.Align.CENTER)

        main_title = Gtk.Label(label="steM.")
        main_title.set_css_classes(["heading"])
        sub_title = Gtk.Label(label="AI Audio Separation Studio")
        sub_title.set_css_classes(["caption", "dim-label"])

        title_box.append(main_title)
        title_box.append(sub_title)
        self.header_bar.set_title_widget(title_box)

        # Left Header: Hardware indicator pill
        hw = get_hardware_info()
        hw_label = Gtk.Label()
        if hw.cuda_available:
            hw_label.set_label(f"CUDA: {hw.name.split()[-1]} (4GB)")
            hw_label.set_css_classes(["hw-pill-cuda"])
        else:
            hw_label.set_label("CPU Mode")
            hw_label.set_css_classes(["hw-pill-cpu"])
        self.header_bar.pack_start(hw_label)

        # Right Header: Menu Button (Preferences, About)
        menu = Gio.Menu.new()
        menu.append("Preferences", "app.preferences")
        menu.append("About steM.", "app.about")

        menu_btn = Gtk.MenuButton()
        menu_btn.set_icon_name("open-menu-symbolic")
        menu_btn.set_menu_model(menu)
        self.header_bar.pack_end(menu_btn)

        # View Switcher in HeaderBar
        self.view_stack = Adw.ViewStack()

        # View 1: Welcome / Drop Zone
        self.welcome_view = WelcomeView(on_files_selected=self.import_files)
        self.view_stack.add_titled_with_icon(self.welcome_view, "welcome", "Welcome", "folder-music-symbolic")

        # View 2: Separation Queue & Controls
        self.separation_view = SeparationView(
            queue_manager=self.queue_manager,
            on_add_files_requested=self._on_browse_files,
            on_open_mixer=self._on_open_mixer_for_item,
        )
        self.view_stack.add_titled_with_icon(self.separation_view, "separation", "Separation", "edit-cut-symbolic")

        # View 3: Mixer Studio
        self.mixer_view = MixerView(on_export_requested=self._on_export_current_stems)
        self.view_stack.add_titled_with_icon(self.mixer_view, "mixer", "Mixer Studio", "audio-volume-high-symbolic")

        # View Switcher Title
        view_switcher_title = Adw.ViewSwitcherTitle()
        view_switcher_title.set_stack(self.view_stack)
        view_switcher_title.set_title("steM.")
        view_switcher_title.set_subtitle("AI Audio Separation Studio")
        self.header_bar.set_title_widget(view_switcher_title)

        main_box.append(self.header_bar)

        # Toast Overlay
        self.toast_overlay = Adw.ToastOverlay()
        self.toast_overlay.set_child(self.view_stack)
        main_box.append(self.toast_overlay)

        self.set_content(main_box)

    def _setup_window_drop_target(self) -> None:
        """Allows dropping audio files anywhere onto the window."""
        target = Gtk.DropTarget.new(Gdk.FileList, Gdk.DragAction.COPY)
        target.connect("drop", self._on_window_drop)
        self.add_controller(target)

    def _on_window_drop(self, target, value, x, y) -> bool:
        if isinstance(value, Gdk.FileList):
            files = [Path(f.get_path()) for f in value.get_files() if f.get_path()]
            if files:
                self.import_files(files)
                return True
        return False

    def import_files(self, file_paths: List[Path]) -> None:
        """Inspects and queues imported audio files."""
        valid_added = 0
        for p in file_paths:
            info = inspect_audio_file(p)
            if info.is_valid:
                self.loaded_tracks.append(info)
                self.separation_view.add_track_to_queue(info)
                valid_added += 1
            else:
                self.show_toast(f"Skipped {p.name}: {info.error_message}")

        if valid_added > 0:
            self.show_toast(f"Added {valid_added} audio track(s) to separation queue.")
            self.view_stack.set_visible_child_name("separation")

    def _on_browse_files(self) -> None:
        dialog = Gtk.FileDialog.new()
        dialog.set_title("Import Audio Files — steM.")
        dialog.open_multiple(self, None, self._on_browse_result)

    def _on_browse_result(self, dialog, result) -> None:
        try:
            files = dialog.open_multiple_finish(result)
            if files:
                paths = [Path(files.get_item(i).get_path()) for i in range(files.get_n_items())]
                self.import_files(paths)
        except Exception:
            pass

    def _on_open_mixer_for_item(self, item: QueueItem) -> None:
        """Switches to the Mixer Studio with the separated stems loaded."""
        if item.stems:
            self.mixer_view.load_stems_session(item.file_info, item.stems)
            self.view_stack.set_visible_child_name("mixer")
            self.show_toast(f"Loaded stems for '{item.file_info.filename}' in Mixer.")

    def _on_export_current_stems(self) -> None:
        """Exports currently loaded mixer stems to user destination."""
        if not self.mixer_view.stems_map:
            self.show_toast("No stems available to export.")
            return

        out_dir = Path(settings.get("output_dir"))
        track_name = (
            self.mixer_view.current_track_info.title
            if self.mixer_view.current_track_info
            else "Separated_Stems"
        )
        opts = ExportOptions(
            destination_dir=out_dir,
            track_title=track_name,
            export_format=settings.get("export_format", "wav"),
            mp3_bitrate=settings.get("mp3_bitrate", 320),
            prevent_overwrite=True,
        )

        try:
            exported = AudioExporter.export_stems(self.mixer_view.stems_map, opts)
            toast = Adw.Toast.new(f"Exported {len(exported)} stems to {out_dir.name}/{opts.track_title}")
            toast.set_button_label("Open Folder")
            target_folder = next(iter(exported.values())).parent if exported else out_dir
            toast.connect("button-clicked", lambda t: Gio.AppInfo.launch_default_for_uri(target_folder.as_uri(), None))
            self.toast_overlay.add_toast(toast)
        except Exception as e:
            self.show_toast(f"Export failed: {e}")

    def _handle_oom_event(self, item: QueueItem, msg: str) -> None:
        """Prompts user to retry on CPU when CUDA OOM happens on 4GB VRAM."""
        dialog = Adw.AlertDialog.new(
            "NVIDIA GPU Out of Memory (CUDA OOM)",
            f"The track '{item.file_info.filename}' exceeded the 4 GB VRAM limit of your RTX 3050.\n\n"
            "Would you like to retry separation on CPU?",
        )
        dialog.add_response("cancel", "Cancel")
        dialog.add_response("retry_cpu", "Retry on CPU")
        dialog.set_response_appearance("retry_cpu", Adw.ResponseAppearance.SUGGESTED)

        def _on_response(d, response_id):
            if response_id == "retry_cpu":
                item.task.device = "cpu"
                item.status = item.status.PENDING
                item.status_text = "Retrying on CPU..."
                self.queue_manager._notify_item(item)
                self.queue_manager.start_queue()

        dialog.connect("response", _on_response)
        dialog.present(self)

    def show_toast(self, message: str) -> None:
        toast = Adw.Toast.new(message)
        self.toast_overlay.add_toast(toast)
