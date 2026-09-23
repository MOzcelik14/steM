"""
Unit tests for Demucs subprocess argument construction and output parsing.
"""

import re
import unittest
from pathlib import Path

from stem.core.demucs_runner import DemucsRunner, SeparationTask


class TestDemucsRunner(unittest.TestCase):
    def test_build_command_default_4stems(self):
        runner = DemucsRunner()
        task = SeparationTask(
            audio_path=Path("/tmp/audio track with spaces.mp3"),
            output_dir=Path("/tmp/output"),
            model_name="htdemucs",
            device="cuda",
            segment_size=6,
            shifts=1,
            overlap=0.25,
            export_format="wav",
        )
        cmd = runner.build_command(task)
        self.assertIn("-m", cmd)
        self.assertIn("demucs.separate", cmd)
        self.assertIn("-n", cmd)
        self.assertIn("htdemucs", cmd)
        self.assertIn("-d", cmd)
        self.assertIn("cuda", cmd)
        self.assertIn("--segment", cmd)
        self.assertIn("6", cmd)
        self.assertIn("-j", cmd)
        self.assertIn("1", cmd)
        self.assertEqual(str(task.audio_path.resolve()), cmd[-1])

    def test_build_command_two_stems_and_mp3(self):
        runner = DemucsRunner()
        task = SeparationTask(
            audio_path=Path("/tmp/song.wav"),
            output_dir=Path("/tmp/output"),
            model_name="htdemucs_ft",
            two_stems="vocals",
            device="cpu",
            segment_size=4,
            shifts=2,
            overlap=0.2,
            export_format="mp3",
            mp3_bitrate=320,
        )
        cmd = runner.build_command(task)
        self.assertIn("--two-stems", cmd)
        self.assertIn("vocals", cmd)
        self.assertIn("--mp3", cmd)
        self.assertIn("--mp3-bitrate", cmd)
        self.assertIn("320", cmd)
        self.assertIn("cpu", cmd)

    def test_progress_regex_parsing(self):
        sample_tqdm = " 45%|████▍     | 45/100 [00:12<00:15,  3.50it/s]"
        pattern = re.compile(r"(\d+)%\|")
        match = pattern.search(sample_tqdm)
        self.assertIsNotNone(match)
        self.assertEqual(int(match.group(1)), 45)

    def test_oom_regex_parsing(self):
        sample_err = "torch.cuda.OutOfMemoryError: CUDA out of memory. Tried to allocate 512.00 MiB"
        pattern = re.compile(r"(CUDA out of memory|OutOfMemoryError)", re.IGNORECASE)
        self.assertIsNotNone(pattern.search(sample_err))


if __name__ == "__main__":
    unittest.main()
