#!/usr/bin/env bash
# steM. — Installation and Environment Setup Script
# Created by M. Özçelik
set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$PROJECT_DIR/.venv"

echo "========================================================"
echo "  steM. — AI Audio Separation Studio Installer"
echo "  Created by M. Özçelik"
echo "========================================================"

echo "[1/4] Setting up Python virtual environment..."
if [ ! -d "$VENV_DIR" ]; then
    python3 -m venv --system-site-packages "$VENV_DIR"
    echo "  Virtual environment created at: $VENV_DIR"
else
    echo "  Virtual environment already exists at: $VENV_DIR"
fi

echo "[2/4] Installing Python dependencies..."
"$VENV_DIR/bin/pip" install --upgrade pip

# Check NVIDIA GPU
if command -v nvidia-smi &> /dev/null; then
    echo "  NVIDIA GPU detected. Installing PyTorch with CUDA 12.4 acceleration..."
    "$VENV_DIR/bin/pip" install torch torchaudio --index-url https://download.pytorch.org/whl/cu124
else
    echo "  No NVIDIA GPU detected. Installing PyTorch CPU..."
    "$VENV_DIR/bin/pip" install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
fi

echo "  Installing Demucs, soundfile, static-ffmpeg, and audio libraries..."
"$VENV_DIR/bin/pip" install demucs soundfile static-ffmpeg numpy scipy

echo "[3/4] Installing Desktop Launcher and Icons..."
mkdir -p ~/.local/share/applications
mkdir -p ~/.local/share/icons/hicolor/scalable/apps
mkdir -p ~/.local/bin

# Copy icons
cp -r "$PROJECT_DIR/data/icons/hicolor/"* ~/.local/share/icons/hicolor/

# Create wrapper script in ~/.local/bin/stem
cat << EOF > ~/.local/bin/stem
#!/usr/bin/env bash
exec "$PROJECT_DIR/run.sh" "\$@"
EOF
chmod +x ~/.local/bin/stem

# Install desktop file
cat << EOF > ~/.local/share/applications/com.mozcelik.stem.desktop
[Desktop Entry]
Name=steM.
GenericName=AI Audio Separation Studio
GenericName[tr]=Yapay Zekâ Destekli Ses Ayrıştırma Stüdyosu
Comment=AI Audio Separation Studio
Comment[tr]=Yapay Zekâ Destekli Ses Ayrıştırma Stüdyosu
Exec=$PROJECT_DIR/run.sh %F
Icon=com.mozcelik.stem
Terminal=false
Type=Application
Categories=AudioVideo;Audio;AudioVideoEditing;Music;
MimeType=audio/mpeg;audio/x-wav;audio/x-flac;audio/ogg;audio/mp4;audio/aac;
Keywords=audio;stem;separation;demucs;vocals;drums;bass;music;ai;studio;
StartupNotify=true
StartupWMClass=com.mozcelik.stem
EOF

update-desktop-database ~/.local/share/applications 2>/dev/null || true
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true

echo "[4/4] Installation Complete!"
echo "  You can launch steM. by typing: stem"
echo "  Or by running: ./run.sh"
echo "  Or via your Linux Mint application menu under Sound & Video!"
echo "========================================================"
