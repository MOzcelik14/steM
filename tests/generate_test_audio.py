"""
Generates a realistic multi-component test audio file for end-to-end separation testing.
Contains synthetic bass, drum pulses, and harmonic melodic tones.
"""

import math
import struct
import wave
from pathlib import Path


def generate_test_mix(output_path: Path | str, duration_sec: float = 6.0) -> Path:
    p = Path(output_path).resolve()
    p.parent.mkdir(parents=True, exist_ok=True)

    sample_rate = 44100
    num_samples = int(sample_rate * duration_sec)

    # 120 BPM beat interval
    beat_samples = int(sample_rate * 0.5)

    with wave.open(str(p), "w") as wav:
        wav.setnchannels(2)  # Stereo
        wav.setsampwidth(2)  # 16-bit
        wav.setframerate(sample_rate)

        frames = bytearray()
        for i in range(num_samples):
            t = i / sample_rate

            # 1. Bass Component (80 Hz + 160 Hz)
            bass = 0.35 * math.sin(2.0 * math.pi * 80.0 * t) + 0.15 * math.sin(2.0 * math.pi * 160.0 * t)

            # 2. Drum Component (Kick pulse at each beat)
            beat_pos = i % beat_samples
            decay = max(0.0, 1.0 - (beat_pos / (sample_rate * 0.15)))
            drum = 0.4 * math.sin(2.0 * math.pi * 60.0 * (beat_pos / sample_rate)) * decay

            # 3. Melody / Vocal-range Component (440 Hz - 880 Hz)
            freq = 440.0 if (int(t * 2) % 2 == 0) else 554.37  # A4 / C#5
            melody = 0.25 * math.sin(2.0 * math.pi * freq * t)

            sample_l = int(32767.0 * max(-1.0, min(1.0, (bass + drum + melody) * 0.8)))
            sample_r = int(32767.0 * max(-1.0, min(1.0, (bass + drum + melody) * 0.8)))

            frames.extend(struct.pack("<hh", sample_l, sample_r))

        wav.writeframes(frames)

    return p


if __name__ == "__main__":
    out = Path(__file__).parent / "fixtures" / "test_track.wav"
    generate_test_mix(out)
    print(f"Generated test audio at: {out}")
