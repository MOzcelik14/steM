#!/usr/bin/env bash
# ==============================================================================
# steM. — Debian/Ubuntu Package (.deb) Builder
# Created by M. Özçelik
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
PKG_VERSION="1.0.0"
PKG_ARCH="amd64"
PKG_NAME="stem"
DIST_DIR="$ROOT_DIR/dist"
BUILD_ROOT="$ROOT_DIR/build/deb"
STAGE_DIR="$BUILD_ROOT/${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}"

echo "=========================================================="
echo " steM. — Building Debian Package (${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}.deb)"
echo " Creator: M. Özçelik"
echo "=========================================================="

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/DEBIAN"
mkdir -p "$STAGE_DIR/usr/bin"
mkdir -p "$STAGE_DIR/usr/share/stem"
mkdir -p "$STAGE_DIR/usr/share/applications"
mkdir -p "$STAGE_DIR/usr/share/metainfo"
mkdir -p "$STAGE_DIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$DIST_DIR"

# 1. DEBIAN/control
cat << 'EOF' > "$STAGE_DIR/DEBIAN/control"
Package: stem
Version: 1.0.0
Section: sound
Priority: optional
Architecture: amd64
Maintainer: M. Özçelik <mozcelik@users.noreply.github.com>
Depends: python3 (>= 3.12), python3-gi, python3-gi-cairo, gir1.2-gtk-4.0, gir1.2-adw-1, gir1.2-gstreamer-1.0, gir1.2-gst-plugins-base-1.0, gstreamer1.0-tools, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-plugins-bad, gstreamer1.0-plugins-ugly, gstreamer1.0-libav, libsndfile1, ffmpeg
Homepage: https://github.com/MOzcelik14/steM
Description: steM. - AI Audio Separation Studio
 Separate the sound. Keep the soul.
 Modern GTK4 and Libadwaita audio workstation powered by Demucs v4
 deep learning models with synchronized phase-locked stem playback.
EOF

# 2. DEBIAN/postinst
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postinst"
#!/bin/sh
set -e

if [ "$1" = "configure" ]; then
    if which update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if which gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q /usr/share/icons/hicolor || true
    fi
fi

exit 0
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postinst"

# 3. DEBIAN/postrm
cat << 'EOF' > "$STAGE_DIR/DEBIAN/postrm"
#!/bin/sh
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    if which update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if which gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q /usr/share/icons/hicolor || true
    fi
fi

exit 0
EOF
chmod 755 "$STAGE_DIR/DEBIAN/postrm"

# 4. Copy Application Source & Data
cp -r "$ROOT_DIR/stem" "$STAGE_DIR/usr/share/stem/"
cp -r "$ROOT_DIR/data" "$STAGE_DIR/usr/share/stem/"
find "$STAGE_DIR/usr/share/stem" -type d -name "__pycache__" -exec rm -rf {} +
find "$STAGE_DIR/usr/share/stem" -type f -name "*.pyc" -delete

# Copy desktop and metainfo
cp "$ROOT_DIR/data/com.mozcelik.stem.desktop" "$STAGE_DIR/usr/share/applications/"
cp "$ROOT_DIR/data/com.mozcelik.stem.metainfo.xml" "$STAGE_DIR/usr/share/metainfo/"

# Copy icons
for size in 16 24 32 48 64 128 256 512; do
    src_icon="$ROOT_DIR/data/icons/hicolor/${size}x${size}/apps/com.mozcelik.stem.png"
    if [ -f "$src_icon" ]; then
        dest_dir="$STAGE_DIR/usr/share/icons/hicolor/${size}x${size}/apps"
        mkdir -p "$dest_dir"
        cp "$src_icon" "$dest_dir/"
    fi
done
cp "$ROOT_DIR/data/icons/hicolor/scalable/apps/com.mozcelik.stem.svg" "$STAGE_DIR/usr/share/icons/hicolor/scalable/apps/"

# 5. Executable Launcher /usr/bin/stem
cat << 'EOF' > "$STAGE_DIR/usr/bin/stem"
#!/usr/bin/env bash
set -e

export PYTHONPATH="/usr/share/stem:${PYTHONPATH:-}"

# Check if demucs is installed in system python
if python3 -c "import demucs, soundfile" >/dev/null 2>&1; then
    exec python3 -m stem.app "$@"
fi

# Fallback to isolated user environment in ~/.local/share/stem/venv
VENV_DIR="$HOME/.local/share/stem/venv"
if [ ! -f "$VENV_DIR/bin/python" ]; then
    echo "[steM.] First run: setting up AI environment in $VENV_DIR..."
    mkdir -p "$HOME/.local/share/stem"
    python3 -m venv --system-site-packages "$VENV_DIR"
    "$VENV_DIR/bin/pip" install --upgrade pip
    if command -v nvidia-smi &>/dev/null; then
        "$VENV_DIR/bin/pip" install torch torchaudio --index-url https://download.pytorch.org/whl/cu124
    else
        "$VENV_DIR/bin/pip" install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
    fi
    "$VENV_DIR/bin/pip" install demucs soundfile numpy scipy
fi

exec "$VENV_DIR/bin/python" -m stem.app "$@"
EOF
chmod 755 "$STAGE_DIR/usr/bin/stem"

# 6. Build the .deb package
DEB_TARGET="$DIST_DIR/steM-Linux-1.0.0.deb"
dpkg-deb --build --root-owner-group "$STAGE_DIR" "$DEB_TARGET"

echo "[+] Debian package successfully built at:"
echo "    $DEB_TARGET"
ls -lh "$DEB_TARGET"
