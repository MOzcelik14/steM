# AGENTS.md — Development Guidelines for steM.

## Project Identity
- **Application Name**: steM. (Capital M, trailing dot)
- **Tagline**: Separate the sound. Keep the soul.
- **Creator / Developer**: M. Özçelik
- **Application ID**: `com.mozcelik.stem`
- **Initial Version**: 1.0.0
- **Repository**: `MOzcelik14/steM`
- **Platform**: Linux (Linux Mint 22.3 Cinnamon target)

## Technology Stack & Guidelines
- **Language**: Python 3.12+
- **GUI Framework**: GTK4 (`gi.repository.Gtk`) & Libadwaita (`gi.repository.Adw`) via PyGObject.
- **Audio Engine**: GStreamer 1.0 (`gi.repository.Gst`) for low-latency, multi-track, phase-locked stem playback.
- **AI Separation Engine**: Meta's Demucs (Hybrid Transformer Demucs: `htdemucs`, `htdemucs_ft`, `htdemucs_6s`, `mdx_extra`).
- **Deep Learning / Hardware**: PyTorch with NVIDIA CUDA acceleration (targeting RTX 3050 Laptop GPU with 4GB VRAM).
- **Audio I/O**: `soundfile` (`libsndfile1`), `ffmpeg` / `static-ffmpeg`.

## Architecture Principles
1. **Separation of Concerns**: Presentation (GTK4/Libadwaita UI) must remain completely decoupled from audio processing, model inference, and playback pipelines.
2. **Non-blocking UI**: Never execute CPU/GPU heavy operations, audio decoding, or subprocesses on the GTK main thread. All background work must run asynchronously with `GLib.idle_add()` or `GLib.timeout_add()` for GUI updates.
3. **Subprocess Safety**: Always pass arguments as arrays (`["demucs", "-n", model, ...]`), never run shell strings (`shell=True`).
4. **Path & Encoding Safety**: Support Unicode, Turkish characters (`ç, ğ, ı, ö, ş, ü`), spaces, and special symbols in file paths.
5. **VRAM Safety & Fallback**: RTX 3050 has 4GB VRAM. Use Demucs segment chunking (`--segment`), avoid multi-worker GPU contention (`--jobs 1`), and gracefully catch CUDA Out Of Memory (OOM) to offer seamless CPU fallback.
6. **Clean Exports**: Generate clean, organized output folder structures (`{output_dir}/{Track Name}/{stem}.{ext}`) and prevent accidental overwrites.

## Environment & Dependency Rules
- Linux Mint 22.3 enforces PEP 668. Always use a virtual environment created with `--system-site-packages` so system-wide PyGObject/GTK4/Libadwaita/GStreamer packages are reused without compilation.
- Do not run destructive commands or use `sudo` without explicit user permission.
- Ensure automated and manual testing before releases.
