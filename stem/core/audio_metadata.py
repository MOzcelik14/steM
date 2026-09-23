"""
steM. - Audio Metadata Extraction & Validation
Inspects audio files, extracts duration, channels, sample rate, and tags.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Optional

SUPPORTED_AUDIO_EXTENSIONS = {
    ".wav", ".mp3", ".flac", ".ogg", ".m4a", ".aac", ".opus", ".aiff", ".wma"
}


@dataclass
class AudioFileInfo:
    path: Path
    filename: str
    duration_seconds: float
    duration_str: str
    sample_rate: int
    channels: int
    channels_str: str
    format_name: str
    file_size_mb: float
    title: Optional[str] = None
    artist: Optional[str] = None
    album: Optional[str] = None
    is_valid: bool = True
    error_message: Optional[str] = None


def format_duration(seconds: float) -> str:
    """Formats seconds into MM:SS or HH:MM:SS."""
    if seconds <= 0:
        return "00:00"
    m, s = divmod(int(seconds), 60)
    h, m = divmod(m, 60)
    if h > 0:
        return f"{h:02d}:{m:02d}:{s:02d}"
    return f"{m:02d}:{s:02d}"


def inspect_audio_file(file_path: Path | str) -> AudioFileInfo:
    """
    Validates and extracts metadata from an audio file.
    Uses soundfile or GStreamer fallback.
    """
    p = Path(file_path).resolve()
    if not p.exists():
        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=0.0,
            duration_str="00:00",
            sample_rate=0,
            channels=0,
            channels_str="Unknown",
            format_name=p.suffix.lower().lstrip("."),
            file_size_mb=0.0,
            is_valid=False,
            error_message="File does not exist.",
        )

    file_size_mb = p.stat().st_size / (1024 * 1024)
    if p.stat().st_size == 0:
        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=0.0,
            duration_str="00:00",
            sample_rate=0,
            channels=0,
            channels_str="Unknown",
            format_name=p.suffix.lower().lstrip("."),
            file_size_mb=0.0,
            is_valid=False,
            error_message="File is empty (0 bytes).",
        )

    ext = p.suffix.lower()
    if ext not in SUPPORTED_AUDIO_EXTENSIONS:
        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=0.0,
            duration_str="00:00",
            sample_rate=0,
            channels=0,
            channels_str="Unknown",
            format_name=ext.lstrip("."),
            file_size_mb=file_size_mb,
            is_valid=False,
            error_message=f"Unsupported format '{ext}'. Supported: {', '.join(sorted(SUPPORTED_AUDIO_EXTENSIONS))}",
        )

    # Attempt metadata extraction via soundfile
    try:
        import soundfile as sf
        info = sf.info(str(p))
        duration = float(info.duration)
        channels = int(info.channels)
        ch_str = "Mono" if channels == 1 else ("Stereo" if channels == 2 else f"{channels} Ch")
        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=duration,
            duration_str=format_duration(duration),
            sample_rate=int(info.samplerate),
            channels=channels,
            channels_str=ch_str,
            format_name=info.format or ext.lstrip(".").upper(),
            file_size_mb=round(file_size_mb, 2),
            title=p.stem,
            is_valid=True,
        )
    except Exception:
        pass

    # Fallback via GStreamer GstDiscoverer
    try:
        import gi
        gi.require_version("Gst", "1.0")
        gi.require_version("GstPbutils", "1.0")
        from gi.repository import Gst, GstPbutils
        Gst.init(None)

        discoverer = GstPbutils.Discoverer.new(5 * Gst.SECOND)
        uri = p.as_uri()
        info = discoverer.discover_uri(uri)
        dur_ns = info.get_duration()
        dur_sec = dur_ns / Gst.SECOND if dur_ns > 0 else 0.0

        sample_rate = 44100
        channels = 2
        audio_streams = info.get_audio_streams()
        if audio_streams:
            astream = audio_streams[0]
            sample_rate = astream.get_sample_rate()
            channels = astream.get_channels()

        ch_str = "Mono" if channels == 1 else ("Stereo" if channels == 2 else f"{channels} Ch")
        tags = info.get_tags()
        title = p.stem
        artist = None
        album = None
        if tags:
            title_success, t = tags.get_string("title")
            if title_success and t:
                title = t
            art_success, a = tags.get_string("artist")
            if art_success and a:
                artist = a
            alb_success, al = tags.get_string("album")
            if alb_success and al:
                album = al

        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=dur_sec,
            duration_str=format_duration(dur_sec),
            sample_rate=sample_rate,
            channels=channels,
            channels_str=ch_str,
            format_name=ext.lstrip(".").upper(),
            file_size_mb=round(file_size_mb, 2),
            title=title,
            artist=artist,
            album=album,
            is_valid=True,
        )
    except Exception as e:
        # Generic fallback
        return AudioFileInfo(
            path=p,
            filename=p.name,
            duration_seconds=0.0,
            duration_str="--:--",
            sample_rate=44100,
            channels=2,
            channels_str="Stereo",
            format_name=ext.lstrip(".").upper(),
            file_size_mb=round(file_size_mb, 2),
            title=p.stem,
            is_valid=True,
            error_message=f"Metadata notice: {e}",
        )
