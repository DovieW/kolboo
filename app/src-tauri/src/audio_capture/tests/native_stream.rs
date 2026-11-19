use super::*;

#[test]
fn first_audio_wakes_startup_and_stop_has_priority() {
    let (stop_tx, stop_rx) = crossbeam_channel::bounded(1);
    let (audio_tx, audio_rx) = crossbeam_channel::bounded(1);
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    audio_tx.send(()).unwrap();
    assert!(wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Ok(())).unwrap());
    audio_tx.send(()).unwrap();
    assert!(matches!(
        wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Err(
            AudioCaptureError::StreamInterrupted
        )),
        Err(AudioCaptureError::StreamInterrupted)
    ));
    audio_tx.send(()).unwrap();
    stop_tx.send(CaptureCommand::Stop).unwrap();
    assert!(!wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Ok(())).unwrap());
    assert_eq!(audio_rx.try_recv(), Ok(()));
    drop(stop_tx);
    assert!(!wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Ok(())).unwrap());
}

#[test]
fn startup_still_detects_deadlines_worker_loss_and_native_faults() {
    let (_stop_tx, stop_rx) = crossbeam_channel::bounded(1);
    let (audio_tx, audio_rx) = crossbeam_channel::bounded(1);
    assert!(matches!(
        wait_for_first_audio(&stop_rx, &audio_rx, Instant::now(), &|| Ok(())),
        Err(AudioCaptureError::StreamStart(_))
    ));
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    assert!(matches!(
        wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Err(
            AudioCaptureError::StreamInterrupted
        )),
        Err(AudioCaptureError::StreamInterrupted)
    ));
    drop(audio_tx);
    assert!(matches!(
        wait_for_first_audio(&stop_rx, &audio_rx, deadline, &|| Ok(())),
        Err(AudioCaptureError::StreamInterrupted)
    ));
}

fn dispatch() -> CaptureDispatch {
    CaptureDispatch {
        consumer: std::thread::current(),
        queue: Arc::new(ArrayQueue::new(1)),
        pool: Arc::new(ArrayQueue::new(1)),
        fence: Arc::new(CaptureFence::default()),
    }
}

#[test]
fn native_error_flood_is_latched_once_without_poisoning_a_fresh_stream() {
    assert_eq!(
        AudioCaptureError::StreamInterrupted.to_string(),
        "Microphone stream failed; the recording is retained. Try recording again."
    );
    for recording in [false, true] {
        let dispatch = dispatch();
        let epoch = if recording { dispatch.fence.begin() } else { 0 };
        let fault = Arc::new(NativeStreamFault::default());
        assert!(fault.check().is_ok());
        let mut on_error = stream_error_handler(dispatch.clone(), fault.clone());
        for _ in 0..1_000_000 {
            on_error(cpal::StreamError::DeviceNotAvailable);
        }
        assert_eq!(fault.0.load(Ordering::SeqCst), 1);
        assert_eq!(dispatch.fence.failed(epoch), recording);
        assert!(!dispatch.fence.stream_failed());
        assert!(matches!(
            fault.check(),
            Err(AudioCaptureError::StreamInterrupted)
        ));
        assert_eq!(fault.0.load(Ordering::SeqCst), 2);
        assert!(matches!(
            fault.check(),
            Err(AudioCaptureError::StreamInterrupted)
        ));
        dispatch.fence.stop();
        let next = dispatch.fence.begin();
        // A retired native source must not re-fail or report against a later
        // recording, even if the backend keeps invoking its old error callback.
        on_error(cpal::StreamError::StreamInvalidated);
        assert!(!dispatch.fence.failed(next));
        assert_eq!(fault.0.load(Ordering::SeqCst), 2);
        let fresh = Arc::new(NativeStreamFault::default());
        assert!(fresh.check().is_ok());
        stream_error_handler(dispatch.clone(), fresh)(cpal::StreamError::DeviceNotAvailable);
        assert!(dispatch.fence.failed(next));
    }
    let native = cpal::BuildStreamError::StreamConfigNotSupported;
    assert!(
        matches!(stream_build_error(native.clone()), AudioCaptureError::StreamBuild(message) if message == native.to_string())
    );
}

#[test]
fn a_joined_native_disconnect_preserves_journal_recovery_without_quarantining_capture() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("interrupted.pcm");
    let mut capture = AudioCapture::new();
    capture.set_recovery_path(Some(path.clone())).unwrap();
    capture.begin_recording_with_pre_roll(1.0);
    capture.buffer.lock().unwrap().append(&[0.25, -0.25]);
    let epoch = capture.buffer.lock().unwrap().recording_epoch;
    capture.capture_fence.fail_latest();
    let (command_tx, command_rx) = crossbeam_channel::bounded(1);
    let (_event_tx, event_rx) = mpsc::channel();
    capture.capture_handle = Some(CaptureHandle {
        command_tx,
        event_rx,
        thread_handle: thread::spawn(move || {
            command_rx.recv().unwrap();
            Err(AudioCaptureError::StreamInterrupted)
        }),
    });
    capture.stop_recording();
    assert!(capture.capture_handle.is_none());
    assert!(!capture.recovery_shutdown_failed);
    assert!(!capture.capture_fence.stream_failed());
    assert!(capture.capture_fence.failed(epoch));
    assert!(AudioCaptureBackend::recording_progress(&capture).1);
    capture.finish_recovery().unwrap();
    assert_eq!(journal::read_chunk(&path, 0, 10).unwrap().2, [0.25, -0.25]);
    // Explicit recovery can still encode the retained prefix. Automatic
    // transcription separately refuses the incomplete recording_progress flag.
    assert!(!capture.stop_and_get_wav().unwrap().is_empty());
    capture.set_recovery_path(None).unwrap();
    capture.begin_recording_with_pre_roll(1.0);
    assert!(!AudioCaptureBackend::recording_progress(&capture).1);
    assert_ne!(capture.buffer.lock().unwrap().recording_epoch, epoch);
    assert!(
        path.exists(),
        "a fresh recording must not discard retained audio"
    );
}

// Called only by the isolated ALSA-null fixture. It exercises the actual owner,
// all native sample callback formats, and stream drop without opening hardware.
#[cfg(target_os = "linux")]
pub(super) fn native_stream_errors() {
    for format in [SampleFormat::F32, SampleFormat::I16, SampleFormat::U16] {
        for recording in [false, true] {
            for auto_recover in [false, true] {
                let mut capture = AudioCapture::new();
                if recording {
                    capture.begin_recording_with_pre_roll(1.0);
                    capture.buffer.lock().unwrap().append(&[0.25]);
                }
                let (command_tx, command_rx) = crossbeam_channel::bounded(1);
                let (event_tx, event_rx) = mpsc::channel();
                let args = CaptureThreadArgs {
                    ready: None,
                    capture_fence: capture.capture_fence.clone(),
                    device: InputDevice::Cpal(cpal::default_host().default_input_device().unwrap()),
                    config: cpal::StreamConfig {
                        channels: 1,
                        sample_rate: 16000,
                        buffer_size: cpal::BufferSize::Fixed(64),
                    },
                    sample_format: format,
                    buffer: capture.buffer.clone(),
                    pre_roll: capture.pre_roll.clone(),
                    recording_active: capture.recording_active.clone(),
                    pre_roll_ms: capture.pre_roll_ms.clone(),
                    meter: capture.level_meter.clone(),
                    waveform_meter: capture.waveform_meter.clone(),
                    command_rx,
                    event_tx,
                    vad_config: VadAutoStopConfig::default(),
                    auto_recover_enabled: Arc::new(AtomicBool::new(auto_recover)),
                    desired_device_name: Arc::new(StdMutex::new(None)),
                };
                capture.capture_handle = Some(CaptureHandle {
                    command_tx,
                    event_rx,
                    thread_handle: thread::spawn(move || {
                        let result = run_capture_thread_with_builder(
                            args,
                            Instant::now(),
                            &mut |device, config, format, start, clock, dispatch| {
                                let stream =
                                    build_stream(device, config, format, start, clock, dispatch)?;
                                let InputStream::Cpal { fault, .. } = &stream else {
                                    panic!("the null fixture must build a native stream");
                                };
                                let mut error =
                                    stream_error_handler(dispatch.clone(), fault.clone());
                                for _ in 0..100_000 {
                                    error(cpal::StreamError::DeviceNotAvailable);
                                }
                                Ok(stream)
                            },
                        );
                        assert!(matches!(result, Err(AudioCaptureError::StreamInterrupted)));
                        result
                    }),
                });
                let deadline = Instant::now() + std::time::Duration::from_secs(2);
                while !capture
                    .capture_handle
                    .as_ref()
                    .unwrap()
                    .thread_handle
                    .is_finished()
                {
                    assert!(
                        Instant::now() < deadline,
                        "faulted stream owner must stop even while idle"
                    );
                    thread::yield_now();
                }
                capture.stop();
                assert!(!capture.recovery_shutdown_failed);
                assert!(!capture.capture_fence.stream_failed());
                assert_eq!(
                    AudioCaptureBackend::recording_progress(&capture).1,
                    recording
                );
                assert_eq!(
                    capture.buffer.lock().unwrap().captured_samples,
                    u64::from(recording)
                );
                // The normal recording path must reopen after the joined fault.
                capture.start_recording_session(1.0, None).unwrap();
                assert!(!capture.recovery_shutdown_failed);
                capture.stop();
            }
        }
    }
}
