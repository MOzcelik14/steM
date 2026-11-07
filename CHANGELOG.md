# Changelog

All notable changes to **steM.** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.5] - 2026-11-07

### Added
- **Modern Studio DAW UI Overhaul**:
  - Refined studio dark carbon theme (`#121419`), panel fill (`#16181e`), and neon blue/cyan accents (`#38bdf8`).
  - Studio hairline borders (`1.0px #262a34`, active `#38bdf8`) with expanded button padding and comfortable margins.
  - Studio pill transport controls: vibrant emerald active Play button, silver Stop, studio crimson Record, and cyan Loop pill.
  - Recessed LCD digital timecode display (`#0c0e12`) with glowing monospace digits (`#34d399`), mode tag, and one-click cycling between Timecode, SMPTE, Bars/Beats, and Samples.
  - Connected segmented view switcher pill for Multitrack and Waveform editor modes.
  - Dedicated DAW-standard studio square track buttons: Mute (`M`), Solo (`S`), and Arm (`R`) with distinct color codes.
  - Audio clip cards with dedicated 16px titlebar strips, preventing waveforms from clashing with clip titles.
  - Subtle zero-crossing center line across waveform bodies, rendered with multi-resolution `PeakPyramid` in studio cyan.
  - Embedded high-resolution vector application branding logo next to the top menu bar title.
- **Audacity-Style Rectangular Time Selection**:
  - Dragging empty canvas space (or holding Shift over clips) draws a semi-transparent blue selection box across all tracks and ruler.
  - Left and right edge handles allow resizing the selection range dynamically.
  - Single-click repositions the playhead cursor at the clicked timestamp and clears the selection.
  - `Escape` key clears the active time range selection.
  - Playback automatically respects selection boundaries, stopping or looping at the end of the range.
- **Timeline Panning & Navigation**:
  - Horizontal timeline scrolling via `Shift + Mouse Wheel`.
  - Vertical track lane scrolling via standard `Mouse Wheel`.
  - Simultaneous horizontal and vertical panning via `Middle-Click Drag`.
  - Interactive horizontal and vertical scrollbars.
- **Editing Keyboard Shortcuts**:
  - Split clip under playhead with `Ctrl+K`.
  - Duplicate selected clip with `Ctrl+D`.
- **Unsaved Changes Dialog**:
  - Added save confirmation modal on close request (`close_requested()` and `File -> Exit`) with "Kaydet", "Kaydetmeden Kapat", and "İptal".

### Changed
- **Audio Import & Decoding Optimization**:
  - Completely refactored `decode_stream` in `stem-audio` to wrap the decoding pipeline once instead of calling `catch_unwind_silent` per packet.
  - Eliminated up to 200,000 closure allocations and lock contentions per audio file import, speeding up loading by up to 10x.
  - Pre-reserved vector capacities based on container header estimates.
- **Top Menu Bar Clean-Up**:
  - Removed top-level language switcher dropdown; language selection is now cleanly organized inside the Preferences dialog.
- **Case-Insensitive Audio Import**:
  - Expanded file dialog filters on Linux/Unix to support uppercase audio extensions (`.MP3`, `.WAV`, `.FLAC`, `.OGG`, `.AAC`, `.M4A`, `.AIFF`).

### Fixed
- **Clip Dragging Crash Fixed**:
  - Fixed `f32.clamp` panic (`min > max, or either was NaN`) caused by dragging or trimming clip ends past boundary thresholds.
  - Enforced minimum clip duration and guarded fade handle hitboxes.
- **Unsaved Close Infinite Loop Fixed**:
  - Resolved dialog re-triggering loop upon choosing "Don't Save" by adding a `force_close` bypass flag.
- **Missing Font Glyphs Fixed**:
  - Replaced unsupported emoji codepoints (`🎚` and `📊`) with clean, universal text labels ("Mikser" and "Spektrogram").

---

## [0.1.1] - 2026-10-31

### Added
- Multi-resolution PeakPyramid waveform caching.
- Lock-free ring buffer communication with real-time audio threads.
- Comprehensive security and robustness test suites.

---

## [0.1.0] - 2026-10-30

### Added
- Initial public release of steM. multi-track audio workstation.
- Non-destructive RON project format and audio engine.
