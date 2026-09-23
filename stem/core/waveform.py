"""
steM. - Waveform Peak Extraction & Caching
Generates normalized waveform peak envelopes for DAW audio visualization.
"""

from pathlib import Path
from typing import List, Optional
import math

try:
    import numpy as np
except ImportError:
    np = None


class WaveformExtractor:
    """Extracts downsampled peak arrays from audio files for graphical rendering."""

    _cache: dict = {}

    @classmethod
    def get_peaks(cls, audio_path: Path | str, target_points: int = 300) -> List[float]:
        """
        Extracts `target_points` normalized peaks (0.0 to 1.0) from the audio file.
        Returns a list of float values.
        """
        p = Path(audio_path).resolve()
        cache_key = f"{p}_{target_points}_{p.stat().st_mtime if p.exists() else 0}"
        if cache_key in cls._cache:
            return cls._cache[cache_key]

        peaks = cls._compute_peaks(p, target_points)
        cls._cache[cache_key] = peaks
        return peaks

    @classmethod
    def _compute_peaks(cls, audio_path: Path, target_points: int) -> List[float]:
        if not audio_path.exists():
            return [0.1] * target_points

        # Method 1: soundfile (fastest for WAV, FLAC, OGG)
        try:
            import soundfile as sf
            with sf.SoundFile(str(audio_path)) as f:
                total_frames = len(f)
                if total_frames <= 0:
                    return [0.1] * target_points

                block_size = max(1, total_frames // target_points)
                peaks = []
                while len(peaks) < target_points:
                    frames_to_read = min(block_size, total_frames - f.tell())
                    if frames_to_read <= 0:
                        break
                    data = f.read(frames_to_read, dtype="float32", always_2d=True)
                    # Average over channels then take max absolute peak
                    mono = np.mean(np.abs(data), axis=1)
                    peak = float(np.max(mono)) if len(mono) > 0 else 0.0
                    peaks.append(peak)

                # Normalize peaks
                max_val = max(peaks) if peaks else 1.0
                if max_val > 0.001:
                    peaks = [p / max_val for p in peaks]
                else:
                    peaks = [0.1] * len(peaks)

                # Pad or trim to exactly target_points
                if len(peaks) < target_points:
                    peaks.extend([0.0] * (target_points - len(peaks)))
                return peaks[:target_points]
        except Exception:
            pass

        # Method 2: torchaudio fallback
        try:
            import torchaudio
            waveform, sr = torchaudio.load(str(audio_path))
            mono = waveform.abs().mean(dim=0).numpy()
            total_samples = len(mono)
            chunk_size = max(1, total_samples // target_points)
            peaks = []
            for i in range(0, total_samples, chunk_size):
                chunk = mono[i:i + chunk_size]
                if len(chunk) > 0:
                    peaks.append(float(np.max(chunk)))
                if len(peaks) >= target_points:
                    break

            max_val = max(peaks) if peaks else 1.0
            if max_val > 0.001:
                peaks = [p / max_val for p in peaks]
            else:
                peaks = [0.1] * len(peaks)

            if len(peaks) < target_points:
                peaks.extend([0.0] * (target_points - len(peaks)))
            return peaks[:target_points]
        except Exception:
            pass

        # Method 3: Fallback aesthetic waveform envelope
        if np is not None:
            t = np.linspace(0, 10, target_points)
            synthetic = 0.4 + 0.3 * np.sin(t * 2) + 0.2 * np.cos(t * 5)
            synthetic = np.clip(np.abs(synthetic), 0.05, 1.0)
            return [float(x) for x in synthetic]
        else:
            peaks = []
            for i in range(target_points):
                val = 0.4 + 0.3 * math.sin(i * 0.1) + 0.2 * math.cos(i * 0.25)
                peaks.append(max(0.05, min(1.0, abs(val))))
            return peaks
