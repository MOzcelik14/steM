# steM.

**steM.** is an open-source, non-destructive multi-track audio editor and mixing workstation for Linux and Windows—designed as a high-performance, quality-first modern alternative to Adobe Audition.

```
          _         __  __ 
 ___ | |_ ___ |  \/  |.
/ __|| __/ _ \| |\/| |.
\__ \| ||  __/| |  | |.
|___/ \__\___||_|  |_|.
```

---

## Core Principles

1. **Never block or allocate on the audio thread.**
   The real-time audio callback never performs heap allocations, mutex locking, file I/O, or logging. The UI communicates with the audio engine via lock-free single-producer single-consumer ring buffers (`rtrb`) and atomics.
2. **Non-destructive editing.**
   Source audio files are never modified on disk. Edits are stored as an edit list (clips referencing source ranges with gain, fades, and effects chains) in the human-readable RON project file. Rendering to a new audio file occurs strictly on explicit export or bounce.
3. **Sample-accurate and quality-first.**
   Audio is processed internally as 32-bit floating point at the source sample rate, resampling only at the hardware device boundary or upon export. When reducing bit depth (to 16-bit or 24-bit PCM), Triangular Probability Density Function (TPDF) dither is applied to eliminate harmonic quantization distortion.
4. **Smooth waveform display at any zoom.**
   A multi-resolution peak cache (pyramid min/max per block at powers of two: 64, 256, 1024, 4096, 16384 samples) is computed in the background, rendering waveforms in $O(1)$ screen pixels regardless of whether the audio is 10 seconds or 10 hours long.
5. **Localization from day one.**
   Every user-facing string is routed through the Fluent localization layer (`locales/en.ftl` and `locales/tr.ftl`), embedded at compile-time. The active locale defaults to the system locale (falling back to English) and can be switched dynamically at runtime without restarting. Turkish audio terminology adheres to standard Turkish audio engineering conventions.

---

## Tech Stack

| Component | Library / Framework | Purpose |
|---|---|---|
| **Language** | Rust (stable 2021 edition) | Memory safety, zero-cost abstractions, real-time safety |
| **User Interface** | `egui` / `eframe` | Fast, immediate-mode GUI with smooth 60fps rendering |
| **Audio I/O** | `cpal` | Cross-platform audio device enumeration and low-latency streams |
| **Lock-free Messaging** | `rtrb` | Real-time safe audio thread ring buffer |
| **Decoding** | `symphonia` | Pure Rust audio decoder: WAV, FLAC, MP3, OGG/Vorbis, AAC/M4A |
| **Encoding** | `hound` (WAV), `flacenc` (FLAC) | Lossless PCM and compressed FLAC export |
| **Resampling** | `rubato` | High-quality asynchronous and synchronous audio resampling |
| **Spectral Analysis** | `realfft` / `rustfft` | Real-to-complex FFT for real-time spectrogram visualization |
| **Loudness Metering** | `ebur128` | ITU-R BS.1770-4 / EBU R128 integrated, momentary, and short-term LUFS |
| **Localization** | `fluent-bundle` + `sys-locale` | FTL translation bundles (EN & TR) and system locale detection |
| **Project Format** | `ron` + `serde` | Human-readable Rusty Object Notation project files |
| **License** | GPL-3.0 | Free and open source software |

---

## Repository Layout

```
stem/
├── Cargo.toml                # Workspace definition
├── locales/
│   ├── en.ftl                # English localization
│   └── tr.ftl                # Turkish localization (Standard Turkish audio terms)
├── crates/
│   ├── stem-app/             # Binary application: GUI views, timeline, transport, mixer
│   ├── stem-core/            # Project model, clips, tracks, undo/redo history, peak caches
│   ├── stem-audio/           # cpal device stream, real-time callback, decoder, export engine
│   ├── stem-dsp/             # Pure DSP: filters, compressor/limiter, spatial, dither, FFT
│   └── stem-i18n/            # Localization runtime and audio formatting helpers
└── README.md
```

> **Note on naming**: The public display name is **steM.** (with trailing dot). In code, this is defined once in `stem_i18n::APP_DISPLAY_NAME` and re-exported throughout. Package names, crate directories, and CLI identifiers remain lowercase and dot-free (`stem`).

---

## Building and Running

### Prerequisites

Ensure you have the Rust toolchain installed (Rust 1.80+):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Linux, install ALSA development headers:
```bash
# Ubuntu / Debian
sudo apt-get install libasound2-dev pkg-config
```

### Running the Application

```bash
cargo run -p stem-app
```

### Running the Test Suite

Run all workspace tests, including the translation parity test between English and Turkish:
```bash
cargo test --workspace
```

---

## License

GNU General Public License v3.0 (GPL-3.0). See [LICENSE](LICENSE) for details.
