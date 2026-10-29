# steM. English localization file

app-title = steM.
app-tagline = Non-destructive Audio Editor & Mixer

# Menu Bar - File
menu-file = File
menu-file-new = New Project
menu-file-open = Open Project...
menu-file-save = Save Project
menu-file-save-as = Save Project As...
menu-file-import-audio = Import Audio...
menu-file-export = Export Audio...
menu-file-export-selection = Export Selection...
menu-file-preferences = Preferences...
menu-file-exit = Exit

# Menu Bar - Edit
menu-edit = Edit
menu-edit-undo = Undo
menu-edit-redo = Redo
menu-edit-cut = Cut
menu-edit-copy = Copy
menu-edit-paste = Paste
menu-edit-delete = Delete
menu-edit-split = Split at Playhead
menu-edit-select-all = Select All
menu-edit-deselect = Deselect All

# Menu Bar - View
menu-view = View
menu-view-zoom-in = Zoom In
menu-view-zoom-out = Zoom Out
menu-view-zoom-fit = Zoom to Fit
menu-view-reset-zoom = Reset Zoom
menu-view-toggle-mixer = Mixer
menu-view-toggle-spectrogram = Spectrogram
menu-view-toggle-meters = Level Meters
menu-view-toggle-history = History

# Menu Bar - Track
menu-track = Track
menu-track-add-audio = Add Audio Track
menu-track-duplicate = Duplicate Track
menu-track-delete = Delete Track
menu-track-mute-all = Mute All Tracks
menu-track-unmute-all = Unmute All Tracks

# Menu Bar - Help
menu-help = Help
menu-help-documentation = Documentation
menu-help-about = About steM.

# Transport Controls
transport-play = Play
transport-pause = Pause
transport-stop = Stop
transport-record = Record
transport-loop = Loop Playback
transport-timecode = Time
transport-samples = Samples
transport-tempo = Tempo (BPM)
transport-time-signature = Signature
transport-master-volume = Master Volume

# Track Controls & Headers
track-header-mute = M
track-header-solo = S
track-header-arm = R
track-header-volume = Gain
track-header-pan = Pan
track-header-effects = Effects
track-header-add-clip = Add Clip
track-default-name = Track { $number }
track-master-name = Master

# Clip Operations
clip-fade-in = Fade In
clip-fade-out = Fade Out
clip-gain = Clip Gain
clip-split = Split
clip-trim = Trim

# Mixer Panel
mixer-title = Mixer
mixer-track-label = Track
mixer-master-label = Master
mixer-fader = Fader
mixer-pan-center = Center
mixer-pan-left = L { $value }%
mixer-pan-right = R { $value }%
mixer-peak = Peak
mixer-rms = RMS
mixer-lufs = LUFS

# Audio DSP & Effects
effect-equalizer = Equalizer
effect-compressor = Compressor
effect-limiter = Limiter
effect-reverb = Reverb
effect-delay = Delay
effect-noise-reduction = Noise Reduction
effect-normalize = Normalize
effect-gain = Gain
effect-threshold = Threshold
effect-ratio = Ratio
effect-attack = Attack
effect-release = Release
effect-makeup-gain = Makeup Gain
effect-frequency = Frequency
effect-q-factor = Q Factor
effect-dry-wet = Dry / Wet

# Views & Displays
view-waveform = Waveform
view-spectrogram = Spectrogram
view-history-title = Edit History
view-no-history = No actions performed yet

# Dialogs & Settings
dialog-about-title = About steM.
dialog-about-description = steM. is a high-performance, non-destructive audio editor and multi-track mixing workstation.
dialog-about-license = License: GNU General Public License v3.0
dialog-about-built-with = Built with Rust, egui, cpal, symphonia, hound, flacenc, rubato, and realfft.
dialog-preferences-title = Preferences
dialog-preferences-language = Language
dialog-preferences-audio-device = Output Device
dialog-preferences-sample-rate = Sample Rate
dialog-preferences-buffer-size = Buffer Size
dialog-preferences-latency = Estimated Latency
dialog-close = Close
dialog-apply = Apply
dialog-cancel = Cancel

# Export Dialog
export-title = Export Audio
export-format = Format
export-format-wav = WAV (Lossless PCM)
export-format-flac = FLAC (Lossless Compressed)
export-format-mp3 = MP3 (MPEG Layer-3)
export-sample-rate = Sample Rate
export-bit-depth = Bit Depth
export-16bit = 16-bit Integer
export-24bit = 24-bit Integer
export-32bit-float = 32-bit Float
export-dither = TPDF Dither
export-dither-enabled = Enabled (Recommended for bit-depth reduction)
export-dither-disabled = Disabled
export-normalize-before = Normalize Before Export
export-target-lufs = Target Loudness (LUFS)
export-range = Range
export-range-entire = Entire Project
export-range-selection = Time Selection
export-btn-start = Export
export-btn-cancel = Cancel Export
export-btn-browse = Browse...
export-status-in-progress = Exporting audio...
export-status-success = Export completed successfully!
export-status-cancelled = Export cancelled by user.
export-status-error = Export failed: { $reason }

# Status Bar & Notifications
status-ready = Ready
status-playing = Playing
status-paused = Paused
status-recording = Recording
status-loading = Loading audio...
status-generating-peaks = Building waveform peak cache...
status-project-saved = Project saved successfully.
status-imported-file = Imported { $filename }

# Units & Formatting
unit-db = dB
unit-hz = Hz
unit-khz = kHz
unit-ms = ms
unit-sec = s
unit-samples = spl
unit-bpm = BPM

# Advanced Timecode & Grid
transport-mode-timecode = Time (MM:SS.mmm)
transport-mode-smpte = SMPTE (HH:MM:SS:FF)
transport-mode-bars = Bars & Beats (Bar.Beat.Tick)
transport-mode-samples = Samples (Absolute)

# Interactive Effects & Visualizers
effect-eq-title = Parametric Equalizer
effect-comp-title = Dynamics Compressor
effect-band-lowshelf = Low Shelf
effect-band-peaking = Peaking
effect-band-highshelf = High Shelf
effect-band-highpass = High Pass
effect-band-lowpass = Low Pass
effect-band-bypass = Bypass
track-add-effect = Add Effect...
track-effects-rack = Effects Rack

# Demo Session & Recording
menu-file-load-demo = Load Demo Session
status-demo-loaded = Loaded demo multi-track session!
status-recording-armed = Ready to record on armed tracks
status-recording-saved = Recording saved to new take clip
recording-input-device = Input Device
recording-armed-track = Armed for recording

# Waveform Editor Mode
view-mode-multitrack = Multitrack
view-mode-waveform = Waveform Editor
menu-file-open-waveform = Open in Waveform Editor...
waveform-title = Waveform Editor
waveform-no-file = No audio file loaded in Waveform Editor. Open an audio file or double-click a clip in the Multitrack timeline.
waveform-selection-start = Start: { $time }
waveform-selection-end = End: { $time }
waveform-selection-duration = Duration: { $time }
waveform-snap-zero-crossing = Snap to Zero-Crossing
waveform-loop-selection = Loop Selection
waveform-play-selection = Play Selection
waveform-play-all = Play All
waveform-btn-cut = Cut
waveform-btn-copy = Copy
waveform-btn-paste = Paste
waveform-btn-delete = Delete
waveform-btn-trim = Trim
waveform-btn-silence = + Silence
waveform-btn-reverse = Reverse
waveform-btn-invert = Invert Phase
waveform-btn-apply-to-project = Apply to Project
waveform-status-cut = Cut selection to clipboard
waveform-status-copied = Copied selection to clipboard
waveform-status-pasted = Pasted from clipboard
waveform-status-deleted = Deleted selection
waveform-status-trimmed = Trimmed to selection
waveform-status-silence-inserted = Inserted 1 second of silence
waveform-status-reversed = Reversed audio range
waveform-status-phase-inverted = Inverted audio phase
waveform-status-applied = Applied waveform changes to project clip

# Phase B: Effects Rack & Audio Operations
effect-btn-apply = Apply to Selection
effect-btn-preview = Live Preview
effect-btn-bypass = Bypass
effect-btn-reset = Reset
effect-btn-move-up = Move Up
effect-btn-move-down = Move Down
effect-limiter-title = Brickwall Limiter
effect-delay-title = Stereo Delay
effect-reverb-title = Algorithmic Reverb
effect-target-master = Master Output
effect-target-track = Track { $index }
effect-target-waveform = Waveform Preview
effect-feedback = Feedback
effect-mix = Dry/Wet Mix
effect-delay-time = Delay Time
effect-room-size = Room Size
effect-damping = Damping
waveform-btn-fade-in = Fade In
waveform-btn-fade-out = Fade Out
waveform-btn-normalize = Normalize...
waveform-fade-linear = Linear
waveform-fade-log = Logarithmic
waveform-fade-exp = Exponential
waveform-fade-scurve = S-Curve
dialog-normalize-title = Peak Normalize
dialog-normalize-target = Target Peak (dBFS):
dialog-normalize-btn-apply = Apply
status-effect-applied = Insert effect applied to audio selection
status-fade-applied = Fade curve applied to audio selection
status-normalized = Audio peak normalized successfully

# Phase C: Cleanup, Restoration & Analysis
menu-cleanup = Cleanup
menu-cleanup-capture-noise = Capture Noise Profile from Selection
menu-cleanup-noise-reduction = Noise Reduction (Spectral Gate)...
menu-cleanup-click-remover = Click & Pop Remover...
cleanup-noise-reduction = Noise Reduction
cleanup-capture-noise-profile = Capture Noise Profile
cleanup-open-noise-dialog = Noise Reduction Settings...
cleanup-reduction-amount = Reduction Amount (dB)
cleanup-sensitivity = Sensitivity
cleanup-profile-status = Profile Status
cleanup-profile-captured = Captured ({ $bins } bins, { $frames } frames)
cleanup-profile-none = No noise profile captured yet
cleanup-btn-capture = Capture from Selection
cleanup-btn-apply = Apply to Selection
cleanup-btn-apply-all = Apply to Entire File
cleanup-status-applied = Noise reduction applied
cleanup-click-remover = Click & Pop Remover
cleanup-click-sensitivity = Sensitivity
cleanup-click-max-width = Max Click Duration (samples)
cleanup-click-btn-repair = Detect & Repair Clicks
cleanup-click-result = Repaired { $count } clicks/pops
cleanup-click-none = No clicks detected in selection

menu-analysis = Analysis
menu-analysis-loudness = Loudness Report (EBU R128)...
menu-analysis-spectrogram = Toggle Spectrogram Panel
analysis-loudness-report = Loudness Report
analysis-loudness-title = EBU R128 Loudness Report
analysis-target-standard = Target Standard
analysis-integrated-lufs = Integrated Loudness
analysis-momentary-max = Max Momentary
analysis-short-term-max = Max Short-Term
analysis-loudness-range = Loudness Range (LRA)
analysis-true-peak = True Peak
analysis-target-diff = Target Difference
analysis-btn-reanalyze = Re-analyze Audio
analysis-compliant = COMPLIANT
analysis-non-compliant = NON-COMPLIANT

spectrogram-fft-size = FFT Size
spectrogram-palette = Color Scale
spectrogram-mode = Display Mode
spectrogram-mode-both = Both (Split)
spectrogram-mode-waterfall = Spectrogram (2D)
spectrogram-mode-curve = Spectrum (1D)
meter-peak = Peak
meter-rms = RMS

# Phase D: Recording Completeness
recording-input-level = Input Level
recording-monitoring = Input Monitoring
recording-monitoring-enabled = Software input monitoring enabled
recording-monitoring-disabled = Software input monitoring disabled
recording-count-in = Count-in
recording-count-in-off = Off
recording-count-in-1bar = 1 Bar
recording-count-in-2bars = 2 Bars
recording-count-in-2s = 2 Seconds
recording-count-in-4s = 4 Seconds
recording-count-in-active = Count-in in progress...
recording-punch-in = Punch-In Recording
recording-punch-enabled = Punch-In Enabled
recording-punch-region = Punch Region
recording-punch-set-selection = Set Punch from Selection
recording-punch-clear = Clear Punch Region
recording-punch-preroll = Preroll playback (waiting for punch-in)...
recording-latency-compensation = Latency Compensation
recording-latency-ms = Latency Offset (ms)
recording-latency-samples = { $samples } samples
recording-device-lost = Audio input device disconnected! Take safely preserved.
recording-device-connected = Audio input device connected
recording-no-device = No audio input device available to record
recording-device-warning-title = Audio Device Disconnected
recording-device-warning-msg = The audio recording device was disconnected. Your current recording take was safely preserved. Please reconnect the device or select an alternate device in Preferences.
recording-input-routing = Input Routing
recording-input-mono1 = Mono (In 1)
recording-input-mono2 = Mono (In 2)
recording-input-stereo = Stereo (1+2)
recording-input-none = No Input

# Phase E: Multitrack Completeness (Busses, Groups, Automation, Crossfades)
mixer-busses = Submix Busses
bus-title = Submix Bus
bus-add = Add Bus
bus-remove = Remove Bus
bus-output = Bus Output
bus-send = Aux Send
bus-send-add = Add Send...
bus-send-pre = Pre-Fader
bus-send-post = Post-Fader
routing-master = Master Output
routing-bus = Bus: { $name }
group-title = Track Group
group-none = No Group
group-add = New Group...
group-link-gain = Link Volume
group-link-pan = Link Pan
group-link-mute = Link Mute
group-link-solo = Link Solo
group-link-arm = Link Record Arm
automation-lane = Automation Lane
automation-toggle = Toggle Automation
automation-param = Parameter
automation-volume = Volume (dB)
automation-pan = Pan
automation-curve = Curve
automation-curve-linear = Linear
automation-curve-bezier = Bezier (Smooth)
automation-curve-step = Step
automation-point-add = Click to add breakpoint
automation-point-delete = Delete point
crossfade-title = Crossfade
crossfade-equal-power = Equal-Power (3dB Sum)
crossfade-linear = Linear
crossfade-exponential = Exponential

# Phase F: Workflow Polish (Markers, Recovery, Keybindings, Batch Export)
marker-title = Marker
marker-add = Add Marker
marker-add-named = Add Named Marker...
marker-edit = Edit Marker
marker-list = Markers...
marker-name = Marker Name
marker-frame = Frame
marker-time = Time
marker-cd-track = CD Track Cue Point
marker-prev = Previous Marker
marker-next = Next Marker
marker-delete = Delete Marker
marker-jump = Jump to Marker

dialog-marker-title = Marker
dialog-marker-list-title = Markers
dialog-batch-export-title = Batch Export
batch-export-stems = Export Track Stems
batch-export-regions = Export Marker Regions
batch-export-pattern = Filename Pattern:
batch-export-pattern-help = Tokens: {"{"}project{"}"}, {"{"}track{"}"}, {"{"}index{"}"}, {"{"}marker{"}"}
batch-export-output-dir = Output Directory:
batch-export-browse = Browse...
batch-export-format = Format:
batch-export-bit-depth = Bit Depth:
batch-export-btn = Start Batch Export
batch-export-completed = Batch export completed successfully

dialog-keybindings-title = Keyboard Shortcuts
keybindings-filter = Search shortcuts:
keybindings-action = Action
keybindings-shortcut = Shortcut
keybindings-conflicts-warning = Conflict detected: multiple actions share the same shortcut!
keybindings-reset = Reset to Defaults
keybindings-export = Export JSON...
keybindings-import = Import JSON...

dialog-recovery-title = Session Crash Recovery
recovery-prompt = An unsaved project session was recovered ({ $time }). Would you like to restore it?
recovery-restore = Restore Session
recovery-discard = Discard
recovery-restored = Project session restored from crash recovery
recovery-discarded = Crash recovery journal discarded



