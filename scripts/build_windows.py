"""
steM. - Windows Build Script
Automates PyInstaller packaging using Python and native os.pathsep.
"""

import os
import shutil
import sys
from pathlib import Path
import PyInstaller.__main__

ROOT_DIR = Path(__file__).resolve().parent.parent
DIST_DIR = ROOT_DIR / "dist"
BUILD_DIR = ROOT_DIR / "build"

def main():
    print("=" * 60)
    print(" steM. — Windows Automated Build Script")
    print(" Creator: M. Özçelik")
    print("=" * 60)

    sep = os.pathsep
    ico_file = ROOT_DIR / "data" / "icons" / "com.mozcelik.stem.ico"
    png_file = ROOT_DIR / "data" / "icons" / "hicolor" / "256x256" / "apps" / "com.mozcelik.stem.png"
    icon_path = str(ico_file if ico_file.exists() else png_file)
    css_arg = f"{ROOT_DIR / 'stem' / 'ui' / 'style.css'}{sep}stem/ui"
    data_arg = f"{ROOT_DIR / 'data'}{sep}data"

    pyinstaller_args = [
        "--name=steM",
        "--windowed",
        f"--icon={icon_path}",
        f"--add-data={css_arg}",
        f"--add-data={data_arg}",
        "--collect-all=soundfile",
        "--collect-submodules=stem",
        "--noconfirm",
        str(ROOT_DIR / "stem" / "app.py"),
    ]

    print("[*] Running PyInstaller with args:")
    for arg in pyinstaller_args:
        print(f"    {arg}")

    PyInstaller.__main__.run(pyinstaller_args)

    output_dir = DIST_DIR / "steM"
    if output_dir.exists():
        print("[+] Build succeeded!")
        print(f"    Output folder: {output_dir}")
        zip_target = DIST_DIR / "steM-Windows-x64"
        print(f"[*] Packaging zip archive to {zip_target}.zip ...")
        shutil.make_archive(str(zip_target), "zip", root_dir=str(DIST_DIR), base_dir="steM")
        print(f"[+] Standalone zip created at: {zip_target}.zip")
    else:
        print("[-] Build finished but output directory was not found.", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
