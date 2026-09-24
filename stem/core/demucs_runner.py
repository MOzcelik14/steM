"""
steM. - Demucs AI Audio Separation Runner
Manages asynchronous Demucs subprocess execution, progress parsing,
CUDA OOM detection, and cancellation.
"""

import os
import re
import shutil
import subprocess
import sys
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, List, Optional


@dataclass
class SeparationTask:
    audio_path: Path
    output_dir: Path
    model_name: str = "htdemucs"
    two_stems: Optional[str] = None  # None for 4 stems, or "vocals"
    device: str = "cuda"            # "cuda" or "cpu"
    segment_size: int = 6           # seconds (crucial for 4GB VRAM)
    shifts: int = 1
    overlap: float = 0.25
    export_format: str = "wav"      # "wav" or "mp3"
    mp3_bitrate: int = 320


class DemucsRunner:
    """Runs Demucs separation in a background subprocess with live progress reporting."""

    def __init__(self) -> None:
        self._process: Optional[subprocess.Popen] = None
        self._is_cancelled: bool = False
        self._lock = threading.Lock()
        self._thread: Optional[threading.Thread] = None

    @property
    def is_running(self) -> bool:
        with self._lock:
            return self._process is not None and self._process.poll() is None

    def cancel(self) -> None:
        """Gracefully terminates the running Demucs process."""
        with self._lock:
            self._is_cancelled = True
            if self._process is not None:
                try:
                    self._process.terminate()
                except Exception:
                    pass

    def run_separation_async(
        self,
        task: SeparationTask,
        on_progress: Optional[Callable[[float, str], None]] = None,
        on_log: Optional[Callable[[str], None]] = None,
        on_finished: Optional[Callable[[bool, Optional[str], Dict[str, Path]], None]] = None,
        on_oom: Optional[Callable[[str], None]] = None,
    ) -> None:
        """Starts separation in a background thread."""
        self._thread = threading.Thread(
            target=self._execute,
            args=(task, on_progress, on_log, on_finished, on_oom),
            daemon=True,
        )
        self._thread.start()

    def _get_python_executable(self) -> str:
        """Determines the current virtualenv Python binary."""
        if getattr(sys, "frozen", False):
            demucs_bin = shutil.which("demucs")
            if demucs_bin:
                return demucs_bin
            return shutil.which("python") or shutil.which("python3") or "python"

        if os.name == "nt":
            venv_python = Path(sys.prefix) / "Scripts" / "python.exe"
            if venv_python.exists():
                return str(venv_python)
            venv_python_root = Path(sys.prefix) / "python.exe"
            if venv_python_root.exists():
                return str(venv_python_root)
        else:
            venv_python = Path(sys.prefix) / "bin" / "python3"
            if venv_python.exists():
                return str(venv_python)
        return sys.executable

    def build_command(self, task: SeparationTask) -> List[str]:
        """Builds the subprocess argument array for Demucs."""
        py_exe = self._get_python_executable()
        if Path(py_exe).stem.lower().startswith("demucs"):
            cmd = [py_exe]
        else:
            cmd = [
                py_exe,
                "-m",
                "demucs.separate",
            ]
        cmd.extend([
            "-n",
            task.model_name,
            "-d",
            task.device,
            "-o",
            str(task.output_dir),
            "--segment",
            str(task.segment_size),
            "--shifts",
            str(task.shifts),
            "--overlap",
            str(task.overlap),
            "-j",
            "1",  # 1 worker job to prevent GPU VRAM duplication
        ])

        if task.two_stems:
            cmd.extend(["--two-stems", task.two_stems])

        if task.export_format == "mp3":
            cmd.extend(["--mp3", "--mp3-bitrate", str(task.mp3_bitrate)])

        # Audio file path as the final argument (safe for spaces and Turkish characters)
        cmd.append(str(task.audio_path.resolve()))
        return cmd

    def _execute(
        self,
        task: SeparationTask,
        on_progress: Optional[Callable[[float, str], None]],
        on_log: Optional[Callable[[str], None]],
        on_finished: Optional[Callable[[bool, Optional[str], Dict[str, Path]], None]],
        on_oom: Optional[Callable[[str], None]],
    ) -> None:
        self._is_cancelled = False
        task.output_dir.mkdir(parents=True, exist_ok=True)
        cmd = self.build_command(task)

        if on_log:
            on_log(f"Starting separation command: {' '.join(cmd)}")
        if on_progress:
            on_progress(0.02, "Initializing Demucs model...")

        env = os.environ.copy()
        # Add local bin or static ffmpeg to PATH if available
        local_bin = str(Path.home() / ".local" / "bin")
        if local_bin not in env.get("PATH", ""):
            env["PATH"] = f"{local_bin}:{env.get('PATH', '')}"

        # PyTorch memory allocation tuning for 4GB VRAM
        env["PYTORCH_CUDA_ALLOC_CONF"] = "expandable_segments:True"

        try:
            creation_flags = (
                subprocess.CREATE_NO_WINDOW if os.name == "nt" and hasattr(subprocess, "CREATE_NO_WINDOW") else 0
            )

            with self._lock:
                self._process = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    bufsize=1,
                    env=env,
                    creationflags=creation_flags,
                )

            progress_pattern = re.compile(r"(\d+)%\|")
            oom_pattern = re.compile(r"(CUDA out of memory|OutOfMemoryError)", re.IGNORECASE)
            oom_detected = False
            last_progress = 0.05
            output_buffer: List[str] = []

            assert self._process.stdout is not None
            # Read output character-by-character or line-by-line (tqdm uses \r)
            chunk = ""
            while True:
                char = self._process.stdout.read(1)
                if not char:
                    break
                if char in ("\r", "\n"):
                    line = chunk.strip()
                    chunk = ""
                    if not line:
                        continue

                    output_buffer.append(line)
                    if on_log:
                        on_log(line)

                    # Check for OOM
                    if oom_pattern.search(line):
                        oom_detected = True

                    # Check for progress
                    match = progress_pattern.search(line)
                    if match:
                        pct = int(match.group(1)) / 100.0
                        # Scale progress from 0.05 to 0.95
                        scaled = 0.05 + (pct * 0.90)
                        last_progress = max(last_progress, scaled)
                        if on_progress:
                            on_progress(last_progress, f"Separating audio stems ({int(pct * 100)}%)...")
                else:
                    chunk += char

            self._process.wait()
            ret_code = self._process.returncode

            if self._is_cancelled:
                if on_finished:
                    on_finished(False, "Separation cancelled by user.", {})
                return

            if oom_detected or (ret_code != 0 and any("CUDA out of memory" in l for l in output_buffer)):
                oom_msg = (
                    "NVIDIA GPU ran out of memory (VRAM). "
                    "You can retry this song using CPU fallback or a smaller segment size."
                )
                if on_oom:
                    on_oom(oom_msg)
                if on_finished:
                    on_finished(False, oom_msg, {})
                return

            if ret_code != 0:
                err_summary = "\n".join(output_buffer[-10:]) if output_buffer else "Unknown error"
                if on_finished:
                    on_finished(False, f"Demucs exited with code {ret_code}:\n{err_summary}", {})
                return

            # Discover generated stems
            stems = self._locate_stems(task)
            if on_progress:
                on_progress(1.0, "Separation completed successfully!")
            if on_finished:
                on_finished(True, None, stems)

        except Exception as e:
            if on_finished:
                on_finished(False, str(e), {})
        finally:
            with self._lock:
                self._process = None

    def _locate_stems(self, task: SeparationTask) -> Dict[str, Path]:
        """Finds the output stem files in the Demucs output hierarchy."""
        stems_dict: Dict[str, Path] = {}
        # Demucs standard directory structure:
        # {output_dir}/{model_name}/{track_name}/{stem}.{ext}
        track_stem_name = task.audio_path.stem
        target_dir = task.output_dir / task.model_name / track_stem_name

        if not target_dir.exists():
            # Search anywhere in output_dir
            for p in task.output_dir.rglob(f"*{track_stem_name}*"):
                if p.is_dir():
                    target_dir = p
                    break

        if target_dir.exists() and target_dir.is_dir():
            for child in target_dir.iterdir():
                if child.is_file() and child.suffix.lower() in (".wav", ".mp3", ".flac"):
                    stems_dict[child.stem.lower()] = child

        return stems_dict
