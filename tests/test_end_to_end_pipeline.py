"""
End-to-End Integration Test for steM.
Tests complete audio workflow: metadata extraction, waveform peaks,
multi-track player, and stem export.
"""

import tempfile
import time
import unittest
from pathlib import Path

from stem.core.audio_metadata import inspect_audio_file
from stem.core.audio_player import MultiTrackPlayer
from stem.core.exporter import AudioExporter, ExportOptions
from stem.core.waveform import WaveformExtractor


class TestEndToEndPipeline(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture_path = Path(__file__).parent / "fixtures" / "test_track.wav"
        if not cls.fixture_path.exists():
            from tests.generate_test_audio import generate_test_mix
            generate_test_mix(cls.fixture_path, duration_sec=6.0)

    def test_complete_audio_lifecycle(self):
        # 1. Metadata Inspection
        info = inspect_audio_file(self.fixture_path)
        self.assertTrue(info.is_valid)
        self.assertEqual(info.sample_rate, 44100)
        self.assertEqual(info.channels, 2)
        self.assertAlmostEqual(info.duration_seconds, 6.0, delta=0.2)

        # 2. Waveform Extraction
        peaks = WaveformExtractor.get_peaks(self.fixture_path, target_points=100)
        self.assertEqual(len(peaks), 100)
        self.assertTrue(all(0.0 <= p <= 1.0 for p in peaks))

        # 3. Synchronized Multi-Track Playback
        player = MultiTrackPlayer()
        player.load_tracks({"master": self.fixture_path})
        self.assertAlmostEqual(player.duration, 6.0, delta=0.2)

        player.play()
        self.assertTrue(player.is_playing)
        time.sleep(0.3)
        self.assertGreater(player.get_position(), 0.0)

        # Volume & Mute/Solo
        player.set_track_volume("master", 0.75)
        player.set_track_mute("master", True)
        player.set_track_mute("master", False)
        player.seek(1.5)
        time.sleep(0.1)
        player.pause()
        self.assertFalse(player.is_playing)
        player.cleanup()

        # 4. Export Verification
        with tempfile.TemporaryDirectory() as export_dir:
            opts = ExportOptions(
                destination_dir=Path(export_dir),
                track_title="Integration_Track",
                export_format="wav",
                prevent_overwrite=True,
            )
            stems = {
                "vocals": self.fixture_path,
                "drums": self.fixture_path,
                "bass": self.fixture_path,
                "other": self.fixture_path,
            }
            exported = AudioExporter.export_stems(stems, opts)
            self.assertEqual(len(exported), 4)
            for stem_name, path in exported.items():
                self.assertTrue(path.exists())
                self.assertEqual(path.suffix, ".wav")
                self.assertEqual(path.parent.name, "Integration_Track")


if __name__ == "__main__":
    unittest.main()
