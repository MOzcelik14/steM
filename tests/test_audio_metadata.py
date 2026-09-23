"""
Unit tests for audio metadata extraction and validation in steM.
"""

import math
import struct
import tempfile
import unittest
import wave
from pathlib import Path

from stem.core.audio_metadata import format_duration, inspect_audio_file


class TestAudioMetadata(unittest.TestCase):
    def test_format_duration(self):
        self.assertEqual(format_duration(0), "00:00")
        self.assertEqual(format_duration(45), "00:45")
        self.assertEqual(format_duration(65), "01:05")
        self.assertEqual(format_duration(3665), "01:01:05")

    def test_inspect_nonexistent_file(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            missing_file = Path(tmp_dir) / "does_not_exist.mp3"
            info = inspect_audio_file(missing_file)
            self.assertFalse(info.is_valid)
            self.assertIn("does not exist", info.error_message.lower())

    def test_inspect_empty_file(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            empty_file = Path(tmp_dir) / "empty.wav"
            empty_file.touch()
            info = inspect_audio_file(empty_file)
            self.assertFalse(info.is_valid)
            self.assertIn("empty", info.error_message.lower())

    def test_inspect_valid_wav_file(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            wav_path = Path(tmp_dir) / "test_sinewave.wav"
            sample_rate = 44100
            duration_sec = 1.5
            num_samples = int(sample_rate * duration_sec)

            with wave.open(str(wav_path), "w") as wav_file:
                wav_file.setnchannels(1)  # Mono
                wav_file.setsampwidth(2)  # 16-bit
                wav_file.setframerate(sample_rate)
                # Generate 440 Hz A tone
                frames = bytearray()
                for i in range(num_samples):
                    val = int(32767.0 * 0.5 * math.sin(2.0 * math.pi * 440.0 * i / sample_rate))
                    frames.extend(struct.pack("<h", val))
                wav_file.writeframes(frames)

            info = inspect_audio_file(wav_path)
            self.assertTrue(info.is_valid)
            self.assertEqual(info.sample_rate, sample_rate)
            self.assertEqual(info.channels, 1)
            self.assertAlmostEqual(info.duration_seconds, duration_sec, delta=0.1)
            self.assertIn(info.format_name.lower(), ("wav", "wav (pcm)"))


if __name__ == "__main__":
    unittest.main()
