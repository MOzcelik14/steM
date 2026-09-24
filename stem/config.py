"""
steM. - AI Audio Separation Studio
Application configuration, constants, and settings manager.
Created by M. Özçelik
"""

import json
import os
from pathlib import Path
from typing import Any, Dict

APP_ID = "com.mozcelik.stem"
APP_NAME = "steM."
APP_VERSION = "1.0.1"
APP_AUTHOR = "M. Özçelik"
APP_SUBTITLE = "AI Audio Separation Studio"
GITHUB_URL = "https://github.com/MOzcelik14/steM"

# Standard paths
if os.name == "nt":
    CONFIG_DIR = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming")) / "steM"
    CACHE_DIR = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData" / "Local")) / "steM"
else:
    CONFIG_DIR = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "steM"
    CACHE_DIR = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "steM"

DATA_DIR = Path(__file__).resolve().parent.parent / "data"

# Default Music directory
DEFAULT_MUSIC_DIR = Path.home() / "Music"
if not DEFAULT_MUSIC_DIR.exists():
    DEFAULT_MUSIC_DIR = Path.home() / "Müzik"
if not DEFAULT_MUSIC_DIR.exists():
    DEFAULT_MUSIC_DIR = Path.home()

DEFAULT_OUTPUT_DIR = DEFAULT_MUSIC_DIR / "steM_Stems"

# Supported Demucs Models
AVAILABLE_MODELS = {
    "htdemucs": {
        "name": "htdemucs",
        "label": "HTDemucs v4 (Default, Balanced)",
        "description": "Standard Hybrid Transformer. 4 stems (Vocals, Drums, Bass, Other). Fast and high quality.",
        "stems": ["vocals", "drums", "bass", "other"],
        "supports_two_stems": True,
    },
    "htdemucs_ft": {
        "name": "htdemucs_ft",
        "label": "HTDemucs Fine-Tuned (Studio Quality)",
        "description": "Fine-tuned version of HTDemucs. Higher fidelity, requires slightly more memory.",
        "stems": ["vocals", "drums", "bass", "other"],
        "supports_two_stems": True,
    },
    "htdemucs_6s": {
        "name": "htdemucs_6s",
        "label": "HTDemucs 6-Stems (Guitar & Piano)",
        "description": "6 stems (Vocals, Drums, Bass, Other, Guitar, Piano).",
        "stems": ["vocals", "drums", "bass", "other", "guitar", "piano"],
        "supports_two_stems": False,
    },
    "mdx_extra": {
        "name": "mdx_extra",
        "label": "MDX-Net Extra",
        "description": "MDX-Net architecture trained on extra dataset. Great vocal isolation.",
        "stems": ["vocals", "drums", "bass", "other"],
        "supports_two_stems": True,
    },
}

# Stem display properties and colors (DAW color scheme)
STEM_METADATA = {
    "vocals": {
        "label": "Vocals",
        "color": "#9d4edd",  # Vibrant Purple
        "icon": "audio-speakers-symbolic",
    },
    "drums": {
        "label": "Drums",
        "color": "#f77f00",  # Amber Orange
        "icon": "audio-volume-high-symbolic",
    },
    "bass": {
        "label": "Bass",
        "color": "#00b4d8",  # Cyan Blue
        "icon": "audio-card-symbolic",
    },
    "other": {
        "label": "Other",
        "color": "#06d6a0",  # Emerald Green
        "icon": "media-playlist-consecutive-symbolic",
    },
    "no_vocals": {
        "label": "Instrumental",
        "color": "#06d6a0",  # Emerald Green
        "icon": "audio-speakers-symbolic",
    },
    "guitar": {
        "label": "Guitar",
        "color": "#e63946",  # Crimson Red
        "icon": "audio-speakers-symbolic",
    },
    "piano": {
        "label": "Piano",
        "color": "#e9c46a",  # Warm Gold
        "icon": "audio-speakers-symbolic",
    },
}

DEFAULT_SETTINGS: Dict[str, Any] = {
    "output_dir": str(DEFAULT_OUTPUT_DIR),
    "model": "htdemucs",
    "two_stems": False,
    "two_stems_target": "vocals",
    "device_preference": "auto",  # auto, cuda, cpu
    "low_vram_mode": True,       # Recommended for 4GB VRAM
    "segment_size": 6,           # Seconds per segment (4-8 recommended for 4GB)
    "shifts": 1,                 # 0=Fastest, 1=Balanced, 2=Studio
    "overlap": 0.25,
    "export_format": "wav",      # wav, mp3
    "mp3_bitrate": 320,          # 192, 256, 320 kbps
    "normalize_output": True,
    "dark_theme": True,
    "language": "auto",  # auto, tr, en
}


class SettingsManager:
    """Manages persistent application settings via JSON storage."""

    def __init__(self) -> None:
        self.config_file = CONFIG_DIR / "settings.json"
        self._settings = dict(DEFAULT_SETTINGS)
        self.load()

    def load(self) -> None:
        if self.config_file.exists():
            try:
                with open(self.config_file, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        self._settings.update(data)
            except Exception as e:
                print(f"[steM.] Warning: Failed to load settings: {e}")

    def save(self) -> None:
        try:
            CONFIG_DIR.mkdir(parents=True, exist_ok=True)
            with open(self.config_file, "w", encoding="utf-8") as f:
                json.dump(self._settings, f, indent=2, ensure_ascii=False)
        except Exception as e:
            print(f"[steM.] Warning: Failed to save settings: {e}")

    def get(self, key: str, default: Any = None) -> Any:
        return self._settings.get(key, default if default is not None else DEFAULT_SETTINGS.get(key))

    def set(self, key: str, value: Any) -> None:
        self._settings[key] = value
        self.save()

    def all(self) -> Dict[str, Any]:
        return dict(self._settings)


# Global settings singleton
settings = SettingsManager()
