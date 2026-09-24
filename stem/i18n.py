"""
steM. - Internationalization (i18n) & Localization
Supports English and Turkish language selection with runtime dynamic switching.
Created by M. Özçelik
"""

import locale
import os
from typing import Any, Callable, Dict, List, Optional

from stem.config import settings

TRANSLATIONS: Dict[str, Dict[str, str]] = {
    "en": {
        # App & Header
        "app_title": "steM.",
        "app_subtitle": "AI Audio Separation Studio",
        "tab_welcome": "Welcome",
        "tab_separation": "Separation",
        "tab_mixer": "Mixer Studio",
        "menu_preferences": "Preferences",
        "menu_about": "About steM.",

        # Welcome View
        "by_author": "by {author}",
        "drag_drop_title": "Drag & Drop Audio Files Here",
        "supported_formats": "Supports MP3, WAV, FLAC, OGG, M4A, AAC",
        "browse_files": "Browse Files...",
        "select_audio_dialog_title": "Select Audio Files — steM.",
        "audio_files_filter": "Audio Files (*.mp3, *.wav, *.flac, *.ogg, *.m4a)",
        "hw_cuda": "NVIDIA CUDA: {name} ({vram} MB VRAM)",
        "hw_cpu": "Processing Mode: CPU Fallback",

        # Separation View - Queue
        "queue_title": "Audio Processing Queue",
        "add_files": "Add Files...",
        "clear": "Clear",
        "status_pending": "Pending",
        "status_processing": "Separating... {pct}%",
        "status_completed": "Completed",
        "status_cancelled": "Cancelled",
        "status_failed": "Failed",
        "tooltip_open_mixer": "Open in Mixer Studio",
        "tooltip_remove_queue": "Remove from Queue",

        # Separation View - Settings
        "settings_group_title": "AI Separation Settings",
        "settings_group_desc": "Demucs Model &amp; Hardware Configuration",
        "demucs_model": "Demucs Model",
        "model_htdemucs": "HTDemucs v4 (Default 4-Stem)",
        "model_htdemucs_ft": "HTDemucs Fine-Tuned (Studio Quality)",
        "model_htdemucs_6s": "HTDemucs 6-Stems (with Guitar/Piano)",
        "model_mdx_extra": "MDX-Net Extra",
        "two_stems_title": "2-Stem Isolation (Vocal + Instrumental)",
        "two_stems_sub": "Extract only vocals and backing track",
        "device_title": "Processing Device",
        "device_auto": "Auto-Detect ({dev})",
        "device_cuda": "NVIDIA CUDA ({name})",
        "device_cpu": "CPU Fallback (Multithreaded)",
        "quality_title": "Separation Precision (Shifts)",
        "quality_fast": "Fast (Shifts: 0)",
        "quality_balanced": "Balanced Studio (Shifts: 1 — Recommended)",
        "quality_high": "High Fidelity (Shifts: 2)",
        "low_vram_title": "Low VRAM Safety (4 GB Optimization)",
        "low_vram_sub": "Applies segment chunking to prevent CUDA OOM",
        "export_format_title": "Export Format",
        "format_wav": "WAV (24-bit Lossless)",
        "format_mp3": "MP3 (320 kbps High Quality)",
        "dest_dir_title": "Destination Directory",
        "select_output_folder": "Select Output Folder — steM.",
        "btn_separate": "Separate Stems",
        "btn_cancel": "Cancel",

        # Mixer View
        "no_track_loaded": "No Track Loaded",
        "btn_export_stems": "Export Stems...",
        "btn_stop": "Stop",
        "btn_play": "Play",
        "btn_pause": "Pause",
        "loop_playback": "Loop Playback",
        "stem_master": "Master Track",
        "stem_vocals": "Vocals",
        "stem_drums": "Drums",
        "stem_bass": "Bass",
        "stem_other": "Other",
        "stem_no_vocals": "Instrumental",
        "stem_guitar": "Guitar",
        "stem_piano": "Piano",
        "tooltip_mute": "Mute {name}",
        "tooltip_solo": "Solo {name}",

        # Preferences Dialog
        "pref_title": "Preferences — steM.",
        "pref_page_general": "General",
        "pref_group_language": "Interface Language",
        "pref_language_title": "Language",
        "pref_language_sub": "Select interface display language",
        "lang_auto": "System Default",
        "lang_tr": "Türkçe",
        "lang_en": "English",
        "pref_group_ai": "Demucs Separation Defaults",
        "pref_model_title": "Default Model",
        "pref_vram_title": "Low VRAM Mode (4 GB RTX 3050 Optimization)",
        "pref_vram_sub": "Restricts segment size to 6s and jobs to 1 to avoid CUDA OOM",
        "pref_group_export": "Export & File Storage",
        "pref_dest_title": "Default Stems Destination",
        "pref_select_dest": "Select Default Destination Directory",
        "pref_format_title": "Default Export Format",
        "pref_bitrate_title": "MP3 Bitrate",
        "pref_group_hw": "Hardware Diagnostics",
        "pref_gpu_device": "GPU Device",
        "pref_vram": "Dedicated VRAM",
        "pref_driver": "NVIDIA Driver",

        # Toasts & Messages
        "toast_added_tracks": "Added {n} tracks to separation queue",
        "toast_exporting": "Exporting stems...",
        "toast_export_success": "Successfully exported stems to {dir}",
        "toast_export_error": "Export failed: {err}",
        "toast_file_skipped": "Skipped {filename}: {err}",
        "toast_stems_loaded": "Loaded stems for '{track}' in Mixer Studio.",
        "toast_no_stems": "No stems available to export.",
        "toast_exported_n_stems": "Exported {n} stems to {dest}",
        "toast_oom_cpu": "GPU memory exhausted! Switched to CPU fallback for: {track}",
        "toast_lang_changed": "Language updated.",
        "btn_open_folder": "Open Folder",
        "btn_retry_cpu": "Retry on CPU",
        "oom_dialog_title": "NVIDIA GPU Out of Memory (CUDA OOM)",
        "oom_dialog_body": "The track '{track}' exceeded the 4 GB VRAM limit of your RTX 3050.\n\nWould you like to retry separation on CPU?",
        "status_retrying_cpu": "Retrying on CPU...",

        # About Dialog
        "about_comments": "• Python {py_ver}\n• GTK 4 & Libadwaita\n• GStreamer 1.0 Multi-Track Audio Engine\n• AI Model: Demucs v4 (Hybrid Transformer)\n• GPU Acceleration: {gpu_info}",
    },
    "tr": {
        # App & Header
        "app_title": "steM.",
        "app_subtitle": "Yapay Zekâ Destekli Ses Ayrıştırma Stüdyosu",
        "tab_welcome": "Giriş",
        "tab_separation": "Ayrıştırma",
        "tab_mixer": "Mikser Stüdyosu",
        "menu_preferences": "Tercihler",
        "menu_about": "steM. Hakkında",

        # Welcome View
        "by_author": "Geliştirici: {author}",
        "drag_drop_title": "Ses Dosyalarını Buraya Sürükleyip Bırakın",
        "supported_formats": "MP3, WAV, FLAC, OGG, M4A, AAC formatlarını destekler",
        "browse_files": "Dosyalara Göz At...",
        "select_audio_dialog_title": "Ses Dosyalarını Seçin — steM.",
        "audio_files_filter": "Ses Dosyaları (*.mp3, *.wav, *.flac, *.ogg, *.m4a)",
        "hw_cuda": "NVIDIA CUDA: {name} ({vram} MB VRAM)",
        "hw_cpu": "İşlem Modu: CPU Geri Dönüşü (Fallback)",

        # Separation View - Queue
        "queue_title": "Ses İşleme Kuyruğu",
        "add_files": "Dosya Ekle...",
        "clear": "Temizle",
        "status_pending": "Bekliyor",
        "status_processing": "Ayrıştırılıyor... %{pct}",
        "status_completed": "Tamamlandı",
        "status_cancelled": "İptal Edildi",
        "status_failed": "Başarısız",
        "tooltip_open_mixer": "Mikser Stüdyosunda Aç",
        "tooltip_remove_queue": "Kuyruktan Kaldır",

        # Separation View - Settings
        "settings_group_title": "Yapay Zekâ Ayrıştırma Ayarları",
        "settings_group_desc": "Demucs Model ve Donanım Yapılandırması",
        "demucs_model": "Demucs Modeli",
        "model_htdemucs": "HTDemucs v4 (Varsayılan 4 Kanal)",
        "model_htdemucs_ft": "HTDemucs İnce Ayarlı (Stüdyo Kalitesi)",
        "model_htdemucs_6s": "HTDemucs 6 Kanal (Gitar ve Piyano dahil)",
        "model_mdx_extra": "MDX-Net Extra",
        "two_stems_title": "2 Kanallı Ayrıştırma (Vokal + Altyapı)",
        "two_stems_sub": "Yalnızca vokal ve enstrümantal altyapıyı çıkarır",
        "device_title": "İşlem Birimi",
        "device_auto": "Otomatik Algıla ({dev})",
        "device_cuda": "NVIDIA CUDA ({name})",
        "device_cpu": "CPU (Çok Çekirdekli)",
        "quality_title": "Ayrıştırma Hassasiyeti (Shifts)",
        "quality_fast": "Hızlı (Shifts: 0)",
        "quality_balanced": "Dengeli Stüdyo (Shifts: 1 — Önerilen)",
        "quality_high": "Yüksek Kalite (Shifts: 2)",
        "low_vram_title": "Düşük VRAM Koruması (4 GB Optimizasyonu)",
        "low_vram_sub": "Bellek aşımını (CUDA OOM) önlemek için parçalı işleme uygular",
        "export_format_title": "Dışa Aktarma Formatı",
        "format_wav": "WAV (24-bit Kayıpsız PCM)",
        "format_mp3": "MP3 (320 kbps Yüksek Kalite)",
        "dest_dir_title": "Hedef Klasör",
        "select_output_folder": "Hedef Klasörü Seçin — steM.",
        "btn_separate": "Kanalları Ayrıştır",
        "btn_cancel": "İptal Et",

        # Mixer View
        "no_track_loaded": "Yüklü Parça Yok",
        "btn_export_stems": "Kanalları Dışa Aktar...",
        "btn_stop": "Durdur",
        "btn_play": "Oynat",
        "btn_pause": "Duraklat",
        "loop_playback": "Döngüsel Oynatma",
        "stem_master": "Ana Parça",
        "stem_vocals": "Vokal",
        "stem_drums": "Davul",
        "stem_bass": "Bas",
        "stem_other": "Diğer",
        "stem_no_vocals": "Enstrümantal",
        "stem_guitar": "Gitar",
        "stem_piano": "Piyano",
        "tooltip_mute": "{name} Sustur",
        "tooltip_solo": "{name} Solo Yap",

        # Preferences Dialog
        "pref_title": "Tercihler — steM.",
        "pref_page_general": "Genel",
        "pref_group_language": "Arayüz Dili",
        "pref_language_title": "Dil / Language",
        "pref_language_sub": "Uygulama arayüzü görüntüleme dili",
        "lang_auto": "Sistem Varsayılanı (System Default)",
        "lang_tr": "Türkçe",
        "lang_en": "English",
        "pref_group_ai": "Demucs Ayrıştırma Varsayılanları",
        "pref_model_title": "Varsayılan Model",
        "pref_vram_title": "Düşük VRAM Modu (4 GB RTX 3050 Optimizasyonu)",
        "pref_vram_sub": "CUDA OOM hatasını önlemek için parça boyutunu 6s ve işçi sayısını 1 ile sınırlar",
        "pref_group_export": "Dışa Aktarma ve Dosya Depolama",
        "pref_dest_title": "Varsayılan Kanallar Konumu",
        "pref_select_dest": "Varsayılan Hedef Klasörü Seçin",
        "pref_format_title": "Varsayılan Format",
        "pref_bitrate_title": "MP3 Bit Hızı",
        "pref_group_hw": "Donanım Tanılama",
        "pref_gpu_device": "GPU Ekran Kartı",
        "pref_vram": "Ayrılmış VRAM",
        "pref_driver": "NVIDIA Sürücüsü",

        # Toasts & Messages
        "toast_added_tracks": "{n} parça ayrıştırma kuyruğuna eklendi",
        "toast_exporting": "Kanallar dışa aktarılıyor...",
        "toast_export_success": "Kanallar başarıyla aktarıldı: {dir}",
        "toast_export_error": "Dışa aktarma başarısız: {err}",
        "toast_file_skipped": "{filename} atlandı: {err}",
        "toast_stems_loaded": "'{track}' için kanallar Mikser Stüdyosuna yüklendi.",
        "toast_no_stems": "Dışa aktarılacak kanal bulunamadı.",
        "toast_exported_n_stems": "{n} kanal şuraya aktarıldı: {dest}",
        "toast_oom_cpu": "GPU belleği yetersiz kaldı! Şu parça için CPU'ya geçildi: {track}",
        "toast_lang_changed": "Dil ayarı güncellendi.",
        "btn_open_folder": "Klasörü Aç",
        "btn_retry_cpu": "CPU ile Tekrar Dene",
        "oom_dialog_title": "NVIDIA GPU Belleği Yetersiz (CUDA OOM)",
        "oom_dialog_body": "'{track}' parçası RTX 3050 ekran kartınızın 4 GB VRAM sınırını aştı.\n\nAyrıştırmayı CPU üzerinde tekrar denemek ister misiniz?",
        "status_retrying_cpu": "CPU üzerinde yeniden deneniyor...",

        # About Dialog
        "about_comments": "• Python {py_ver}\n• GTK 4 & Libadwaita\n• GStreamer 1.0 Çok Kanallı Ses Motoru\n• Yapay Zekâ Modeli: Demucs v4 (Hybrid Transformer)\n• GPU Hızlandırma: {gpu_info}",
    },
}

_current_lang: Optional[str] = None
_listeners: List[Callable[[str], None]] = []


def detect_system_language() -> str:
    """Detects system language from environment variables or locale."""
    for env_var in ("LC_ALL", "LC_MESSAGES", "LANG"):
        val = os.environ.get(env_var, "")
        if val.lower().startswith("tr"):
            return "tr"
    try:
        loc = locale.getdefaultlocale()[0]
        if loc and loc.lower().startswith("tr"):
            return "tr"
    except Exception:
        pass
    return "en"


def get_language() -> str:
    """Gets currently effective language ('en' or 'tr')."""
    global _current_lang
    if _current_lang is not None:
        return _current_lang

    saved = settings.get("language", "auto")
    if saved == "auto":
        _current_lang = detect_system_language()
    elif saved in ("tr", "en"):
        _current_lang = saved
    else:
        _current_lang = "en"
    return _current_lang


def set_language(lang: str) -> None:
    """Sets application language ('auto', 'tr', or 'en') and notifies listeners."""
    global _current_lang
    settings.set("language", lang)
    if lang == "auto":
        _current_lang = detect_system_language()
    else:
        _current_lang = lang if lang in ("tr", "en") else "en"

    for listener in list(_listeners):
        try:
            listener(_current_lang)
        except Exception as e:
            print(f"[steM. i18n] Listener error: {e}")


def add_language_listener(callback: Callable[[str], None]) -> None:
    """Registers a listener function to be called when language changes."""
    if callback not in _listeners:
        _listeners.append(callback)


def t(key: str, **kwargs: Any) -> str:
    """Translates a key into current language with optional keyword replacements."""
    lang = get_language()
    val = TRANSLATIONS.get(lang, {}).get(key)
    if val is None:
        val = TRANSLATIONS.get("en", {}).get(key, key)
    if kwargs:
        try:
            return val.format(**kwargs)
        except Exception:
            return val
    return val
