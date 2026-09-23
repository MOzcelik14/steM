"""
steM. - Export Management & Audio Conversion
Handles directory organization, naming collisions, and WAV/MP3 stem export.
"""

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional


@dataclass
class ExportOptions:
    destination_dir: Path
    track_title: str
    export_format: str = "wav"  # "wav" or "mp3"
    mp3_bitrate: int = 320      # 192, 256, 320
    prevent_overwrite: bool = True


class AudioExporter:
    """Manages copying and transcoding separated stems to the user destination."""

    @staticmethod
    def sanitize_filename(name: str) -> str:
        """Sanitizes track name for safe directory and file creation."""
        # Strip invalid path characters while preserving Turkish Unicode letters
        invalid_chars = '<>:"/\\|?*'
        clean = "".join(c for c in name if c not in invalid_chars)
        return clean.strip() or "Untitled_Track"

    @classmethod
    def get_target_directory(cls, base_dir: Path, track_name: str, prevent_overwrite: bool = True) -> Path:
        """Generates an organized, collision-safe target folder."""
        safe_name = cls.sanitize_filename(track_name)
        target = base_dir / safe_name
        if not prevent_overwrite or not target.exists():
            return target

        # Collision avoidance: append increment
        counter = 1
        while True:
            candidate = base_dir / f"{safe_name} ({counter})"
            if not candidate.exists():
                return candidate
            counter += 1

    @classmethod
    def export_stems(
        cls,
        stems: Dict[str, Path],
        options: ExportOptions,
        on_progress: Optional[callable] = None,
    ) -> Dict[str, Path]:
        """
        Exports stems to options.destination_dir.
        Returns a mapping of {stem_name: exported_path}.
        """
        target_dir = cls.get_target_directory(
            options.destination_dir,
            options.track_title,
            prevent_overwrite=options.prevent_overwrite,
        )
        target_dir.mkdir(parents=True, exist_ok=True)

        exported_map: Dict[str, Path] = {}
        total = len(stems)

        for idx, (stem_name, src_path) in enumerate(stems.items()):
            if not src_path.exists():
                continue

            dest_filename = f"{stem_name}.{options.export_format.lower()}"
            dest_path = target_dir / dest_filename

            if options.export_format.lower() == "wav" and src_path.suffix.lower() == ".wav":
                shutil.copy2(src_path, dest_path)
            elif options.export_format.lower() == "mp3" and src_path.suffix.lower() == ".mp3":
                shutil.copy2(src_path, dest_path)
            else:
                # Transcode required
                cls._transcode_audio(src_path, dest_path, options.mp3_bitrate)

            exported_map[stem_name] = dest_path

            if on_progress:
                on_progress((idx + 1) / total, f"Exported {stem_name}...")

        return exported_map

    @classmethod
    def _transcode_audio(cls, src: Path, dest: Path, mp3_bitrate: int) -> None:
        """Transcodes audio using ffmpeg or soundfile."""
        ffmpeg_bin = shutil.which("ffmpeg")
        if not ffmpeg_bin:
            # Check local bin
            local_ffmpeg = Path.home() / ".local" / "bin" / "ffmpeg"
            if local_ffmpeg.exists():
                ffmpeg_bin = str(local_ffmpeg)

        if ffmpeg_bin:
            cmd = [
                ffmpeg_bin,
                "-y",
                "-i",
                str(src.resolve()),
            ]
            if dest.suffix.lower() == ".mp3":
                cmd.extend(["-b:a", f"{mp3_bitrate}k"])
            cmd.append(str(dest.resolve()))
            res = subprocess.run(cmd, capture_output=True, text=True)
            if res.returncode == 0 and dest.exists():
                return

        # Fallback via soundfile (WAV to WAV or read/write)
        try:
            import soundfile as sf
            data, sr = sf.read(str(src))
            # Soundfile natively writes WAV, FLAC, OGG
            sf.write(str(dest), data, sr)
        except Exception:
            # Direct copy fallback
            shutil.copy2(src, dest)
