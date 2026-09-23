# Changelog

All notable changes to **steM.** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-09-23

### Added
- **Core AI Separation**:
  - Demucs v4 (Hybrid Transformer) integration with model support for `htdemucs`, `htdemucs_ft`, `htdemucs_6s`, and `mdx_extra`.
  - 4-stem separation (vocals, drums, bass, and other).
  - 2-stem vocal/instrumental extraction mode.
  - Configurable shifts (0–2), overlap (0.1–0.5), and segment size parameters.
  - Safe subprocess execution with real-time progress parsing and graceful cancellation.
- **Hardware Optimization**:
  - Automatic detection of NVIDIA RTX 3050 Laptop GPU (4 GB VRAM) and driver environment.
  - VRAM safeguards: low-VRAM chunking (segment=6s, jobs=1) to prevent out-of-memory errors.
  - Seamless CUDA OOM detection with automatic prompt to retry on CPU.
  - CPU multithreaded fallback when GPU is unavailable or busy.
- **Playback Studio & Mixer**:
  - Low-latency multi-track synchronized audio player powered by GStreamer 1.0.
  - Independent channel strips for Master, Vocals, Drums, Bass, Other, and Instrumental.
  - Vertical volume faders (0% to 150%) with decibel/percentage indicators.
  - Individual Mute and Solo buttons with DAW-standard logic.
  - Interactive Cairo-rendered waveform visualizer with click/drag playhead scrubbing.
  - Timecode display and loop toggle.
- **Audio Importing & Exporting**:
  - Drag-and-drop file import supporting MP3, WAV, FLAC, OGG, M4A, and AAC.
  - Pre-processing file validation and metadata extraction (sample rate, channels, duration).
  - Processing queue for batch audio separation.
  - Export to 24-bit lossless WAV and high-bitrate MP3 (320 kbps).
  - Automatic directory organization (`{Destination}/{Track Name}/{stem}.{ext}`).
  - Naming collision avoidance preventing accidental file overwrites.
- **User Interface**:
  - Native Linux desktop application built with GTK4 and Libadwaita.
  - Sleek dark audio production workstation theme with signature purple accent (`#9d4edd`).
  - ViewStack navigation (Welcome, Separation Queue, Mixer Studio).
  - In-app Preferences dialog and About dialog.
  - Custom original SVG and hicolor PNG application icons.
  - Desktop launcher and AppStream metainfo specification.
