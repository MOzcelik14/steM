<p align="center">
  <img src="assets/logo.svg" alt="steM." width="380" />
</p>

<h1 align="center">steM.</h1>

<p align="center">
  Non-destructive multi-track audio workstation and waveform editor for Linux and Windows.
</p>

<p align="center">
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Version-0.5.0-blue.svg" alt="Version 0.5.0" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0-green.svg" alt="License: GPL-3.0" /></a>
  <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-lightgrey.svg" alt="Platform: Linux | Windows" />
</p>

---

## Overview

**steM.** is a lightweight desktop audio editor and multi-track workstation built for podcasters, sound designers, and musicians. It keeps your original recordings untouched while delivering responsive waveform navigation, real-time effects, and broadcast-ready loudness analysis.

## Key Features

- **Non-Destructive Editing**: Cut, trim, copy, paste, fade, and normalize audio without altering source files on disk.
- **Real-Time Effects**: Per-track and master effect chains including EQ, Compressor, Limiter, Delay, and Reverb with instant live preview and reorderable slots.
- **Audio Cleanup & Restoration**: Noise reduction for removing background hum and hiss, plus click and pop removal for clean recordings.
- **Visual Analysis**: Smooth multi-zoom waveform display, 2D waterfall spectrogram, 1D frequency analyzer, Peak/RMS level meters, and EBU R128 loudness reporting.
- **Multi-Track Recording**: Live input monitoring, count-in timer, punch-in/punch-out recording, per-track channel routing, and latency compensation.
- **Format Support**: Import WAV, FLAC, MP3, OGG Vorbis, and AAC/M4A; export lossless WAV and FLAC.
- **Bilingual Interface**: Full English and Turkish language support, switchable at runtime without restarting.

---

## Download & Installation

You can install and update **steM.** directly through [prisM. Suite](https://github.com/MOzcelik14/prism-suite), or download standalone builds from [GitHub Releases](https://github.com/MOzcelik14/steM/releases).

## System Requirements

- **OS**: Linux (x86_64) or Windows 10/11 (x86_64)
- **Audio**: Standard system audio output/input (ALSA / PulseAudio / PipeWire on Linux; WASAPI on Windows)

## Building from Source

Requires Rust 1.80+ (and `libasound2-dev` + `pkg-config` on Debian/Ubuntu):

```bash
git clone https://github.com/MOzcelik14/steM.git
cd steM
cargo build --release -p stem-app
```

Run tests:
```bash
cargo test --workspace
```

## License

Licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See [LICENSE](LICENSE) for details.
