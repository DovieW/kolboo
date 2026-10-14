use super::super::dispatch::CaptureFence;
use super::*;
use crossbeam_queue::ArrayQueue;
use serde_json::json;

#[test]
fn server_inputs_exclude_monitors_and_keep_distinct_same_name_microphones() {
    let payload = json!([
        {"name":"usb.input", "description":"Wireless microphone", "monitor_of_sink":null},
        {"name":"internal.analog", "description":"Internal microphone", "monitor_of_sink":4294967295u64},
        {"name":"internal.digital", "description":"Internal microphone"},
        {"name":"usb.input", "description":"Duplicate snapshot entry"},
        {"name":""},
        {"name":"speaker.monitor"},
        {"name":"output", "monitor_source":"speaker"},
        {"name":"old.monitor", "monitor_of_sink":0},
        {"name":"loop", "monitor_of_sink":3},
        {"name":"class.monitor", "properties":{"device.class":"monitor"}},
        {"name":"renamed-monitor", "properties":{"device.class":"monitor"}},
        {"name":"virtual.input", "description":" ", "properties":{"device.class":null}}
    ]);
    let devices = input_sources(&serde_json::to_vec(&payload).unwrap()).unwrap();
    assert_eq!(
        devices.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        [
            "Wireless microphone",
            "Internal microphone",
            "Internal microphone",
            "virtual.input"
        ]
    );
    assert_eq!(source_name(&devices[0].id).as_deref(), Some("usb.input"));
    assert_ne!(devices[1].id, devices[2].id);
    let renamed =
        input_sources(br#"[{"name":"usb.input","description":"New friendly label"}]"#).unwrap();
    assert_eq!(devices[0].id, renamed[0].id);
    assert!(input_sources(b"bad json").is_none());
    assert!(input_sources(b"{}").is_none());
    assert!(input_sources(b"[]").unwrap().is_empty());
    for id in [
        "default",
        "mic:pulse:v1:!",
        "mic:pulse:v1:_w",
        "mic:pulse:v1:",
        "mic:pulse:v1:AA",
    ] {
        assert!(source_name(id).is_none(), "{id}");
    }
}

fn shell(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

#[test]
fn discovery_helpers_are_bounded_and_reaped() {
    assert!(discover(false, shell("exit 99")).is_none());
    assert!(discover(true, shell("printf '[]'")).unwrap().is_empty());
    assert_eq!(
        query(shell("printf '[]'"), Duration::from_secs(2)).unwrap(),
        b"[]"
    );
    assert!(query(shell("exit 7"), Duration::from_secs(2)).is_none());
    assert!(query(
        Command::new("/no/such/kolboo-test-command"),
        Duration::from_secs(1)
    )
    .is_none());
    assert!(query(shell("exec sleep 600"), Duration::ZERO).is_none());
    assert!(query(shell("head -c 1048577 /dev/zero"), Duration::from_secs(2)).is_none());
}

// Only the private native fixture calls this: generated silence, no hardware.
pub(crate) fn native_inputs() {
    use super::super::{device_selection, AudioCapture};
    assert!(std::env::var("PULSE_SERVER")
        .unwrap()
        .contains("kolboo-window-test-"));
    let devices = super::super::list_input_devices_v2();
    assert_eq!(devices.len(), 2);
    let host = cpal::default_host();
    assert!(device_selection::input_device_options(&host, Some(Vec::new())).is_empty());
    let fallback = device_selection::input_device_options(&host, None);
    let fallback = fallback
        .iter()
        .find(|device| device.name.starts_with("KolbooTestALSA"))
        .expect("isolated ALSA null input");
    assert!(device_selection::select_input_device_from_host(&host, Some(&fallback.id)).is_ok());
    assert!(device_selection::select_input_device_from_host(&host, Some("KolbooTestALSA")).is_ok());
    let selected =
        device_selection::select_input_device_from_host(&host, Some(&devices[1].id)).unwrap();
    assert_eq!(
        selected.default_input_config().unwrap().sample_rate(),
        48000
    );
    assert!(
        device_selection::select_input_device_from_host(&host, Some("mic:pulse:v1:!")).is_err()
    );
    for id in [
        "mic:alsa:v1:!",
        "mic:alsa:v1:eA",
        "mic:alsa:v1:_w",
        "mic:alsa:v1:YWxzYTp1bmF2YWlsYWJsZQ",
    ] {
        assert!(device_selection::select_input_device_from_host(&host, Some(id)).is_err());
    }
    let clock = Arc::new(AtomicU64::new(0));
    let dispatch = dispatch();
    let capture = Capture::start(
        &source_name(&devices[1].id).unwrap(),
        &cpal::StreamConfig {
            channels: 2,
            sample_rate: 44100,
            buffer_size: cpal::BufferSize::Default,
        },
        dispatch.clone(),
        Instant::now(),
        clock,
    )
    .unwrap();
    let expected_pid = capture.child.id().to_string();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut command = Command::new("/usr/bin/pactl");
        command.args(["--format=json", "list", "source-outputs"]);
        let outputs: serde_json::Value =
            serde_json::from_slice(&query(command, Duration::from_secs(1)).unwrap()).unwrap();
        if let Some(output) = outputs
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["properties"]["application.process.id"] == expected_pid)
        {
            let mut command = Command::new("/usr/bin/pactl");
            command.args(["--format=json", "list", "sources"]);
            let sources: serde_json::Value =
                serde_json::from_slice(&query(command, Duration::from_secs(1)).unwrap()).unwrap();
            let source = sources
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["index"] == output["source"])
                .unwrap();
            assert_eq!(source["name"], source_name(&devices[1].id).unwrap());
            let format = output["sample_specification"].as_str().unwrap();
            assert!(format.contains("2ch") && format.contains("44100Hz"));
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    drop(capture);
    let mut audio = AudioCapture::new();
    let missing = format!(
        "{ID_PREFIX}{}",
        URL_SAFE_NO_PAD.encode("kolboo_nonexistent_source")
    );
    audio.start_with_device_name(2.0, Some(&missing)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !audio
        .capture_handle
        .as_ref()
        .unwrap()
        .thread_handle
        .is_finished()
    {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    audio.ensure_stream_running(Some(&devices[0].id)).unwrap();
    audio.begin_recording_with_pre_roll(2.0);
    let deadline = Instant::now() + Duration::from_secs(3);
    while audio.buffer.lock().unwrap().captured_samples == 0 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    audio.stop();
    assert!(!audio.buffer.lock().unwrap().journal_failed);

    audio
        .set_capture_behavior(true, 0, false, Some(&devices[0].id))
        .unwrap();
    audio.begin_recording_with_pre_roll(2.0);
    audio
        .set_capture_behavior(true, 0, false, Some(&devices[1].id))
        .unwrap();
    assert_eq!(
        audio.stream_device_name.as_deref(),
        Some(devices[0].id.as_str())
    );
    audio.stop_recording();
    audio.ensure_stream_running(Some(&devices[1].id)).unwrap();
    assert_eq!(
        audio.stream_device_name.as_deref(),
        Some(devices[1].id.as_str())
    );
    audio.stop();
    native_recovery(&devices[0].id);
}

fn native_recovery(selected: &str) {
    use super::super::*;
    use cpal::traits::HostTrait;
    let audio = AudioCapture::new();
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (event_tx, _) = std::sync::mpsc::channel();
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: 44100,
        buffer_size: cpal::BufferSize::Fixed(64),
    };
    let device = InputDevice::Cpal(cpal::default_host().default_input_device().unwrap());
    let check = build_stream(
        &device,
        &config,
        cpal::SampleFormat::F32,
        Instant::now(),
        Arc::new(AtomicU64::new(0)),
        &dispatch(),
    )
    .unwrap();
    check.play().unwrap();
    check.check().unwrap();
    drop(check);
    let args = CaptureThreadArgs {
        capture_fence: audio.capture_fence.clone(),
        device,
        config,
        sample_format: cpal::SampleFormat::F32,
        buffer: audio.buffer.clone(),
        pre_roll: audio.pre_roll.clone(),
        recording_active: audio.recording_active.clone(),
        pre_roll_ms: audio.pre_roll_ms.clone(),
        meter: audio.level_meter.clone(),
        waveform_meter: audio.waveform_meter.clone(),
        command_rx,
        event_tx,
        vad_config: VadAutoStopConfig::default(),
        auto_recover_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        desired_device_name: Arc::new(std::sync::Mutex::new(Some(selected.into()))),
    };
    let mut calls = 0;
    run_capture_thread_with_builder(args, Instant::now() - Duration::from_secs(3), |device, config, format, start, clock, dispatch| {
        calls += 1;
        match calls {
            1 => {
                clock.store(1, Ordering::Relaxed);
                Capture::start_command(shell("exec sleep 600"), dispatch.clone(), start, clock).map(InputStream::Pulse)
            }
            2 => Err(AudioCaptureError::StreamBuild("synthetic device rebuild failure".into())),
            3 => {
                assert!(matches!(device, InputDevice::Pulse(source) if source == &source_name(selected).unwrap()));
                command_tx.send(CaptureCommand::Stop).unwrap();
                build_stream(device, config, format, start, clock, dispatch)
            }
            _ => panic!("recovery must stop after the selected source is rebound"),
        }
    }).unwrap();
    assert_eq!(calls, 3);
}

fn dispatch() -> CaptureDispatch {
    let pool = Arc::new(ArrayQueue::new(32));
    for _ in 0..32 {
        pool.push(Vec::with_capacity(1024)).unwrap();
    }
    CaptureDispatch {
        queue: Arc::new(ArrayQueue::new(32)),
        pool,
        fence: Arc::new(CaptureFence::default()),
    }
}

#[test]
fn native_stream_errors_fail_the_recording_and_preserve_startup_error_details() {
    use super::super::{stream_build_error, stream_error_handler};
    let dispatch = dispatch();
    let epoch = dispatch.fence.begin();
    let mut on_error = stream_error_handler(dispatch.clone());
    on_error(cpal::StreamError::DeviceNotAvailable);
    assert!(dispatch.fence.failed(epoch));
    dispatch.fence.stop();
    on_error(cpal::StreamError::StreamInvalidated);
    let next = dispatch.fence.begin();
    assert!(!dispatch.fence.failed(next));
    let native = cpal::BuildStreamError::StreamConfigNotSupported;
    assert!(
        matches!(stream_build_error(native.clone()), AudioCaptureError::StreamBuild(message) if message == native.to_string())
    );
}

#[test]
fn helper_stream_delivers_float_samples_marks_abrupt_eof_and_joins() {
    let dispatch = dispatch();
    let epoch = dispatch.fence.begin();
    let clock = Arc::new(AtomicU64::new(u64::MAX));
    let capture = Capture::start_command(
        shell("printf '\\000\\000\\000\\077\\000\\000\\200\\276'"),
        dispatch.clone(),
        Instant::now(),
        clock.clone(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !capture.is_finished() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let chunk = dispatch.queue.pop().unwrap();
    assert_eq!(chunk.samples, [0.5, -0.25]);
    assert_eq!(chunk.epoch, epoch);
    assert!(dispatch.fence.failed(epoch));
    assert_ne!(clock.load(Ordering::Relaxed), u64::MAX);
    let stream = super::super::InputStream::Pulse(capture);
    assert!(stream.play().is_ok());
    assert!(stream.check().is_err());
    drop(stream);
}

#[test]
fn helper_cancellation_owns_child_and_does_not_fail_later_recordings() {
    let dispatch = dispatch();
    let previous = dispatch.fence.begin();
    let capture = Capture::start_command(
        shell("exec sleep 600"),
        dispatch.clone(),
        Instant::now(),
        Arc::new(AtomicU64::new(0)),
    )
    .unwrap();
    let pid = capture.child.id();
    assert!(!capture.is_finished());
    let stream = super::super::InputStream::Pulse(capture);
    assert!(stream.check().is_ok());
    dispatch.fence.stop();
    drop(stream);
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let next = dispatch.fence.begin();
    assert!(!dispatch.fence.failed(next));
    assert!(!dispatch.fence.failed(previous));
    assert!(Capture::start_command(
        Command::new("/no/such/kolboo-helper"),
        dispatch,
        Instant::now(),
        Arc::new(AtomicU64::new(0))
    )
    .is_err());
}
