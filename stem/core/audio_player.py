"""
steM. - Multi-Track Audio Playback Engine
GStreamer-based synchronized audio engine for master track and separated stems.
Supports independent volume, mute, solo, seeking, and position tracking.
"""

from pathlib import Path
from typing import Callable, Dict, Optional

import gi
gi.require_version("Gst", "1.0")
from gi.repository import GLib, Gst

Gst.init(None)


class TrackState:
    """Represents the playback state and audio pipeline of an individual stem."""

    def __init__(self, track_id: str, path: Path) -> None:
        self.track_id = track_id
        self.path = path
        self.volume: float = 1.0  # 0.0 to 1.5 (100% is 1.0)
        self.is_muted: bool = False
        self.is_solo: bool = False
        self.player: Optional[Gst.Element] = None
        self._init_pipeline()

    def _init_pipeline(self) -> None:
        self.player = Gst.ElementFactory.make("playbin", f"player_{self.track_id}")
        if self.player:
            self.player.set_property("uri", self.path.resolve().as_uri())
            self.player.set_property("volume", self.volume)

    def set_effective_volume(self, any_solo_active: bool) -> None:
        if not self.player:
            return

        if self.is_muted:
            target_vol = 0.0
        elif any_solo_active:
            target_vol = self.volume if self.is_solo else 0.0
        else:
            target_vol = self.volume

        self.player.set_property("volume", max(0.0, min(1.5, target_vol)))

    def cleanup(self) -> None:
        if self.player:
            self.player.set_state(Gst.State.NULL)
            self.player = None


class MultiTrackPlayer:
    """Synchronized multi-track stems audio player engine."""

    def __init__(self) -> None:
        self._tracks: Dict[str, TrackState] = {}
        self._is_playing: bool = False
        self._duration_seconds: float = 0.0
        self._loop: bool = False
        self._bus_watch_ids: list = []

        # Callbacks
        self.on_position_changed: Optional[Callable[[float, float], None]] = None
        self.on_playback_state_changed: Optional[Callable[[bool], None]] = None
        self.on_eos: Optional[Callable[[], None]] = None

        # Position poll timer
        self._timer_id: Optional[int] = None

    @property
    def is_playing(self) -> bool:
        return self._is_playing

    @property
    def duration(self) -> float:
        return self._duration_seconds

    @property
    def loop(self) -> bool:
        return self._loop

    @loop.setter
    def loop(self, value: bool) -> None:
        self._loop = value

    def load_tracks(self, tracks: Dict[str, Path | str]) -> None:
        """Loads a dictionary of {track_id: file_path}."""
        self.stop()
        self.cleanup()

        for track_id, path in tracks.items():
            p = Path(path).resolve()
            if p.exists():
                t_state = TrackState(track_id, p)
                self._tracks[track_id] = t_state
                self._setup_bus(t_state)

        self._update_all_effective_volumes()
        self._probe_duration()

    def _setup_bus(self, track_state: TrackState) -> None:
        if not track_state.player:
            return
        bus = track_state.player.get_bus()
        bus.add_signal_watch()
        bus.connect("message::eos", self._handle_bus_eos)
        bus.connect("message::error", self._handle_bus_error)

    def _handle_bus_eos(self, bus, msg) -> None:
        if self._loop:
            self.seek(0.0)
            self.play()
        else:
            self.pause()
            self.seek(0.0)
            if self.on_eos:
                self.on_eos()

    def _handle_bus_error(self, bus, msg) -> None:
        err, debug = msg.parse_error()
        print(f"[steM. Audio Player Error]: {err.message} ({debug})")

    def _probe_duration(self) -> None:
        """Queries duration from the first valid track once available."""
        self._duration_seconds = 0.0
        for track in self._tracks.values():
            if track.player:
                track.player.set_state(Gst.State.PAUSED)
                # Wait briefly for preroll to complete
                track.player.get_state(250 * Gst.MSECOND)
                success, dur = track.player.query_duration(Gst.Format.TIME)
                if success and dur > 0:
                    self._duration_seconds = dur / Gst.SECOND
                    break

        if self._duration_seconds <= 0.0 and self._tracks:
            # Fallback to soundfile if GStreamer preroll took longer
            first_track = next(iter(self._tracks.values()))
            try:
                import soundfile as sf
                info = sf.info(str(first_track.path))
                self._duration_seconds = float(info.duration)
            except Exception:
                pass

    def play(self) -> None:
        if not self._tracks:
            return
        for track in self._tracks.values():
            if track.player:
                track.player.set_state(Gst.State.PLAYING)
        self._is_playing = True
        self._start_timer()
        if self.on_playback_state_changed:
            self.on_playback_state_changed(True)

    def pause(self) -> None:
        for track in self._tracks.values():
            if track.player:
                track.player.set_state(Gst.State.PAUSED)
        self._is_playing = False
        self._stop_timer()
        if self.on_playback_state_changed:
            self.on_playback_state_changed(False)

    def stop(self) -> None:
        self.pause()
        self.seek(0.0)

    def toggle_playback(self) -> None:
        if self._is_playing:
            self.pause()
        else:
            self.play()

    def seek(self, position_seconds: float) -> None:
        """Seeks all loaded tracks to the given position in seconds."""
        pos_ns = int(max(0.0, position_seconds) * Gst.SECOND)
        for track in self._tracks.values():
            if track.player:
                track.player.seek_simple(
                    Gst.Format.TIME,
                    Gst.SeekFlags.FLUSH | Gst.SeekFlags.KEY_UNIT,
                    pos_ns,
                )
        if self.on_position_changed:
            self.on_position_changed(position_seconds, self._duration_seconds)

    def get_position(self) -> float:
        """Returns the current playback position in seconds."""
        for track in self._tracks.values():
            if track.player:
                success, pos = track.player.query_position(Gst.Format.TIME)
                if success and pos >= 0:
                    return pos / Gst.SECOND
        return 0.0

    def set_track_volume(self, track_id: str, volume: float) -> None:
        """Sets track volume (0.0 to 1.5)."""
        if track_id in self._tracks:
            self._tracks[track_id].volume = volume
            self._update_all_effective_volumes()

    def set_track_mute(self, track_id: str, is_muted: bool) -> None:
        """Toggles track mute state."""
        if track_id in self._tracks:
            self._tracks[track_id].is_muted = is_muted
            self._update_all_effective_volumes()

    def set_track_solo(self, track_id: str, is_solo: bool) -> None:
        """Toggles track solo state."""
        if track_id in self._tracks:
            self._tracks[track_id].is_solo = is_solo
            self._update_all_effective_volumes()

    def _update_all_effective_volumes(self) -> None:
        any_solo = any(t.is_solo for t in self._tracks.values())
        for track in self._tracks.values():
            track.set_effective_volume(any_solo)

    def _start_timer(self) -> None:
        if self._timer_id is None:
            self._timer_id = GLib.timeout_add(100, self._on_timer_tick)

    def _stop_timer(self) -> None:
        if self._timer_id is not None:
            GLib.source_remove(self._timer_id)
            self._timer_id = None

    def _on_timer_tick(self) -> bool:
        if not self._is_playing:
            return False
        pos = self.get_position()
        if self._duration_seconds <= 0:
            self._probe_duration()
        if self.on_position_changed:
            self.on_position_changed(pos, self._duration_seconds)
        return True

    def cleanup(self) -> None:
        self._stop_timer()
        for track in self._tracks.values():
            track.cleanup()
        self._tracks.clear()
        self._is_playing = False
        self._duration_seconds = 0.0
