"""
steM. - Application Entry Point & Lifecycle
Manages Adw.Application, actions, CSS provider, and window lifecycle.
"""

import sys
from pathlib import Path
from typing import List
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from stem.config import APP_ID, APP_NAME
from stem.ui.about_dialog import show_about_dialog
from stem.ui.main_window import MainWindow
from stem.ui.settings_dialog import SettingsDialog


class StemApplication(Adw.Application):
    """Main Adw.Application class for steM."""

    def __init__(self) -> None:
        super().__init__(
            application_id=APP_ID,
            flags=Gio.ApplicationFlags.HANDLES_OPEN | Gio.ApplicationFlags.DEFAULT_FLAGS,
        )
        self.window: MainWindow | None = None

    def do_startup(self) -> None:
        Adw.Application.do_startup(self)
        self._load_css()
        self._setup_actions()

    def _load_css(self) -> None:
        css_file = Path(__file__).parent / "ui" / "style.css"
        if css_file.exists():
            provider = Gtk.CssProvider()
            provider.load_from_path(str(css_file))
            display = Gdk.Display.get_default()
            if display:
                Gtk.StyleContext.add_provider_for_display(
                    display,
                    provider,
                    Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
                )

    def _setup_actions(self) -> None:
        # Preferences Action
        pref_action = Gio.SimpleAction.new("preferences", None)
        pref_action.connect("activate", self._on_preferences)
        self.add_action(pref_action)

        # About Action
        about_action = Gio.SimpleAction.new("about", None)
        about_action.connect("activate", self._on_about)
        self.add_action(about_action)

        # Quit Action
        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", lambda a, p: self.quit())
        self.add_action(quit_action)

        self.set_accels_for_action("app.quit", ["<primary>q"])
        self.set_accels_for_action("app.preferences", ["<primary>comma"])

    def do_activate(self) -> None:
        if not self.window:
            self.window = MainWindow(self)
        self.window.present()

    def do_open(self, files: List[Gio.File], hint: str) -> None:
        self.do_activate()
        if self.window:
            paths = [Path(f.get_path()) for f in files if f.get_path()]
            if paths:
                self.window.import_files(paths)

    def _on_preferences(self, action, param) -> None:
        if self.window:
            dialog = SettingsDialog(self.window)
            dialog.present()

    def _on_about(self, action, param) -> None:
        if self.window:
            show_about_dialog(self.window)


def main() -> int:
    """Application main runner."""
    app = StemApplication()
    return app.run(sys.argv)


if __name__ == "__main__":
    sys.exit(main())
