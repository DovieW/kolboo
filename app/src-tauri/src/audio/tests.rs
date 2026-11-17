use super::*;
use rodio::mixer::mixer;

fn new_request() -> (Request, Receiver<CueResult>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (
        Request {
            sound: SoundType::RecordingStart,
            cue: AudioCue::Kolboo,
            expires_at: Instant::now() + CALLER_LIMIT,
            completion: Some(sender),
        },
        receiver,
    )
}

fn new_pending(now: Instant) -> (Pending, rodio::mixer::MixerSource, Receiver<CueResult>) {
    let (mixer, samples) = mixer(NonZeroU16::new(1).unwrap(), NonZeroU32::new(44100).unwrap());
    let player = Player::connect_new(&mixer);
    player.append(SamplesBuffer::new(
        NonZeroU16::new(1).unwrap(),
        NonZeroU32::new(44100).unwrap(),
        vec![0.25; 8],
    ));
    let (completion, receiver) = {
        let (sender, receiver) = mpsc::sync_channel(1);
        (Some(sender), receiver)
    };
    (
        Pending {
            player,
            deadline: now + PLAYBACK_LIMIT,
            tail_until: None,
            completion,
        },
        samples,
        receiver,
    )
}

#[test]
fn cues_preserve_themes_assets_and_expected_synth_durations() {
    assert_eq!(
        output_device_error(rodio::DeviceSinkError::NoDevice),
        "notification output is unavailable"
    );
    for cue in [
        AudioCue::Kolboo,
        AudioCue::Maraca,
        AudioCue::Clave,
        AudioCue::Legacy,
    ] {
        for sound in [SoundType::RecordingStart, SoundType::RecordingStop] {
            let source = cue_source(sound, cue).unwrap();
            assert!(source.count() > 0);
            assert!(!estimated_duration(sound, cue).is_zero());
            if cue != AudioCue::Legacy {
                let (source, duration) = build_synth_cue_source(sound, cue);
                assert_eq!(duration, estimated_duration(sound, cue));
                assert!(source
                    .clone()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
            }
        }
    }
    assert!(decode_legacy(b"not audio").is_err());
    assert!(
        build_synth_cue_source(SoundType::RecordingStart, AudioCue::Legacy)
            .1
            .is_zero()
    );
    for (name, cue) in [
        ("kolboo", AudioCue::Kolboo),
        ("maraca", AudioCue::Maraca),
        ("clave", AudioCue::Clave),
        ("legacy", AudioCue::Legacy),
        ("unknown", AudioCue::Kolboo),
    ] {
        assert_eq!(AudioCue::from_str(name), cue);
    }
}

#[test]
fn output_callbacks_latch_without_io_and_completion_waits_for_actual_samples_and_tail() {
    let failed = Arc::new(AtomicBool::new(false));
    let mut error = output_error_handler(failed.clone());
    for _ in 0..1_000_000 {
        error(cpal::StreamError::DeviceNotAvailable);
    }
    assert!(failed.load(Ordering::Acquire));
    let now = Instant::now();
    let (mut pending, mut samples, receiver) = new_pending(now);
    assert!(!pending.completed(now + Duration::from_secs(1)));
    assert!(
        pending.tail_until.is_none(),
        "elapsed estimates cannot finish unconsumed audio"
    );
    for _ in 0..1024 {
        let _ = samples.next();
    }
    assert!(pending.player.empty());
    assert!(!pending.completed(now));
    assert!(!pending.completed(now + TAIL_PAD - Duration::from_nanos(1)));
    assert!(pending.completed(now + TAIL_PAD));
    finish(pending.completion.take(), Ok(()));
    assert_eq!(receiver.recv().unwrap(), Ok(()));
    finish(None, Err(OUTPUT_FAILED));
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    finish(Some(sender), Ok(()));
}

#[test]
fn queue_and_blocking_waits_have_fixed_bounds_and_closed_failures() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (one, _) = new_request();
    submit(&sender, one).unwrap();
    let (two, _) = new_request();
    assert_eq!(
        submit(&sender, two),
        Err("notification playback queue is full")
    );
    drop(receiver);
    let (three, _) = new_request();
    assert_eq!(
        submit(&sender, three),
        Err("notification playback worker stopped")
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    assert!(wait_for_completion(receiver, Duration::ZERO).is_err());
    drop(sender);
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(sender);
    assert!(wait_for_completion(receiver, Duration::ZERO).is_err());
    let (sender, receiver) = mpsc::sync_channel(1);
    sender.send(Err(OUTPUT_FAILED)).unwrap();
    assert_eq!(
        wait_for_completion(receiver, Duration::ZERO),
        Err(OUTPUT_FAILED)
    );

    // Exercise the blocking API's submit-failure path, not merely the channel
    // helper: a rejected caller must return without waiting for completion.
    let (sender, receiver) = mpsc::sync_channel(1);
    let (queued, queued_completion) = new_request();
    sender.send(queued).ok().unwrap();
    assert_eq!(
        play_with_sender(&sender, SoundType::RecordingStart, AudioCue::Kolboo, true),
        Err("notification playback queue is full")
    );
    let queued = receiver.try_recv().unwrap();
    assert!(
        receiver.try_recv().is_err(),
        "failed submission must not leave a cue behind"
    );
    queued.finish(Ok(()));
    assert_eq!(queued_completion.recv().unwrap(), Ok(()));
    drop(receiver);
    assert_eq!(
        play_with_sender(&sender, SoundType::RecordingStop, AudioCue::Legacy, true),
        Err("notification playback worker stopped")
    );
}

#[test]
fn owner_completion_capacity_deadlines_and_rejected_sources_are_deterministic() {
    let now = Instant::now();
    let mut owner = Owner::default();
    assert!(owner.report_failure(OUTPUT_FAILED));
    for _ in 0..10_000 {
        assert!(
            !owner.report_failure(OUTPUT_FAILED),
            "fresh failures must not create a new warning episode before recovery"
        );
    }
    let (mut request, receiver) = new_request();
    request.expires_at = now;
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStart, AudioCue::Kolboo),
        &mut || panic!("expired cues must not open output"),
    );
    assert_eq!(receiver.recv().unwrap(), Err(REQUEST_EXPIRED));
    let (request, receiver) = new_request();
    owner.start(request, now, decode_legacy(b"not audio"), &mut || {
        panic!("invalid audio must not open output")
    });
    assert_eq!(
        receiver.recv().unwrap(),
        Err("notification audio could not be decoded")
    );
    let (request, receiver) = new_request();
    let source = cue_source(request.sound, request.cue);
    owner.start(request, now, source, &mut || {
        Err("synthetic output unavailable")
    });
    assert_eq!(
        receiver.recv().unwrap(),
        Err("synthetic output unavailable")
    );
    let (pending, mut samples, receiver) = new_pending(now);
    owner.pending.push(pending);
    owner.poll(now).unwrap();
    assert!(receiver.try_recv().is_err());
    for _ in 0..1024 {
        let _ = samples.next();
    }
    owner.poll(now).unwrap();
    assert!(receiver.try_recv().is_err());
    owner.poll(now + TAIL_PAD).unwrap();
    assert_eq!(receiver.recv().unwrap(), Ok(()));
    assert!(owner.pending.is_empty());
    assert!(!owner.failure_reported);
    assert!(
        owner.report_failure(OUTPUT_STALLED),
        "actual completed playback re-arms one report for a later failure"
    );
    let (pending, _, receiver) = new_pending(now);
    owner.pending.push(pending);
    assert_eq!(owner.poll(now + PLAYBACK_LIMIT), Err(OUTPUT_STALLED));
    owner.stop(OUTPUT_STALLED);
    assert_eq!(receiver.recv().unwrap(), Err(OUTPUT_STALLED));
    for _ in 0..CAPACITY {
        owner.pending.push(new_pending(now).0);
    }
    let (request, receiver) = new_request();
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStop, AudioCue::Clave),
        &mut || panic!("a full owner must not open output"),
    );
    assert_eq!(
        receiver.recv().unwrap(),
        Err("too many notification cues are already playing")
    );
    owner.stop(OUTPUT_FAILED);
    assert!(owner.pending.is_empty());
}

#[test]
fn worker_reports_open_failure_and_exits_when_request_channel_closes() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (request, done) = new_request();
    sender.send(request).ok().unwrap();
    drop(sender);
    run_worker(receiver, &mut || Err("synthetic output unavailable"));
    assert_eq!(done.recv().unwrap(), Err("synthetic output unavailable"));
}

#[test]
fn failed_worker_spawn_is_finite_cached_and_content_safe() {
    fn fail_spawn(_receiver: Receiver<Request>) -> std::io::Result<()> {
        Err(std::io::Error::other(
            "synthetic sensitive native spawn details",
        ))
    }
    fn forbidden_retry(_receiver: Receiver<Request>) -> std::io::Result<()> {
        panic!("a cached spawn failure must not retry or create more threads")
    }
    let cell = OnceLock::new();
    assert!(matches!(
        worker_sender(&cell, fail_spawn),
        Err("notification playback worker could not start")
    ));
    assert!(matches!(
        worker_sender(&cell, forbidden_retry),
        Err("notification playback worker could not start")
    ));
    assert!(matches!(
        cell.get(),
        Some(Err("notification playback worker could not start"))
    ));
}

#[cfg(target_os = "linux")]
pub(crate) fn native_output() {
    let directory = std::env::var("KOLBOO_NATIVE_WINDOW_DIRECTORY").unwrap();
    assert_eq!(
        std::env::var("ALSA_CONFIG_PATH").unwrap(),
        format!("{directory}/asound.conf")
    );
    for cue in [
        AudioCue::Kolboo,
        AudioCue::Maraca,
        AudioCue::Clave,
        AudioCue::Legacy,
    ] {
        super::super::play_sound_blocking(SoundType::RecordingStart, cue).unwrap();
        super::super::play_sound(SoundType::RecordingStop, cue);
    }
    let now = Instant::now();
    let mut owner = Owner::default();
    let (mut request, done) = new_request();
    request.expires_at = now - Duration::from_millis(1);
    owner.start(
        request,
        now - Duration::from_secs(1),
        cue_source(SoundType::RecordingStart, AudioCue::Kolboo),
        &mut Output::open,
    );
    assert_eq!(done.recv().unwrap(), Err(REQUEST_EXPIRED));
    assert!(
        owner.pending.is_empty(),
        "a cue expiring during native open must never append audio"
    );
    owner.stop(REQUEST_EXPIRED);
    let (request, done) = new_request();
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStart, AudioCue::Kolboo),
        &mut || {
            let output = Output::open()?;
            output_error_handler(output.failed.clone())(cpal::StreamError::DeviceNotAvailable);
            Ok(output)
        },
    );
    assert!(
        owner.pending.is_empty(),
        "a fault reported during open must not receive a cue"
    );
    assert_eq!(done.recv().unwrap(), Err(OUTPUT_FAILED));
    assert_eq!(owner.poll(now), Err(OUTPUT_FAILED));
    owner.stop(OUTPUT_FAILED);
    // A delayed native open gets a fresh playback budget, still capped by the
    // caller's request deadline. The injected old origin avoids timing sleeps.
    let before_open = Instant::now();
    let (request, done) = new_request();
    owner.start(
        request,
        before_open - PLAYBACK_LIMIT,
        cue_source(SoundType::RecordingStart, AudioCue::Clave),
        &mut Output::open,
    );
    assert!(owner.pending[0].deadline >= before_open + PLAYBACK_LIMIT);
    assert!(owner.poll(before_open).is_ok());
    owner.stop(OUTPUT_FAILED);
    assert_eq!(done.recv().unwrap(), Err(OUTPUT_FAILED));
    let (request, done) = new_request();
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStart, AudioCue::Kolboo),
        &mut Output::open,
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.pending[0].player.empty() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    owner.poll(now).unwrap();
    assert!(done.try_recv().is_err());
    owner.poll(now + TAIL_PAD).unwrap();
    assert_eq!(done.recv().unwrap(), Ok(()));
    assert!(
        owner.output.is_some(),
        "tail completion must preserve short output grace"
    );
    let retained_output = owner.output.as_ref().unwrap().failed.clone();
    let reuse_at = now + TAIL_PAD + IDLE_GRACE / 2;
    let (request, done) = new_request();
    owner.start(
        request,
        reuse_at,
        cue_source(SoundType::RecordingStop, AudioCue::Clave),
        &mut || panic!("rapid cues must reuse the existing native output"),
    );
    assert!(Arc::ptr_eq(
        &retained_output,
        &owner.output.as_ref().unwrap().failed
    ));
    let deadline = Instant::now() + Duration::from_secs(2);
    while !owner.pending[0].player.empty() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    owner.poll(reuse_at).unwrap();
    owner.poll(reuse_at + TAIL_PAD).unwrap();
    assert_eq!(done.recv().unwrap(), Ok(()));
    owner.poll(reuse_at + TAIL_PAD + IDLE_GRACE).unwrap();
    assert!(owner.output.is_none());
    let (request, done) = new_request();
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStop, AudioCue::Legacy),
        &mut Output::open,
    );
    let failed = owner.output.as_ref().unwrap().failed.clone();
    let mut error = output_error_handler(failed);
    for _ in 0..100_000 {
        error(cpal::StreamError::DeviceNotAvailable);
    }
    assert_eq!(owner.poll(now), Err(OUTPUT_FAILED));
    owner.stop(OUTPUT_FAILED);
    assert!(owner.output.is_none());
    assert_eq!(done.recv().unwrap(), Err(OUTPUT_FAILED));
    // Faults are local to the retired output; a new explicit cue opens cleanly.
    let (request, done) = new_request();
    owner.start(
        request,
        now,
        cue_source(SoundType::RecordingStart, AudioCue::Clave),
        &mut Output::open,
    );
    assert!(!owner
        .output
        .as_ref()
        .unwrap()
        .failed
        .load(Ordering::Acquire));
    owner.stop(OUTPUT_FAILED);
    assert_eq!(done.recv().unwrap(), Err(OUTPUT_FAILED));
    // All requests already queued when a device fails are refused, not replayed
    // on a silently substituted output. A later explicit call can reopen.
    let (sender, receiver) = mpsc::sync_channel(CAPACITY);
    let (request, completion) = new_request();
    sender.send(request).ok().unwrap();
    let mut completions = vec![completion];
    let mut sender = Some(sender);
    let mut opens = 0;
    run_worker(receiver, &mut || {
        opens += 1;
        let output = Output::open().expect("isolated null output must open");
        // Fill the actual worker queue while its native-open boundary is still
        // owned here. No timing race can let these requests be accepted first.
        let sender = sender.take().expect("failed output must not reopen");
        for _ in 0..CAPACITY {
            let (request, completion) = new_request();
            sender.send(request).ok().unwrap();
            completions.push(completion);
        }
        drop(sender);
        output_error_handler(output.failed.clone())(cpal::StreamError::DeviceNotAvailable);
        Ok(output)
    });
    assert_eq!(opens, 1);
    assert_eq!(completions.len(), CAPACITY + 1);
    for completion in completions {
        assert_eq!(completion.recv().unwrap(), Err(OUTPUT_FAILED));
    }
}
