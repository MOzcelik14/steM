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

## Architecture & Design Decisions

### Non-Destructive Waveform Edit List
Unlike traditional destructive editors that mutate audio files or allocate huge raw copies for undo history, **steM.** models single-file audio documents as an immutable sequence of segments (`AudioSegment::SourceSlice` and `AudioSegment::Silence`).
- Edits (cut, copy, paste, delete, trim, silence, reverse, invert phase) execute instantaneously in $O(N)$ segment count without copying or modifying underlying audio files on disk.
- An in-memory `AudioClipboard` carries planar audio buffers that register new immutable audio sources when pasted across documents.
- Snapping to zero-crossings identifies the closest sign-change transition sample $s[i] \cdot s[i+1] \le 0$ within a configurable search radius to ensure click-free splice boundaries.
- Dedicated transactional history (`WaveformHistory`) provides snapshot-based undo and redo.
- Fast multi-resolution peak pyramids (`PeakPyramid`) deliver smooth 60fps rendering at arbitrary zoom levels with $O(1)$ query overhead.

### Real-Time Audio Engine Safety & Effects Chains
- The audio callback runs with zero heap allocations, zero locking (`Mutex`/`RwLock`), zero system calls, and zero file I/O.
- State swaps between the GUI thread and audio callback use atomic pointers and lock-free single-producer single-consumer ring buffers (`rtrb`).
- Retired timeline buffers are transmitted across a dedicated channel back to the UI thread to ensure deallocations never happen on the audio thread.
- Real-time per-track insert effect chains (EQ, Compressor, Limiter, Delay, Reverb) and master chain execute sample-by-sample inside the audio callback with support for dynamic reordering, bypass, and reset.
- Dynamic parameter updates are smoothly interpolated on every sample using single-pole exponential smoothing (`SmoothedParam`), eliminating zipper noise and audio clicks.
- Fade operations (Linear, Logarithmic, Exponential, S-Curve) and peak normalization operate on audio selections non-destructively, rendering virtual slices into the edit list without modifying files on disk.

### Audio Cleanup, Restoration & Spectral Analysis
- **Spectral Gating Noise Reduction**: Background noise floor profiles are statistically captured across frequency bins using an upper envelope ($\mu + 1.8\sigma$) to account for Rayleigh distribution variance. Noise reduction applies Short-Time Fourier Transform (STFT) overlap-add with Hann windows (50% overlap Constant Overlap-Add) and Wiener spectral subtraction to ensure smooth, artifact-free attenuation without musical chirps.
- **Impulse Click & Pop Removal**: High-frequency transients and clicks are isolated via adaptive moving median difference filtering. Corrupted samples are seamlessly reconstructed using cubic Hermite spline interpolation anchored on surrounding clean signal derivatives.
- **2D Waterfall Spectrogram & Frequency Analyzer**: Configurable FFT window sizes (256 to 4096 samples) paired with scientific colormaps (Viridis, Magma, Inferno, Classic Heat, Cyberpunk) for simultaneous 2D time-frequency waterfall rendering and 1D magnitude curve analysis with frequency and dB gridlines.
- **Dual Peak & RMS Telemetry**: The real-time audio callback calculates both peak magnitude and RMS sum of squares with zero heap allocations, powering live transport VU meters, channel strips, and numerical readouts.
- **EBU R128 / ITU-R BS.1770 Loudness Report**: Comprehensive dialog measuring Integrated LUFS, Momentary max, Short-term max, Loudness Range (LRA), and True Peak with compliance indicators for broadcast and streaming platforms.

---

## Roadmap

- [x] **Milestone 1**: Project skeleton, localization engine (EN/TR), playback engine, lock-free telemetry.
- [x] **Phase A (Milestone 2)**: Single-file waveform editor mode, non-destructive edit list, zero-crossing snapping, selection playback, clipboard.
- [x] **Phase B (Milestone 3)**: Real-time per-track and master insert effect chains (EQ, Compressor, Limiter, Delay, Reverb) in audio callback, live preview, lock-free parameter smoothing (`SmoothedParam`), reordering, per-effect bypass/reset, fade curve operations (Linear, Logarithmic, Exponential, S-Curve), peak normalization, and selection rendering.
- [x] **Phase C (Milestone 4)**: Cleanup tools (spectral gating noise reduction, click/pop removal), 2D waterfall spectrogram & 1D spectrum analyzer with adjustable FFT size and color palettes, dual Peak/RMS metering, and EBU R128 loudness report.
- [ ] **Phase D (Milestone 5)**: Multi-channel recording completeness, punch-in/out, latency compensation.
- [ ] **Phase E (Milestone 7)**: Advanced multitrack session (busses, sends/returns, grouping).
- [ ] **Phase F (Milestone 8)**: Workflow polish, custom keybindings, theme support, session recovery.

---

## License

GNU General Public License v3.0 (GPL-3.0). See [LICENSE](LICENSE) for details.
