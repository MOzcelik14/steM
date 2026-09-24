# steM.

<p align="center">
  <img src="data/icons/hicolor/scalable/apps/com.mozcelik.stem.svg" width="128" height="128" alt="steM. Icon"/>
</p>

<p align="center">
  <strong>AI Audio Separation Studio</strong><br>
  <em>Created by <strong>M. Özçelik</strong></em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-purple.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-Linux%20Mint%2022.3%20%7C%20Ubuntu%2024.04-informational.svg" alt="Platform">
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20Ready-0078d7.svg?logo=windows" alt="Windows Ready">
  <img src="https://img.shields.io/badge/GTK-4.0%20%2B%20Libadwaita-blueviolet.svg" alt="GTK4">
  <img src="https://img.shields.io/badge/AI%20Engine-Demucs%20v4-ff007f.svg" alt="Demucs">
  <img src="https://img.shields.io/badge/GPU%20Ready-NVIDIA%20CUDA-76b900.svg" alt="CUDA">
</p>

<p align="center">
  <strong>English</strong> • <a href="README.tr.md">🇹🇷 <strong>Türkçe Dokümantasyon</strong></a> • <a href="docs/WINDOWS.md">🪟 <strong>Windows Guide</strong></a>
</p>

> [!NOTE]
> 🇹🇷 **Türkçe Dokümantasyon**: Bu projenin Türkçe açıklamaları, kurulum adımları ve kullanım kılavuzu için lütfen [**README.tr.md**](README.tr.md) belgesini inceleyin. Windows kullanıcıları için ayrıca [**docs/WINDOWS.tr.md**](docs/WINDOWS.tr.md) rehberi mevcuttur.

---

## Overview

**steM.** is a native, modern Linux desktop audio workstation application for musicians, producers, DJs, and sound engineers. Powered by state-of-the-art **Demucs** deep learning models, steM. separates mixed music files into individual high-fidelity stems with an intuitive, hardware-optimized graphical studio.

Unlike generic wrappers, steM. features a full DAW-inspired multi-track playback studio, synchronized phase-locked stem listening, independent channel faders, solo/mute matrix, interactive Cairo waveform scrubbing, and fine-tuned memory safeguards for modern GPUs such as the 4 GB NVIDIA RTX 3050 Laptop GPU.

---

## Key Features

### 🎧 Audio Importing & Batch Queue
- **Supported Formats**: MP3, WAV, FLAC, OGG, M4A, AAC, and more.
- **Drag-and-Drop**: Drop single audio files or multiple songs directly from Nemo or Nautilus.
- **Pre-Validation**: File verification, sample rate, bit depth, channel format, and duration parsing.
- **Batch Processing Queue**: Queue multiple songs and process them sequentially without freezing the UI.

### 🧠 AI Stem Separation (Demucs v4)
- **4-Stem Separation**: Vocals, Drums, Bass, and Other.
- **2-Stem Extraction**: Quick vocal isolation and backing track (instrumental) creation.
- **Supported Architectures**:
  - `htdemucs` (Default Hybrid Transformer — balanced speed & studio quality)
  - `htdemucs_ft` (Fine-tuned model for superior vocal isolation)
  - `htdemucs_6s` (6 stems including piano and guitar isolation)
  - `mdx_extra` (MDX-Net architecture trained on extra datasets)
- **Quality Precision Tuning**: Configurable shifts (0–2), overlap, and segment sizing.
- **Real-Time Progress**: Subprocess tracking with live percentage progress reporting.
- **Instant Cancellation**: Gracefully terminate ongoing operations without corrupting data.

### ⚡ Hardware Optimization (RTX 3050 / 4 GB VRAM)
- **Automatic CUDA Detection**: Detects NVIDIA GPUs, driver versions, and available VRAM.
- **Low-VRAM Safety Mode**: Segments audio into 6-second chunks with single-worker execution (`-j 1`) to eliminate memory thrashing.
- **CUDA OOM Recovery**: If GPU memory is exhausted, steM. catches the error and offers an immediate, one-click fallback to multithreaded CPU processing.
- **CPU Fallback**: Full multithreaded CPU separation when no compatible GPU is present.

### 🎚️ Multi-Track Playback Studio & Mixer
- **Synchronized Playback**: Native GStreamer 1.0 engine playing all stems in perfect phase synchronization.
- **Channel Strips**: Dedicated strips for Master Track, Vocals, Drums, Bass, Other, and Instrumental.
- **Faders & Levels**: Vertical volume faders (0% to 150%) with decibel/percentage indicators.
- **DAW Solo & Mute Matrix**:
  - Individual **Mute (M)** buttons.
  - Exclusive **Solo (S)** mode: soloing one or more stems automatically silences all non-soloed tracks.
- **Interactive Waveform Visualizer**:
  - Smooth Cairo-rendered peak envelope visualization.
  - Interactive playhead scrubber: click or drag along the waveform to seek instantly across all stems.
  - Monospace digital timecode (`00:00 / 03:45`) and loop toggle.

### 💾 Export Options
- **Formats**: Lossless 24-bit PCM WAV and High-Quality 320 kbps MP3.
- **Destination Folder**: Configurable output folder (defaults to `~/Müzik/steM_Stems`).
- **Clean Structure**: Automatically saves to `{Destination}/{Track Name}/{stem}.{ext}`.
- **Collision Protection**: Prevents accidental overwriting of previously exported sessions.

---

## System Requirements

- **Operating System**: Linux (Tested and optimized on Linux Mint 22.3 Cinnamon / Ubuntu 24.04 LTS).
- **Python**: 3.12 or higher.
- **Desktop Libraries**: GTK 4.0, Libadwaita 1.x, GStreamer 1.0 (with standard plugins).
- **GPU (Recommended)**: NVIDIA GPU with CUDA support (e.g. GeForce RTX 3050 Laptop GPU, 4 GB VRAM).
- **RAM**: Minimum 8 GB (16–24 GB recommended).

---

## Installation

### Automatic Quick Setup (Recommended)

Clone the repository and run the setup script:

```bash
git clone https://github.com/MOzcelik14/steM.git
cd steM
./install.sh
```

The installer will:
1. Create a Python virtual environment reusing system GTK4 / Libadwaita bindings.
2. Install PyTorch with CUDA 12.4 acceleration and Demucs v4.
3. Install the application icon and `.desktop` launcher into your application menu under **Sound & Video**.
4. Create the `stem` CLI shortcut in `~/.local/bin/stem`.

---

### Manual Setup

If you prefer to configure the environment step-by-step:

```bash
# 1. Ensure system packages are present (Linux Mint / Ubuntu)
sudo apt update
sudo apt install -y python3-gi python3-gi-cairo gir1.2-gtk-4.0 gir1.2-adw-1 \
    gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav libsndfile1

# 2. Create virtual environment with system site packages
python3 -m venv --system-site-packages .venv
source .venv/bin/activate

# 3. Install PyTorch with CUDA 12.4
pip install --upgrade pip
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu124

# 4. Install Demucs and audio dependencies
pip install demucs soundfile static-ffmpeg numpy scipy

# 5. Launch steM.
./run.sh
```

---

### 🪟 Windows 10 / 11 Setup

steM. offers full cross-platform compatibility on Windows:

1. **Pre-compiled Releases**: Download `steM-Windows-x64.zip` directly from [GitHub Releases](https://github.com/MOzcelik14/steM/releases) (built via our automated GitHub Actions workflow).
2. **Developer Environment**: Run natively from source using the MSYS2 UCRT64 environment.
3. Detailed installation, build scripts, and dependencies are documented in the [**Windows Setup Guide (docs/WINDOWS.md)**](docs/WINDOWS.md).

---

## Launching the Application

- **From the Application Menu**: Open your Linux Mint / Cinnamon menu, navigate to **Sound & Video**, and click **steM.**
- **From Any Terminal**:
  ```bash
  stem
  ```
- **From the Repository Folder**:
  ```bash
  ./run.sh
  ```

---

## Project Structure

```
steM/
├── stem/
│   ├── __init__.py
│   ├── app.py                  # Adw.Application lifecycle and actions
│   ├── config.py               # Constants, model definitions, and settings
│   ├── core/
│   │   ├── __init__.py
│   │   ├── hardware.py         # NVIDIA GPU/CUDA detection & 4GB VRAM tuning
│   │   ├── demucs_runner.py    # Demucs subprocess execution & progress parsing
│   │   ├── audio_metadata.py   # Audio file inspector & format validator
│   │   ├── audio_player.py     # GStreamer multi-track synchronized stems player
│   │   ├── waveform.py         # Waveform peak extraction for visualizer
│   │   ├── exporter.py         # WAV & MP3 export and folder structure manager
│   │   └── queue_manager.py    # Asynchronous batch processing queue
│   └── ui/
│       ├── __init__.py
│       ├── main_window.py      # Adw.ApplicationWindow & ViewStack controller
│       ├── welcome_view.py     # Drag-and-drop landing view & branding banner
│       ├── separation_view.py  # Queue list, parameter selection, live progress
│       ├── mixer_view.py       # DAW-style channel strips, faders, mute/solo
│       ├── waveform_view.py    # Cairo-rendered interactive waveform scrubber
│       ├── settings_dialog.py  # Preferences dialog (models, formats, paths)
│       ├── about_dialog.py     # About dialog with system specs & attribution
│       └── style.css           # Studio dark theme stylesheet (#9d4edd)
├── data/
│   ├── icons/
│   │   └── hicolor/scalable/apps/com.mozcelik.stem.svg
│   ├── com.mozcelik.stem.desktop
│   └── com.mozcelik.stem.metainfo.xml
├── tests/
│   ├── test_hardware.py
│   ├── test_audio_metadata.py
│   ├── test_demucs_runner.py
│   ├── test_audio_player.py
│   ├── test_exporter.py
│   └── test_gui_startup.py
├── pyproject.toml
├── requirements.txt
├── install.sh                  # One-step automated installer
├── run.sh                      # Executable launcher
├── AGENTS.md                   # Development conventions
├── README.md                   # English documentation
├── README.tr.md                # Turkish documentation
├── LICENSE                     # MIT License
└── CHANGELOG.md                # Version history
```

---

## Running the Automated Tests

Run the full test suite using Python's standard unittest discovery:

```bash
python3 -m unittest discover -s tests -p "test_*.py" -v
```

---

## License

This project is open-source software licensed under the **MIT License**.  
See the [LICENSE](LICENSE) file for details.

Copyright © 2026 **M. Özçelik**. All rights reserved.
