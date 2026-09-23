"""
Unit tests for AudioExporter: filename sanitization, collision avoidance, and folder organization.
"""

import tempfile
import unittest
from pathlib import Path

from stem.core.exporter import AudioExporter, ExportOptions


class TestExporter(unittest.TestCase):
    def test_sanitize_filename(self):
        self.assertEqual(AudioExporter.sanitize_filename("Normal Song"), "Normal Song")
        self.assertEqual(AudioExporter.sanitize_filename("Şarkı_Örneği_Çok_Güzel"), "Şarkı_Örneği_Çok_Güzel")
        self.assertEqual(AudioExporter.sanitize_filename("Track: With? Invalid / Chars*"), "Track With Invalid  Chars")
        self.assertEqual(AudioExporter.sanitize_filename("???"), "Untitled_Track")

    def test_get_target_directory_collision_avoidance(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            base_dir = Path(tmp_dir)
            track_title = "My Best Song"

            # First creation
            target1 = AudioExporter.get_target_directory(base_dir, track_title, prevent_overwrite=True)
            self.assertEqual(target1, base_dir / track_title)
            target1.mkdir()

            # Second creation should avoid overwriting and append (1)
            target2 = AudioExporter.get_target_directory(base_dir, track_title, prevent_overwrite=True)
            self.assertEqual(target2, base_dir / f"{track_title} (1)")
            target2.mkdir()

            # Third creation should append (2)
            target3 = AudioExporter.get_target_directory(base_dir, track_title, prevent_overwrite=True)
            self.assertEqual(target3, base_dir / f"{track_title} (2)")

    def test_export_stems_wav_copy(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            base_dir = Path(tmp_dir)
            src_dir = base_dir / "demucs_out"
            src_dir.mkdir()
            vocals_wav = src_dir / "vocals.wav"
            drums_wav = src_dir / "drums.wav"
            vocals_wav.write_text("dummy audio data")
            drums_wav.write_text("dummy audio data")

            dest_dir = base_dir / "export_out"
            stems = {"vocals": vocals_wav, "drums": drums_wav}
            opts = ExportOptions(
                destination_dir=dest_dir,
                track_title="Test Track",
                export_format="wav",
                prevent_overwrite=True,
            )

            exported = AudioExporter.export_stems(stems, opts)
            self.assertEqual(len(exported), 2)
            self.assertIn("vocals", exported)
            self.assertIn("drums", exported)
            self.assertTrue(exported["vocals"].exists())
            self.assertTrue(exported["drums"].exists())
            self.assertEqual(exported["vocals"].parent.name, "Test Track")


if __name__ == "__main__":
    unittest.main()
