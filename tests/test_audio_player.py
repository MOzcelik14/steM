"""
Unit tests for the multi-track audio player engine.
"""

import tempfile
import unittest
from pathlib import Path

from stem.core.audio_player import MultiTrackPlayer, TrackState


class TestAudioPlayer(unittest.TestCase):
    def test_player_initial_state(self):
        player = MultiTrackPlayer()
        self.assertFalse(player.is_playing)
        self.assertEqual(player.duration, 0.0)
        self.assertFalse(player.loop)

    def test_mute_and_solo_effective_volume(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            dummy_wav = Path(tmp_dir) / "dummy.wav"
            dummy_wav.touch()

            track1 = TrackState("vocals", dummy_wav)
            track2 = TrackState("drums", dummy_wav)

            track1.volume = 1.0
            track2.volume = 0.8

            # No solo, not muted
            track1.set_effective_volume(any_solo_active=False)
            track2.set_effective_volume(any_solo_active=False)
            self.assertEqual(track1.player.get_property("volume"), 1.0)
            self.assertEqual(round(track2.player.get_property("volume"), 2), 0.8)

            # Mute track 1
            track1.is_muted = True
            track1.set_effective_volume(any_solo_active=False)
            self.assertEqual(track1.player.get_property("volume"), 0.0)

            # Solo track 2 (track 1 muted, track 2 soloed)
            track2.is_solo = True
            track1.set_effective_volume(any_solo_active=True)
            track2.set_effective_volume(any_solo_active=True)
            self.assertEqual(track1.player.get_property("volume"), 0.0)
            self.assertEqual(round(track2.player.get_property("volume"), 2), 0.8)

            track1.cleanup()
            track2.cleanup()


if __name__ == "__main__":
    unittest.main()
