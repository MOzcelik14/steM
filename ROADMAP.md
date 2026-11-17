# steM — Development Roadmap

**Project:** steM  
**Version:** 0.5.x  
**Language:** Rust  
**Purpose:** Multitrack audio editing, stem separation, audio restoration and mastering.

---

## 1. Critical Improvements

These issues should be addressed before introducing major new features.

### 1.1 Offline Export and DSP Consistency

- [x] Make offline rendering use the same DSP processing chain as real-time playback.
- [x] Ensure track-level effects are applied during export.
- [x] Ensure master bus effects are applied during export.
- [x] Preserve automation, routing, sends and gain settings.
- [x] Add automated tests comparing playback rendering and offline rendering.
- [x] Verify that exported audio matches the project mix.

**Priority:** Critical

### 1.2 Audio Engine Track Limits

- [x] Resolve the inconsistency between the 256-track project validation limit and the 32-track audio engine limit.
- [x] Introduce a consistent track capacity policy (`MAX_PROJECT_TRACKS = 64`).
- [x] Handle track overflow gracefully.
- [x] Add stress tests for large projects.

**Priority:** Critical

### 1.3 Batch Export Reliability

- [x] Properly propagate export errors.
- [x] Do not count failed exports as successful.
- [x] Display individual export results.
- [x] Add cancellation support.
- [x] Prevent accidental overwriting without confirmation.
- [x] Add an export summary containing successful, failed and skipped files.

**Priority:** High

### 1.4 Audio Device Compatibility

- [x] Improve device channel configuration.
- [x] Support different input and output channel layouts.
- [x] Improve sample rate negotiation.
- [ ] Test compatibility with PipeWire, JACK and ALSA.
- [x] Improve device disconnection and reconnection handling.
- [x] Add configurable audio buffer sizes and latency settings.

**Priority:** High

### 1.5 Code Architecture

- [x] Reduce the size of oversized application modules.
- [x] Separate UI components from audio processing logic.
- [x] Improve module boundaries between `stem-core`, `stem-audio` and `stem-dsp`.
- [x] Keep the real-time audio callback isolated from UI operations.
- [x] Avoid unnecessary allocations and blocking operations in the audio callback.

**Priority:** High

---

## 2. Audio Plugin Support

### VST3 / LV2 Plugin Hosting

Introduce support for external audio plugins.

- [x] Research suitable Rust plugin hosting libraries.
- [x] Implement plugin discovery and scanning.
- [x] Implement plugin loading and unloading.
- [x] Support VST3 plugins.
- [x] Support LV2 plugins.
- [x] Create a plugin browser.
- [x] Allow plugins to be inserted into track effect chains.
- [x] Support plugin parameter automation.
- [x] Save plugin states inside project files.
- [x] Handle missing or incompatible plugins gracefully.

**Priority:** High

---

## 3. Spectral Audio Editing

Introduce advanced spectral editing and restoration capabilities.

### Spectral Repair

- [x] Improve spectrogram selection tools.
- [x] Allow frequency-range selection.
- [x] Allow time-frequency region selection.
- [x] Implement spectral attenuation.
- [x] Implement spectral interpolation.
- [x] Implement spectral noise reduction.
- [x] Add click and transient removal tools.
- [x] Support non-destructive spectral processing.

**Priority:** High

### Additional Restoration Tools

- [x] Broadband noise reduction.
- [x] Hum removal.
- [x] De-clicking.
- [x] De-clipping.
- [x] De-reverberation.
- [x] De-essing.

**Priority:** Medium

---

## 4. AI Stem Separation

Integrate AI-powered stem separation directly into the steM workflow.

### Demucs Integration

- [x] Implement Demucs integration.
- [x] Support vocals, drums, bass and other stems.
- [x] Allow users to select separation models.
- [x] Add separation progress reporting.
- [x] Support cancellation.
- [x] Automatically import generated stems into separate tracks.
- [x] Preserve synchronization between generated stems.
- [x] Allow users to replace or regenerate individual stems.

### Future Improvements

- [x] Explore GPU acceleration.
- [x] Explore alternative separation models.
- [x] Implement separation quality presets.
- [x] Support batch stem separation.

**Priority:** High

---

## 5. Advanced Mastering Tools

Develop a dedicated mastering workflow.

### Loudness and Dynamics Analysis

- [x] Integrated LUFS measurement.
- [x] Short-term LUFS measurement.
- [x] Momentary LUFS measurement.
- [x] True Peak measurement.
- [x] RMS measurement.
- [x] Crest factor analysis.
- [x] Dynamic range analysis.
- [x] Clipping detection.

### Mastering Assistant

- [x] Analyze the complete mix.
- [x] Detect excessive peaks and clipping.
- [x] Suggest safe gain adjustments.
- [x] Provide loudness target recommendations.
- [x] Provide tonal balance analysis.
- [x] Allow A/B comparisons.
- [x] Support reference track comparison.

### Export Improvements

- [x] True Peak limiting.
- [x] Configurable loudness targets.
- [x] Configurable dither.
- [x] High-quality sample rate conversion.
- [x] Export presets for common delivery platforms.

**Priority:** High

---

## 6. Advanced Automation

Expand the existing automation system.

- [x] Automate effect parameters.
- [x] Automate EQ frequency, gain and bandwidth.
- [x] Automate compressor threshold and ratio.
- [x] Automate reverb parameters.
- [x] Automate send levels.
- [x] Improve automation curve editing.
- [x] Support Bézier interpolation.
- [x] Improve automation visualization.
- [x] Support automation copy and paste.

**Priority:** Medium

---

## 7. Advanced Mixing and Routing

- [x] Sidechain compression.
- [x] Improved bus routing.
- [x] Flexible auxiliary sends.
- [x] Improved group management.
- [x] Track freezing and rendering.
- [x] Improved phase inversion controls.
- [x] Stereo width processing.
- [x] Mid/Side processing.
- [x] 5.1 surround support.

**Priority:** Medium

---

## 8. Workflow Improvements

- [x] A/B mastering comparison.
- [x] Reference audio import.
- [x] Reference loudness comparison.
- [x] Reference frequency spectrum comparison.
- [x] User-defined effect chain presets.
- [x] Project media collection (Collect All and Save).
- [x] Improved undo/redo history.
- [x] Waveform caching.
- [x] Improved timeline performance.
- [x] Export progress visualization.
- [x] Batch audio processing.
- [x] Project backup and recovery improvements.

**Priority:** Medium

---

## 9. Testing and Quality Assurance

- [x] Expand unit test coverage.
- [x] Add integration tests for audio rendering.
- [x] Add DSP regression tests.
- [x] Test different sample rates and bit depths.
- [x] Test long-duration audio projects.
- [x] Test projects containing 100+ tracks.
- [x] Test malformed project files.
- [x] Add performance benchmarks.
- [x] Test memory usage during long sessions.
- [x] Test audio callback stability under CPU load.
- [x] Verify cross-platform behavior.

---

## 10. Recommended Development Order

| Phase | Focus | Priority |
|---|---|---|
| 1 | Export/DSP consistency | Critical |
| 2 | Audio engine reliability | Critical |
| 3 | Batch export and device compatibility | High |
| 4 | VST3/LV2 plugin hosting | High |
| 5 | Spectral repair and restoration | High |
| 6 | Demucs integration | High |
| 7 | Advanced mastering | High |
| 8 | Automation and routing | Medium |
| 9 | Workflow improvements | Medium |
| 10 | Surround audio and advanced features | Future |

---

## Development Principles

All new features must follow these principles:

1. **Rust-first:** Keep the core application and audio processing infrastructure in Rust.
2. **Non-destructive editing:** Original audio files must never be modified by default.
3. **Real-time safety:** Never perform blocking operations, filesystem access or unpredictable memory allocations inside the real-time audio callback.
4. **DSP consistency:** Real-time playback and offline rendering must produce equivalent results.
5. **Performance:** Optimize for low latency, low memory usage and efficient CPU utilization.
6. **Modularity:** Keep audio processing, project management and UI responsibilities separated.
7. **Reliability:** Every critical audio operation must have proper error handling.
8. **Backward compatibility:** Existing project files must remain readable whenever possible.
9. **Internationalization:** All user-facing strings must use the existing localization infrastructure.
10. **Quality over quantity:** Do not introduce unfinished features merely to increase the feature count.

---

**Long-term Vision**

steM aims to become a modular, professional-grade audio workstation focused on multitrack editing, AI-powered stem separation, spectral restoration and mastering.

The priority is not to replicate existing DAWs feature by feature, but to build a reliable and distinctive audio production environment.
