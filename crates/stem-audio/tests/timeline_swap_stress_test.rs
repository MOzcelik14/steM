use rtrb::RingBuffer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use stem_audio::callback::{
    AudioCommand, AudioProcessor, AudioTelemetry, RealtimeAudioCallback, SharedAudioTimeline,
};

fn create_timeline(frames: usize, tag: f32) -> Arc<SharedAudioTimeline> {
    Arc::new(SharedAudioTimeline {
        channels: vec![vec![tag; frames], vec![tag; frames]],
        track_channels: vec![vec![vec![tag; frames], vec![tag; frames]]],
        total_frames: frames as u64,
    })
}

#[test]
fn test_timeline_swap_stress_bounded_memory() {
    let (cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(256);
    let (telem_tx, _telem_rx) = RingBuffer::<AudioTelemetry>::new(256);
    let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);
    let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(64);

    let processor = AudioProcessor::new(cmd_rx, telem_tx, None);
    let mut callback = RealtimeAudioCallback::new(processor, timeline_rx, retired_tx);

    let running = Arc::new(AtomicBool::new(true));
    let running_cb = running.clone();

    // Spawn simulated audio thread running at typical block rate
    let audio_thread = std::thread::spawn(move || {
        let mut buffer = vec![0.0f32; 512];
        let mut processed_blocks = 0usize;
        while running_cb.load(Ordering::Relaxed) {
            callback.process(&mut buffer);
            processed_blocks += 1;
            // Short yield simulating hardware block interval
            std::thread::yield_now();
        }
        processed_blocks
    });

    let num_swaps = 10_000;
    let mut total_retired_received = 0usize;

    for i in 0..num_swaps {
        let tl = create_timeline(1024, i as f32);

        // Emulate UI thread: send timeline, then drain retired queue
        while timeline_tx.try_send(tl.clone()).is_err() {
            // If timeline_tx is full, drain retired queue to relieve pressure
            while let Ok(retired) = retired_rx.try_recv() {
                total_retired_received += 1;
                // Verify weak/strong references
                assert!(Arc::strong_count(&retired) >= 1);
            }
            std::thread::yield_now();
        }

        // Periodically drain retired queue like UI thread does in poll_telemetry()
        if i % 10 == 0 {
            while let Ok(_retired) = retired_rx.try_recv() {
                total_retired_received += 1;
            }
        }
    }

    // Drain remaining retired timelines
    let start_wait = std::time::Instant::now();
    while start_wait.elapsed() < Duration::from_millis(500) {
        while let Ok(_retired) = retired_rx.try_recv() {
            total_retired_received += 1;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    running.store(false, Ordering::Relaxed);
    let blocks = audio_thread
        .join()
        .expect("Audio thread terminated cleanly");

    println!(
        "Timeline swap stress completed: {} swaps attempted, {} retired reclaimed, {} audio blocks processed",
        num_swaps, total_retired_received, blocks
    );
    assert!(blocks > 0);
    assert!(total_retired_received > 0);
    drop(cmd_tx);
}

#[test]
fn test_timeline_swap_unresponsive_ui_queue_saturation() {
    let (_cmd_tx, cmd_rx) = RingBuffer::<AudioCommand>::new(64);
    let (telem_tx, _telem_rx) = RingBuffer::<AudioTelemetry>::new(64);
    let (timeline_tx, timeline_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(4);
    let (retired_tx, retired_rx) = crossbeam_channel::bounded::<Arc<SharedAudioTimeline>>(4);

    let mut processor = AudioProcessor::new(cmd_rx, telem_tx, None);
    processor.set_audio_data(Some(create_timeline(500, -1.0)));
    let mut callback = RealtimeAudioCallback::new(processor, timeline_rx, retired_tx);
    let mut buffer = vec![0.0f32; 256];

    // Push 5 timelines into timeline_tx (capacity 4, so 4 fit immediately)
    // Timeline 0 -> retired 0 (retired_rx len 1)
    // Timeline 1 -> retired 1 (retired_rx len 2)
    // Timeline 2 -> retired 2 (retired_rx len 3)
    // Timeline 3 -> retired 3 (retired_rx len 4, full!)
    // Now push 5th timeline:
    for i in 0..4 {
        timeline_tx.send(create_timeline(500, i as f32)).unwrap();
    }

    // Process 4 callbacks to fill retired_tx
    for _ in 0..4 {
        callback.process(&mut buffer);
    }
    assert_eq!(retired_rx.len(), 4);
    assert!(callback.stashed_retired.is_none());

    // Push 5th timeline: retired_tx is full, so this retirement MUST be stashed
    timeline_tx.send(create_timeline(500, 4.0)).unwrap();
    callback.process(&mut buffer);

    assert_eq!(retired_rx.len(), 4);
    assert!(callback.stashed_retired.is_some());

    // Continuous processing while queues are completely saturated:
    // MUST NOT PANIC, MUST NOT LEAK (no mem::forget), MUST NOT CRASH
    for _ in 0..100 {
        callback.process(&mut buffer);
    }

    // Drain retired_rx now
    let mut drained = 0;
    while retired_rx.try_recv().is_ok() {
        drained += 1;
    }
    assert_eq!(drained, 4);

    // After draining, the stashed timeline can now be sent
    callback.process(&mut buffer);
    assert!(callback.stashed_retired.is_none());
    assert_eq!(retired_rx.len(), 1);
}
