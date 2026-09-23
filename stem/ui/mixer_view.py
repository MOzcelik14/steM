"""
steM. - Mixer Studio View
DAW-style multi-track playback studio with stems mixer, faders, mute/solo, and waveform.
"""

from pathlib import Path
from typing import Callable, Dict, Optional
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gtk

from stem.config import STEM_METADATA, settings
from stem.core.audio_metadata import AudioFileInfo, format_duration
from stem.core.audio_player import MultiTrackPlayer
from stem.core.waveform import WaveformExtractor
from stem.ui.waveform_view import WaveformView


class StemChannelStrip(Gtk.Box):
    """An individual vertical channel strip for a stem in the DAW mixer."""

    def __init__(
        self,
        stem_id: str,
        name: str,
        color_hex: str,
        badge_class: str,
        on_volume_changed: Callable[[str, float], None],
        on_mute_toggled: Callable[[str, bool], None],
        on_solo_toggled: Callable[[str, bool], None],
    ) -> None:
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.stem_id = stem_id
        self.on_volume_changed = on_volume_changed
        self.on_mute_toggled = on_mute_toggled
        self.on_solo_toggled = on_solo_toggled

        self.set_css_classes(["channel-strip"])
        self.set_size_request(110, -1)

        # Stem Header Badge
        self.badge = Gtk.Label(label=name)
        self.badge.set_css_classes([badge_class])
        self.append(self.badge)

        # Mute / Solo Buttons Row
        ms_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        ms_box.set_halign(Gtk.Align.CENTER)

        self.mute_btn = Gtk.ToggleButton(label="M")
        self.mute_btn.set_tooltip_text(f"Mute {name}")
        self.mute_btn.set_css_classes(["btn-mute"])
        self.mute_btn.connect("toggled", self._on_mute_click)
        ms_box.append(self.mute_btn)

        self.solo_btn = Gtk.ToggleButton(label="S")
        self.solo_btn.set_tooltip_text(f"Solo {name}")
        self.solo_btn.set_css_classes(["btn-solo"])
        self.solo_btn.connect("toggled", self._on_solo_click)
        ms_box.append(self.solo_btn)

        self.append(ms_box)

        # Vertical Volume Fader
        self.volume_scale = Gtk.Scale.new_with_range(Gtk.Orientation.VERTICAL, 0.0, 1.5, 0.02)
        self.volume_scale.set_inverted(True)
        self.volume_scale.set_vexpand(True)
        self.volume_scale.set_value(1.0)
        self.volume_scale.set_size_request(-1, 160)
        self.volume_scale.connect("value-changed", self._on_fader_changed)
        self.append(self.volume_scale)

        # Fader Readout Label
        self.level_label = Gtk.Label(label="100%")
        self.level_label.set_css_classes(["dim-label", "caption"])
        self.append(self.level_label)

    def _on_fader_changed(self, scale: Gtk.Scale) -> None:
        val = scale.get_value()
        pct = int(val * 100)
        self.level_label.set_label(f"{pct}%")
        self.on_volume_changed(self.stem_id, val)

    def _on_mute_click(self, btn: Gtk.ToggleButton) -> None:
        active = btn.get_active()
        if active:
            btn.add_css_class("active")
        else:
            btn.remove_css_class("active")
        self.on_mute_toggled(self.stem_id, active)

    def _on_solo_click(self, btn: Gtk.ToggleButton) -> None:
        active = btn.get_active()
        if active:
            btn.add_css_class("active")
        else:
            btn.remove_css_class("active")
        self.on_solo_toggled(self.stem_id, active)


class MixerView(Gtk.Box):
    """Complete DAW Playback & Stems Mixer studio interface."""

    def __init__(self, on_export_requested: Optional[Callable[[], None]] = None) -> None:
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        self.on_export_requested = on_export_requested

        self.set_margin_top(16)
        self.set_margin_bottom(16)
        self.set_margin_start(20)
        self.set_margin_end(20)

        self.player = MultiTrackPlayer()
        self.player.on_position_changed = self._on_playback_position
        self.player.on_playback_state_changed = self._on_playback_state

        self.current_track_info: Optional[AudioFileInfo] = None
        self.stems_map: Dict[str, Path] = {}
        self.channel_strips: Dict[str, StemChannelStrip] = {}

        self._build_ui()

    def _build_ui(self) -> None:
        # Header Info Card
        header_card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        header_card.set_hexpand(True)

        self.track_title_label = Gtk.Label(label="No Track Loaded")
        self.track_title_label.set_css_classes(["title-2"])
        self.track_title_label.set_halign(Gtk.Align.START)
        self.track_title_label.set_hexpand(True)
        header_card.append(self.track_title_label)

        self.track_meta_label = Gtk.Label(label="--:-- • -- Hz • --")
        self.track_meta_label.set_css_classes(["dim-label"])
        header_card.append(self.track_meta_label)

        if self.on_export_requested:
            export_btn = Gtk.Button(label="Export Stems...")
            export_btn.set_css_classes(["suggested-action", "pill"])
            export_btn.connect("clicked", lambda b: self.on_export_requested())
            header_card.append(export_btn)

        self.append(header_card)

        # Waveform Visualizer
        self.waveform_view = WaveformView()
        self.waveform_view.on_seek_requested = self._on_user_seek
        self.append(self.waveform_view)

        # Transport Bar
        transport = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        transport.set_css_classes(["transport-bar"])
        transport.set_halign(Gtk.Align.FILL)

        # Stop button
        stop_btn = Gtk.Button.new_from_icon_name("media-playback-stop-symbolic")
        stop_btn.set_css_classes(["flat", "circular"])
        stop_btn.connect("clicked", lambda b: self.player.stop())
        transport.append(stop_btn)

        # Play / Pause button
        self.play_btn = Gtk.Button.new_from_icon_name("media-playback-start-symbolic")
        self.play_btn.set_css_classes(["accent-button", "circular"])
        self.play_btn.connect("clicked", lambda b: self.player.toggle_playback())
        transport.append(self.play_btn)

        # Loop Toggle
        self.loop_btn = Gtk.ToggleButton()
        self.loop_btn.set_icon_name("media-playlist-repeat-symbolic")
        self.loop_btn.set_tooltip_text("Loop Playback")
        self.loop_btn.set_css_classes(["flat", "circular"])
        self.loop_btn.connect("toggled", lambda b: setattr(self.player, "loop", b.get_active()))
        transport.append(self.loop_btn)

        # Timecode Display
        self.time_label = Gtk.Label(label="00:00 / 00:00")
        self.time_label.set_css_classes(["timecode-display"])
        self.time_label.set_hexpand(True)
        self.time_label.set_halign(Gtk.Align.END)
        transport.append(self.time_label)

        self.append(transport)

        # Mixer Channel Strips Container (Scrolled Horizontally)
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_hexpand(True)
        scrolled.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.NEVER)

        self.strips_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        self.strips_box.set_halign(Gtk.Align.CENTER)
        self.strips_box.set_valign(Gtk.Align.FILL)
        scrolled.set_child(self.strips_box)

        self.append(scrolled)

    def load_stems_session(
        self,
        original_track: Optional[AudioFileInfo],
        stems: Dict[str, Path],
    ) -> None:
        """Loads original audio and stems into the mixer."""
        self.player.stop()
        self.current_track_info = original_track
        self.stems_map = stems

        # Clear existing channel strips
        for strip in self.channel_strips.values():
            self.strips_box.remove(strip)
        self.channel_strips.clear()

        # Update Header
        if original_track:
            self.track_title_label.set_label(original_track.title or original_track.filename)
            self.track_meta_label.set_label(
                f"{original_track.duration_str} • {original_track.sample_rate} Hz • {original_track.channels_str}"
            )
            # Load waveform peaks from original
            peaks = WaveformExtractor.get_peaks(original_track.path, target_points=260)
            self.waveform_view.set_peaks(peaks)
        elif stems:
            # Use first stem
            first_stem = next(iter(stems.values()))
            self.track_title_label.set_label(first_stem.parent.name)
            peaks = WaveformExtractor.get_peaks(first_stem, target_points=260)
            self.waveform_view.set_peaks(peaks)

        # Prepare track mapping for player
        audio_map: Dict[str, Path] = {}
        if original_track and original_track.path.exists():
            audio_map["original"] = original_track.path
            self._add_strip("original", "Master Track", "#e63946", "stem-badge-master")

        # Add stem strips
        for stem_key, path in stems.items():
            audio_map[stem_key] = path
            meta = STEM_METADATA.get(stem_key, {
                "label": stem_key.capitalize(),
                "color": "#9d4edd",
            })
            badge_cls = f"stem-badge-{stem_key}" if stem_key in ("vocals", "drums", "bass", "other") else "stem-badge-master"
            self._add_strip(stem_key, meta["label"], meta["color"], badge_cls)

        self.player.load_tracks(audio_map)
        self.waveform_view.set_position(0.0, self.player.duration)

    def _add_strip(self, stem_id: str, label: str, color_hex: str, badge_cls: str) -> None:
        strip = StemChannelStrip(
            stem_id=stem_id,
            name=label,
            color_hex=color_hex,
            badge_class=badge_cls,
            on_volume_changed=self.player.set_track_volume,
            on_mute_toggled=self.player.set_track_mute,
            on_solo_toggled=self.player.set_track_solo,
        )
        self.channel_strips[stem_id] = strip
        self.strips_box.append(strip)

    def _on_user_seek(self, target_seconds: float) -> None:
        self.player.seek(target_seconds)

    def _on_playback_position(self, current: float, duration: float) -> None:
        self.waveform_view.set_position(current, duration)
        c_str = format_duration(current)
        d_str = format_duration(duration)
        self.time_label.set_label(f"{c_str} / {d_str}")

    def _on_playback_state(self, is_playing: bool) -> None:
        icon_name = "media-playback-pause-symbolic" if is_playing else "media-playback-start-symbolic"
        self.play_btn.set_icon_name(icon_name)

    def cleanup(self) -> None:
        self.player.cleanup()
