"""
GUI startup and widget initialization test for steM.
"""

import sys
import unittest
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from stem.app import StemApplication
from stem.ui.main_window import MainWindow
from stem.ui.welcome_view import WelcomeView
from stem.ui.waveform_view import WaveformView


class TestGuiStartup(unittest.TestCase):
    def setUp(self):
        Adw.init()

    def test_widget_instantiation(self):
        welcome = WelcomeView(on_files_selected=lambda files: None)
        self.assertIsNotNone(welcome)

        waveform = WaveformView()
        waveform.set_peaks([0.2, 0.5, 0.8, 0.3])
        waveform.set_position(10.0, 30.0)
        self.assertIsNotNone(waveform)

    def test_application_window_creation(self):
        app = StemApplication()
        window = MainWindow(app)
        self.assertIsNotNone(window)
        self.assertIn("steM.", window.get_title())
        window.destroy()


if __name__ == "__main__":
    unittest.main()
