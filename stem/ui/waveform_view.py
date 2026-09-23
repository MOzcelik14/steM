"""
steM. - Waveform Visualizer Widget
Interactive Cairo-rendered waveform drawing area with click-to-seek playhead.
"""

from typing import Callable, List, Optional
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, Gtk


class WaveformView(Gtk.DrawingArea):
    """Visualizes audio waveform peaks with an interactive playhead."""

    def __init__(self) -> None:
        super().__init__()
        self.set_css_classes(["waveform-view"])
        self.set_hexpand(True)
        self.set_vexpand(False)
        self.set_content_height(96)
        self.set_content_width(300)

        self._peaks: List[float] = []
        self._current_pos: float = 0.0
        self._duration: float = 0.0
        self._is_seeking: bool = False

        self.on_seek_requested: Optional[Callable[[float], None]] = None

        self.set_draw_func(self._on_draw)

        # Gestures for seeking
        click_gesture = Gtk.GestureClick.new()
        click_gesture.connect("pressed", self._on_click_pressed)
        self.add_controller(click_gesture)

        drag_gesture = Gtk.GestureDrag.new()
        drag_gesture.connect("drag-begin", self._on_drag_begin)
        drag_gesture.connect("drag-update", self._on_drag_update)
        drag_gesture.connect("drag-end", self._on_drag_end)
        self.add_controller(drag_gesture)

    def set_peaks(self, peaks: List[float]) -> None:
        self._peaks = peaks
        self.queue_draw()

    def set_position(self, current_seconds: float, duration_seconds: float) -> None:
        self._current_pos = max(0.0, current_seconds)
        self._duration = max(0.0, duration_seconds)
        self.queue_draw()

    def _seek_from_x(self, x: float) -> None:
        width = self.get_width()
        if width > 0 and self._duration > 0 and self.on_seek_requested:
            ratio = max(0.0, min(1.0, x / width))
            target_sec = ratio * self._duration
            self._current_pos = target_sec
            self.queue_draw()
            self.on_seek_requested(target_sec)

    def _on_click_pressed(self, gesture, n_press, x, y) -> None:
        self._seek_from_x(x)

    def _on_drag_begin(self, gesture, start_x, start_y) -> None:
        self._is_seeking = True
        self._seek_from_x(start_x)

    def _on_drag_update(self, gesture, offset_x, offset_y) -> None:
        start_x, _ = gesture.get_start_point()
        self._seek_from_x(start_x + offset_x)

    def _on_drag_end(self, gesture, offset_x, offset_y) -> None:
        self._is_seeking = False

    def _on_draw(self, area, cr, width, height) -> None:
        # Background
        cr.set_source_rgb(0.06, 0.06, 0.08)
        cr.paint()

        if not self._peaks:
            # Draw empty center line
            cr.set_source_rgba(0.4, 0.3, 0.5, 0.3)
            cr.set_line_width(1.5)
            cr.move_to(0, height / 2)
            cr.line_to(width, height / 2)
            cr.stroke()
            return

        mid_y = height / 2.0
        num_peaks = len(self._peaks)
        bar_step = width / max(1, num_peaks)
        bar_width = max(1.0, bar_step * 0.75)

        # Progress ratio
        progress_ratio = 0.0
        if self._duration > 0:
            progress_ratio = max(0.0, min(1.0, self._current_pos / self._duration))
        playhead_x = progress_ratio * width

        # Draw waveform bars
        for i, peak in enumerate(self._peaks):
            x = i * bar_step
            # Symmetrical height
            bar_h = max(2.0, peak * (height * 0.42))
            top_y = mid_y - bar_h
            bot_y = mid_y + bar_h

            # Color before playhead is brighter purple, after is dimmer violet
            if x <= playhead_x:
                cr.set_source_rgba(0.61, 0.30, 0.86, 0.95)  # #9d4edd
            else:
                cr.set_source_rgba(0.35, 0.20, 0.55, 0.45)

            cr.rectangle(x, top_y, bar_width, bot_y - top_y)
            cr.fill()

        # Draw Playhead Needle
        cr.set_source_rgb(0.0, 0.96, 0.83)  # Neon cyan #00f5d4
        cr.set_line_width(2.0)
        cr.move_to(playhead_x, 0)
        cr.line_to(playhead_x, height)
        cr.stroke()

        # Playhead handle cap
        cr.arc(playhead_x, 6, 4, 0, 2 * 3.14159)
        cr.fill()
