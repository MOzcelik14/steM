use rtrb::RingBuffer;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use stem_audio::callback::{
    AudioCommand, AudioProcessor, AudioTelemetry, RealtimeAudioCallback, SharedAudioTimeline,
};
use stem_audio::record::{InputTelemetry, RealtimeRecordCallback};

struct CountingAllocator;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static DEALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static TRACK_ACTIVE: AtomicBool = AtomicBool::new(false);
static TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ACTIVE.load(Ordering::SeqCst) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACK_ACTIVE.load(Ordering::SeqCst) {
            DEALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK_ACTIVE.load(Ordering::SeqCst) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn with_alloc_guard<F: FnOnce() -> R, R>(f: F) -> (R, usize, usize) {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    DEALLOC_COUNT.store(0, Ordering::SeqCst);
    TRACK_ACTIVE.store(true, Ordering::SeqCst);
    let res = f();
    TRACK_ACTIVE.store(false, Ordering::SeqCst);
    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);
    let deallocs = DEALLOC_COUNT.load(Ordering::SeqCst);
    (res, allocs, deallocs)
}

fn create_test_timeline(frames: usize) -> Arc<SharedAudioTimeline> {
    Arc::new(SharedAudioTimeline {
        channels: vec![vec![0.5f32; frames], vec![0.5f32; frames]],
        track_channels: vec![
            vec![vec![0.25f32; frames], vec![0.25f32; frames]],
            vec![vec![0.1f32; frames], vec![0.1f32; frames]],
        ],
        total_frames: frames as u64,
    })
}

#[test]
fn test_audio_realtime_threads_zero_alloc_proof() {
    run_audio_output_thread_zero_alloc_test();
    run_audio_record_thread_zero_alloc_test();
}

fn run_audio_output_thread_zero_alloc_test() {
    let _lock = TEST_LOCK.lock().unwrap();

    // 1. Preallocate all resources
    let (mut cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(256);
    let (telem_tx, mut telem_rx) = RingBuffer::<AudioTelemetry>::new(256);
    let (mon_tx, mon_rx) = RingBuffer::<f32>::new(1024);
    let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);
    let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);

    let processor = AudioProcessor::new(cmd_rx, telem_tx, Some(mon_rx));
    let mut callback = RealtimeAudioCallback::new(processor, timeline_rx, retired_tx.clone());

    let mut out_buffer = vec![0.0f32; 512];
    let initial_timeline = create_test_timeline(48000);

    // Initial timeline swap before tracking
    timeline_tx.send(initial_timeline).unwrap();
    cmd_tx.push(AudioCommand::Play).unwrap();

    // Run first callback to establish steady state
    callback.process(&mut out_buffer);

    // 2. Test steady-state playback: must have ZERO alloc and ZERO dealloc
    let (_, allocs, deallocs) = with_alloc_guard(|| {
        for _ in 0..100 {
            callback.process(&mut out_buffer);
        }
    });
    assert_eq!(allocs, 0, "Audio thread must not allocate in steady-state playback");
    assert_eq!(deallocs, 0, "Audio thread must not deallocate in steady-state playback");

    // 3. Test timeline swap under active playback: must have ZERO alloc and ZERO dealloc
    let new_timeline = create_test_timeline(48000);
    timeline_tx.send(new_timeline).unwrap();

    let (_, allocs, deallocs) = with_alloc_guard(|| {
        callback.process(&mut out_buffer);
    });
    assert_eq!(allocs, 0, "Audio thread must not allocate during timeline swap");
    assert_eq!(deallocs, 0, "Audio thread must not deallocate during timeline swap");

    // Verify retired timeline was moved to retired_rx without dropping on audio thread
    let retired = retired_rx.try_recv().expect("Old timeline must be returned to UI channel");
    assert_eq!(retired.total_frames, 48000);

    // 4. Test command burst: push multiple diverse commands into ringbuffer
    for _ in 0..10 {
        cmd_tx.push(AudioCommand::SetMasterGain(0.8)).unwrap();
        cmd_tx.push(AudioCommand::SetTrackGain { track_idx: 0, gain: 0.5 }).unwrap();
        cmd_tx.push(AudioCommand::SetTrackPan { track_idx: 0, pan: -0.2 }).unwrap();
        cmd_tx.push(AudioCommand::SetTrackMute { track_idx: 1, muted: true }).unwrap();
        cmd_tx.push(AudioCommand::SetEffectBypass { track_idx: Some(0), effect_idx: 0, bypassed: true }).unwrap();
        cmd_tx.push(AudioCommand::SetEffectParam { track_idx: Some(0), effect_idx: 0, param_id: 1, value: 0.5 }).unwrap();
        cmd_tx.push(AudioCommand::Seek(1000)).unwrap();
    }

    let (_, allocs, deallocs) = with_alloc_guard(|| {
        callback.process(&mut out_buffer);
    });
    assert_eq!(allocs, 0, "Audio thread must not allocate during command bursts");
    assert_eq!(deallocs, 0, "Audio thread must not deallocate during command bursts");

    // 5. Test queue-full conditions: fill telemetry and retired queue to capacity
    // Fill retired queue completely (capacity 64)
    for _ in 0..64 {
        let dummy = create_test_timeline(100);
        retired_tx.try_send(dummy).unwrap();
    }
    // Now push another timeline to timeline_tx
    let overflow_timeline = create_test_timeline(96000);
    timeline_tx.send(overflow_timeline).unwrap();

    // Call process: retired_tx is full, so callback must stash without deallocating or allocating
    let (_, allocs, deallocs) = with_alloc_guard(|| {
        for _ in 0..10 {
            callback.process(&mut out_buffer);
        }
    });
    assert_eq!(allocs, 0, "Audio thread must not allocate when retired queue is full");
    assert_eq!(deallocs, 0, "Audio thread must not deallocate when retired queue is full");

    // Drain retired_rx to simulate UI recovery
    while retired_rx.try_recv().is_ok() {}

    // Call process again: callback should flush stashed retired timeline
    let (_, allocs, deallocs) = with_alloc_guard(|| {
        callback.process(&mut out_buffer);
    });
    assert_eq!(allocs, 0, "Audio thread must not allocate during recovery flush");
    assert_eq!(deallocs, 0, "Audio thread must not deallocate during recovery flush");

    // Clean up
    while telem_rx.pop().is_ok() {}
    drop(mon_tx);
}

fn run_audio_record_thread_zero_alloc_test() {
    let _lock = TEST_LOCK.lock().unwrap();

    let (record_tx, mut record_rx) = RingBuffer::<f32>::new(1024);
    let (telem_tx, mut telem_rx) = RingBuffer::<InputTelemetry>::new(128);
    let (mon_tx, mut mon_rx) = RingBuffer::<f32>::new(1024);

    let is_recording = Arc::new(AtomicBool::new(true));
    let monitoring_enabled = Arc::new(AtomicBool::new(true));

    let mut record_callback = RealtimeRecordCallback::new(
        2,
        record_tx,
        telem_tx,
        Some(mon_tx),
        is_recording.clone(),
        monitoring_enabled.clone(),
    );

    let input_samples = vec![0.1f32; 256];

    // Steady state input processing
    let (_, allocs, deallocs) = with_alloc_guard(|| {
        record_callback.process(&input_samples);
    });
    assert_eq!(allocs, 0, "Record callback must not allocate during recording");
    assert_eq!(deallocs, 0, "Record callback must not deallocate during recording");

    // Full buffer saturation condition: drain nothing, fill buffers to maximum
    let (_, allocs, deallocs) = with_alloc_guard(|| {
        for _ in 0..50 {
            record_callback.process(&input_samples);
        }
    });
    assert_eq!(allocs, 0, "Record callback must not allocate when ringbuffers are saturated");
    assert_eq!(deallocs, 0, "Record callback must not deallocate when ringbuffers are saturated");

    // Clean up
    while record_rx.pop().is_ok() {}
    while telem_rx.pop().is_ok() {}
    while mon_rx.pop().is_ok() {}
}
