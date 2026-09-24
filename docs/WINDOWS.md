# steM. on Windows — Setup & Build Guide

<p align="center">
  <strong>Separate the sound. Keep the soul.</strong><br>
  <em>Created by <strong>M. Özçelik</strong></em>
</p>

steM. is designed with cross-platform architecture in mind. While originally developed for Linux (GTK4 & Libadwaita), it runs on **Windows 10 and 11 (64-bit)** using native GTK4 runtimes and GStreamer.

---

## 🚀 Option 1: Standalone Release (Recommended for Users)

Pre-built standalone Windows zip archives are automatically compiled by our [GitHub Actions CI/CD pipeline](https://github.com/MOzcelik14/steM/actions).

1. Go to the [Releases page](https://github.com/MOzcelik14/steM/releases).
2. Download `steM-Windows-x64.zip`.
3. Extract the ZIP archive to a folder of your choice (e.g. `C:\steM`).
4. Run `steM.exe`.

> [!NOTE]
> On first launch, Demucs will automatically download AI model weights to `%APPDATA%\steM\cache` or `%USERPROFILE%\.cache\torch`.

---

## 🛠️ Option 2: Running from Source via MSYS2 (Recommended for Developers)

GTK4, Libadwaita, and GStreamer are pre-compiled and maintained on Windows via the **MSYS2 UCRT64** environment.

### 1. Install MSYS2
Download and install MSYS2 from [msys2.org](https://www.msys2.org/).

### 2. Install Dependencies in MSYS2 UCRT64
Open the **MSYS2 UCRT64** shell and run:

```bash
pacman -Syu
pacman -S \
  mingw-w64-ucrt-x86_64-python \
  mingw-w64-ucrt-x86_64-python-pip \
  mingw-w64-ucrt-x86_64-python-pygobject \
  mingw-w64-ucrt-x86_64-gtk4 \
  mingw-w64-ucrt-x86_64-libadwaita \
  mingw-w64-ucrt-x86_64-gstreamer \
  mingw-w64-ucrt-x86_64-gst-plugins-base \
  mingw-w64-ucrt-x86_64-gst-plugins-good \
  mingw-w64-ucrt-x86_64-gst-plugins-bad \
  mingw-w64-ucrt-x86_64-gst-libav \
  mingw-w64-ucrt-x86_64-ffmpeg \
  mingw-w64-ucrt-x86_64-pyinstaller
```

### 3. Clone Repository & Install AI Packages
```bash
git clone https://github.com/MOzcelik14/steM.git
cd steM

# Install PyTorch and Demucs
python -m pip install --upgrade pip
# For GPU (NVIDIA CUDA):
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu124
# For CPU only:
# pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu

pip install demucs soundfile numpy scipy
```

### 4. Run steM.
```bash
python -m stem.app
```

Or execute `scripts/run_windows.bat`.

---

## 📦 Option 3: Building Standalone Executable (.exe)

To compile a standalone `.exe` using PyInstaller:

### In MSYS2 UCRT64:
```bash
python scripts/build_windows.py
```

### In Windows PowerShell:
```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
.\scripts\build_windows.ps1
```

The compiled standalone executable and archive will be located at:
`dist/steM/` and `dist/steM-Windows-x64.zip`.
