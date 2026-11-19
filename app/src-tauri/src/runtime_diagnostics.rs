//! Local, content-free lifecycle evidence. Never write from an audio callback.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const RETENTION_MS: u64 = 62 * 24 * 60 * 60 * 1000;
const SAMPLE_INTERVAL: Duration = Duration::from_secs(60);
fn sample_interval(phase: Phase) -> Duration {
    if matches!(
        phase,
        Phase::Recording | Phase::Transcribing | Phase::Routing | Phase::Rewriting
    ) {
        Duration::from_secs(2)
    } else {
        SAMPLE_INTERVAL
    }
}
static SENDER: OnceLock<mpsc::SyncSender<Event>> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);
static WORKER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
static RUN_ID: OnceLock<Uuid> = OnceLock::new();
static START_LOCK: Mutex<()> = Mutex::new(());
static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
static LAST_AUDIO_MS: AtomicU64 = AtomicU64::new(0);
static AUDIO_CALLBACKS: AtomicU64 = AtomicU64::new(0);
static AUDIO_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
static CURRENT_PHASE: AtomicU64 = AtomicU64::new(0);
pub(crate) fn phase_tag() -> &'static str {
    phase_tag_for(CURRENT_PHASE.load(Ordering::Relaxed))
}
fn phase_tag_for(phase: u64) -> &'static str {
    match phase {
        1 => "idle",
        2 => "recording",
        3 => "transcribing",
        4 => "routing",
        5 => "rewriting",
        6 => "error",
        _ => "startup",
    }
}

/// Only atomics and a monotonic clock on the realtime path, never logging/I/O.
pub(crate) fn audio_callback(queue_depth: usize) {
    if let Some(origin) = ORIGIN.get() {
        LAST_AUDIO_MS.store(origin.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
        AUDIO_CALLBACKS.fetch_add(1, Ordering::Relaxed);
        AUDIO_QUEUE_DEPTH.store(queue_depth as u64, Ordering::Relaxed);
    }
}
pub(crate) fn run_id() -> Option<Uuid> {
    RUN_ID.get().copied()
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    #[default]
    Startup,
    Idle,
    Recording,
    Transcribing,
    Routing,
    Rewriting,
    Error,
}
impl From<crate::pipeline::PipelineState> for Phase {
    fn from(value: crate::pipeline::PipelineState) -> Self {
        use crate::pipeline::PipelineState as P;
        match value {
            P::Idle => Self::Idle,
            P::Recording => Self::Recording,
            P::Transcribing => Self::Transcribing,
            P::Routing => Self::Routing,
            P::Rewriting => Self::Rewriting,
            P::Error => Self::Error,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Failure {
    AudioStream,
    AudioOutput,
    MainWindow,
    PresetWindow,
    QuickAskWindow,
    AudioHelperExited,
    AudioQueueOverflow,
    AudioPoolExhausted,
    AudioCallbackStalled,
    AudioRecoveryStarted,
    AudioRecoverySucceeded,
    AudioRecoveryFailed,
    AudioStopTimedOut,
    Pipeline,
    DiagnosticStorage,
    LogQueueOverflow,
}
impl Failure {
    fn report_code(self) -> Option<&'static str> {
        Some(match self {
            Self::AudioStream => "audio_stream",
            Self::AudioOutput => "audio_output",
            Self::MainWindow => "main_window",
            Self::PresetWindow => "preset_window",
            Self::QuickAskWindow => "quick_ask_window",
            Self::AudioHelperExited => "audio_helper_exited",
            Self::AudioQueueOverflow => "audio_queue_overflow",
            Self::AudioPoolExhausted => "audio_pool_exhausted",
            Self::AudioCallbackStalled => "audio_callback_stalled",
            Self::AudioRecoveryFailed => "audio_recovery_failed",
            Self::AudioStopTimedOut => "audio_stop_timed_out",
            Self::Pipeline => "pipeline",
            Self::DiagnosticStorage => "diagnostic_storage",
            Self::LogQueueOverflow => "log_queue_overflow",
            Self::AudioRecoveryStarted | Self::AudioRecoverySucceeded => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum Event {
    Started,
    Phase(Phase),
    Failure(Failure),
    Sample,
    PreviousRunUnclean,
    Shutdown,
    DeviceSelectionChanged,
    AudioConfigured {
        sample_rate: Option<u32>,
        channels: Option<u16>,
        helper_pid: Option<u32>,
    },
}

/// All exported fields have closed types: no messages, paths, device names or content.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    run_id: Uuid,
    pid: u32,
    operation_id: u64,
    version: [u32; 3],
    build_revision: Option<Uuid>,
    started_ms: u64,
    updated_ms: u64,
    phase: Phase,
    clean_shutdown: bool,
    event: Event,
    dropped_events: u64,
    dropped_logs: u64,
    rss_bytes: Option<u64>,
    swap_bytes: Option<u64>,
    cpu_ticks: Option<u64>,
    last_audio_callback_age_ms: Option<u64>,
    audio_callback_count: u64,
    audio_queue_depth: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

struct Recorder {
    directory: PathBuf,
    state: Record,
    _lock: File,
}
impl Recorder {
    fn open(directory: PathBuf, now: u64) -> io::Result<(Self, Option<Record>)> {
        fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("owner.lock"))?;
        lock.try_lock()
            .map_err(|_| io::Error::from(io::ErrorKind::WouldBlock))?;
        let mut damaged = false;
        let previous = match File::open(directory.join("state.json")) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(8193).read_to_end(&mut bytes)?;
                let parsed = serde_json::from_slice::<Record>(&bytes).ok();
                damaged = parsed.is_none();
                parsed.filter(|record| !record.clean_shutdown)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let state = Record {
            run_id: Uuid::new_v4(),
            pid: std::process::id(),
            operation_id: 0,
            version: [
                env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            ],
            // A typed 128-bit prefix identifies the build without accepting arbitrary text.
            build_revision: option_env!("KOLBOO_BUILD_REVISION")
                .and_then(|value| value.get(..32))
                .and_then(|value| Uuid::parse_str(value).ok()),
            started_ms: now,
            updated_ms: now,
            phase: Phase::Startup,
            clean_shutdown: false,
            event: Event::Started,
            dropped_events: 0,
            dropped_logs: 0,
            rss_bytes: None,
            swap_bytes: None,
            cpu_ticks: None,
            last_audio_callback_age_ms: None,
            audio_callback_count: 0,
            audio_queue_depth: 0,
        };
        let mut recorder = Self {
            directory,
            state,
            _lock: lock,
        };
        if let Some(previous) = &previous {
            let mut evidence = previous.clone();
            evidence.event = Event::PreviousRunUnclean;
            recorder.append(&evidence, now)?;
        }
        recorder.write(Event::Started, now)?;
        if damaged {
            recorder.write(Event::Failure(Failure::DiagnosticStorage), now)?;
        }
        Ok((recorder, previous))
    }
    fn append(&self, record: &Record, now: u64) -> io::Result<()> {
        self.prune(now)?;
        let path = self
            .directory
            .join(format!("events-{}.jsonl", now / 86_400_000));
        let mut options = File::options();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(path)?;
        serde_json::to_writer(&mut output, record)?;
        output.write_all(b"\n")?;
        output.sync_data()
    }
    fn write(&mut self, event: Event, now: u64) -> io::Result<()> {
        self.state.updated_ms = now;
        self.state.event = event;
        if let Event::Phase(phase) = event {
            CURRENT_PHASE.store(phase as u64, Ordering::Relaxed);
            if matches!(
                self.state.phase,
                Phase::Idle | Phase::Startup | Phase::Error
            ) && matches!(phase, Phase::Recording | Phase::Transcribing)
            {
                self.state.operation_id += 1;
            }
            self.state.phase = phase;
        }
        if matches!(
            event,
            Event::Phase(_) | Event::Failure(_) | Event::Started | Event::Shutdown
        ) {
            sample_resources(&mut self.state);
        }
        self.state.clean_shutdown = event == Event::Shutdown;
        self.state.dropped_events = DROPPED.load(Ordering::Relaxed);
        self.state.dropped_logs = crate::tracing_init::dropped_lines();
        self.append(&self.state, now)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        serde_json::to_writer(temporary.as_file_mut(), &self.state)?;
        temporary.as_file().sync_all()?;
        temporary.persist(self.directory.join("state.json"))?;
        Ok(())
    }
    fn prune(&self, now: u64) -> io::Result<()> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && event_file_day(&entry.file_name().to_string_lossy()).is_some()
            {
                files.push((entry.path(), entry.metadata()?.len()));
            }
        }
        files.sort_by_key(|(path, _)| {
            event_file_day(path.file_name().unwrap().to_str().unwrap()).unwrap()
        });
        let mut bytes: u64 = files.iter().map(|(_, size)| size).sum();
        for (path, size) in files {
            let day = event_file_day(path.file_name().unwrap().to_str().unwrap()).unwrap();
            if now.saturating_sub(day.saturating_mul(86_400_000)) > RETENTION_MS
                || bytes >= MAX_BYTES - 8192
            {
                fs::remove_file(path)?;
                bytes = bytes.saturating_sub(size);
            }
        }
        Ok(())
    }
}
fn event_file_day(name: &str) -> Option<u64> {
    name.strip_prefix("events-")?
        .strip_suffix(".jsonl")?
        .parse()
        .ok()
}

pub(crate) fn record(event: Event) {
    if let Some(sender) = SENDER.get() {
        if sender.try_send(event).is_err() {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) fn start(directory: PathBuf) -> io::Result<()> {
    let _starting = START_LOCK.lock().unwrap();
    if SENDER.get().is_some() {
        return Ok(());
    }
    let _ = ORIGIN.set(std::time::Instant::now());
    let (recorder, previous) = Recorder::open(directory, now_ms())?;
    let _ = RUN_ID.set(recorder.state.run_id);
    if previous.is_some() {
        crate::sentry_init::report_runtime_failure("previous_run_unclean");
    }
    if recorder.state.event == Event::Failure(Failure::DiagnosticStorage) {
        crate::sentry_init::report_runtime_failure("diagnostic_storage");
    }
    let (tx, rx) = mpsc::sync_channel(64);
    let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));
    let worker = std::thread::Builder::new()
        .name("kolboo-diagnostics".into())
        .spawn(move || {
            sentry::Hub::run(hub, || {
                run_worker(recorder, rx, std::time::Instant::now() + SAMPLE_INTERVAL)
            });
        })?;
    SENDER
        .set(tx)
        .expect("runtime initialization is serialized");
    *WORKER.lock().unwrap() = Some(worker);
    Ok(())
}

fn run_worker(
    mut recorder: Recorder,
    rx: mpsc::Receiver<Event>,
    mut sample_at: std::time::Instant,
) {
    let mut reported_logs = 0;
    let mut reported_events = 0;
    loop {
        let event =
            match rx.recv_timeout(sample_at.saturating_duration_since(std::time::Instant::now())) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Timeout) => Event::Sample,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
        if event == Event::Sample {
            sample_resources(&mut recorder.state);
            sample_at = std::time::Instant::now() + sample_interval(recorder.state.phase);
            if crate::tracing_init::dropped_lines() > reported_logs {
                crate::sentry_init::report_runtime_failure("log_queue_overflow");
                reported_logs = crate::tracing_init::dropped_lines();
                let _ = recorder.write(Event::Failure(Failure::LogQueueOverflow), now_ms());
            }
            if DROPPED.load(Ordering::Relaxed) > reported_events {
                crate::sentry_init::report_runtime_failure("diagnostic_queue_overflow");
                reported_events = DROPPED.load(Ordering::Relaxed);
            }
        }
        if let Event::Failure(failure) = event {
            if let Some(code) = failure.report_code() {
                crate::sentry_init::report_runtime_failure(code);
            }
        }
        if recorder.write(event, now_ms()).is_err() {
            crate::sentry_init::report_runtime_failure("diagnostic_storage");
        }
        if matches!(event, Event::Phase(_)) {
            sample_at = std::time::Instant::now() + sample_interval(recorder.state.phase);
        }
        if event == Event::Shutdown {
            break;
        }
    }
}

pub(crate) fn start_best_effort(directory: Result<PathBuf, String>) {
    let ready = directory
        .ok()
        .is_some_and(|directory| start(directory.join("diagnostics")).is_ok());
    if !ready {
        crate::sentry_init::report_runtime_failure("diagnostic_storage");
        log::warn!("Content-free runtime diagnostics unavailable");
    }
}

pub(crate) fn shutdown() {
    record(Event::Shutdown);
    // Do not hang shutdown on a failed disk: an incomplete shutdown remains unclean.
    let worker = WORKER.lock().unwrap().take();
    if let Some(worker) = worker {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !worker.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if worker.is_finished() {
            let _ = worker.join();
        }
    }
    crate::tracing_init::shutdown();
    let _ = crate::sentry_init::flush_on_exit();
}

fn sample_resources(state: &mut Record) {
    let last = LAST_AUDIO_MS.load(Ordering::Relaxed);
    state.last_audio_callback_age_ms = ORIGIN
        .get()
        .filter(|_| last != 0)
        .map(|origin| (origin.elapsed().as_millis() as u64 + 1).saturating_sub(last));
    state.audio_callback_count = AUDIO_CALLBACKS.load(Ordering::Relaxed);
    state.audio_queue_depth = AUDIO_QUEUE_DEPTH.load(Ordering::Relaxed);
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status").ok();
        state.rss_bytes = status
            .as_deref()
            .and_then(|status| status_kib(status, "VmRSS:"));
        state.swap_bytes = status
            .as_deref()
            .and_then(|status| status_kib(status, "VmSwap:"));
        state.cpu_ticks = fs::read_to_string("/proc/self/stat").ok().and_then(|stat| {
            let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
            fields
                .get(11)?
                .parse::<u64>()
                .ok()?
                .checked_add(fields.get(12)?.parse().ok()?)
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = state; // Unavailable is null, never a misleading zero.
}
#[cfg(any(target_os = "linux", test))]
fn status_kib(status: &str, key: &str) -> Option<u64> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(key))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

/// Export only validated diagnostic records, never arbitrary log lines or files.
pub(crate) fn export_records_to(directory: &Path, output: &mut impl Write) -> io::Result<()> {
    export_bounded(directory, output, MAX_BYTES as usize)
}
fn export_bounded(directory: &Path, output: &mut impl Write, budget: usize) -> io::Result<()> {
    let mut total = 0;
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || event_file_day(&entry.file_name().to_string_lossy()).is_none()
        {
            continue;
        }
        if entry.metadata()?.len() > MAX_BYTES {
            continue;
        }
        let mut input = BufReader::new(File::open(entry.path())?);
        loop {
            let mut line = Vec::new();
            let size = input.by_ref().take(8193).read_until(b'\n', &mut line)?;
            if size == 0 || size > 8192 {
                break;
            }
            if let Ok(record) = serde_json::from_slice::<Record>(&line) {
                if total + size > budget {
                    return Ok(());
                }
                serde_json::to_writer(&mut *output, &record)?;
                output.write_all(b"\n")?;
                total += size;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn export_records(directory: &Path) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        export_records_to(directory, &mut output)?;
        Ok(output)
    }
    const TIME: u64 = 1000 * 86_400_000;

    #[test]
    fn worker_process() {
        let Some(directory) = std::env::var_os("KOLBOO_RUNTIME_FIXTURE_DIR") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let mode = std::env::var("KOLBOO_RUNTIME_FIXTURE_MODE").unwrap();
        if mode == "overflow" {
            let (tx, rx) = mpsc::sync_channel(1);
            SENDER.set(tx).unwrap();
            record(Event::Sample);
            record(Event::Sample);
            assert_eq!(DROPPED.load(Ordering::Relaxed), 1);
            let (recorder, _) = Recorder::open(directory.clone(), now_ms()).unwrap();
            let events = crate::sentry_init::tests::captured_runtime_events(|| {
                let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));
                let worker = std::thread::spawn(move || {
                    sentry::Hub::run(hub, || {
                        run_worker(recorder, rx, std::time::Instant::now() + SAMPLE_INTERVAL)
                    });
                });
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while serde_json::from_slice::<Record>(
                    &fs::read(directory.join("state.json")).unwrap(),
                )
                .unwrap()
                .event
                    != Event::Sample
                {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
                record(Event::Shutdown);
                worker.join().unwrap();
            });
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].tags["runtime_code"], "diagnostic_queue_overflow");
            let state: Record =
                serde_json::from_slice(&fs::read(directory.join("state.json")).unwrap()).unwrap();
            assert!(state.clean_shutdown);
            assert_eq!(state.dropped_events, 1);
            return;
        }
        if mode == "loss" {
            fs::write(directory.join("state.json"), b"PRIVATE damaged marker").unwrap();
            let (release, guard) = crate::tracing_init::overflowing_test_writer();
            let events = crate::sentry_init::tests::captured_runtime_events(|| {
                start(directory.clone()).unwrap();
                start(directory.join("never-created")).unwrap();
                assert!(!directory.join("never-created").exists());
                record(Event::Phase(Phase::Recording));
                record(Event::Failure(Failure::AudioStream));
                record(Event::Sample);
                shutdown();
            });
            drop(release);
            drop(guard);
            assert!(events
                .iter()
                .any(|event| event.tags["runtime_code"] == "log_queue_overflow"));
            assert!(events
                .iter()
                .any(|event| event.tags["runtime_code"] == "diagnostic_storage"));
            for event in &events {
                assert_eq!(
                    event.tags["diagnostic_run_id"],
                    run_id().unwrap().to_string()
                );
                assert!(!serde_json::to_string(event).unwrap().contains("PRIVATE"));
            }
            let output = String::from_utf8(export_records(&directory).unwrap()).unwrap();
            assert!(output.contains("log_queue_overflow"));
            return;
        }
        crate::tracing_init::init();
        start(directory.clone()).unwrap();
        audio_callback(3);
        record(Event::Phase(Phase::Recording));
        record(Event::Failure(Failure::AudioStream));
        record(Event::Failure(Failure::AudioRecoverySucceeded));
        record(Event::Sample);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let current = fs::read(directory.join("state.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok());
            if current.is_some_and(|current| current.event == Event::Sample) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker failed to acknowledge sample"
            );
            std::thread::yield_now();
        }
        assert!(run_id().is_some());
        if mode == "abrupt" {
            std::process::exit(0);
        }
        log::info!("diagnostic-flush-fixture");
        shutdown();
        let logs = crate::tracing_init::log_dir().unwrap();
        let flushed = fs::read_dir(logs)
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read_to_string(entry.path()).ok())
            .any(|text| text.contains("diagnostic-flush-fixture"));
        assert!(
            flushed,
            "orderly shutdown must flush the nonblocking file writer"
        );
        assert!(
            serde_json::from_slice::<Record>(&fs::read(directory.join("state.json")).unwrap())
                .unwrap()
                .clean_shutdown
        );
        shutdown(); // idempotent cleanup
    }

    #[test]
    fn actual_worker_and_restart_preserve_unclean_evidence_without_gui_or_network() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("diagnostics");
        for mode in ["abrupt", "clean", "clean"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "runtime_diagnostics::tests::worker_process",
                    "--nocapture",
                ])
                .env("KOLBOO_RUNTIME_FIXTURE_DIR", &directory)
                .env("KOLBOO_RUNTIME_FIXTURE_MODE", mode)
                .env(crate::app_paths::APP_DATA_DIR_ENV, temp.path())
                .env("TAURI_SENTRY_DSN", "")
                .status()
                .unwrap();
            assert!(status.success());
        }
        let output = String::from_utf8(export_records(&directory).unwrap()).unwrap();
        assert!(output.contains("previous_run_unclean"));
        assert!(output.contains("audio_stream"));
        assert!(output.contains("shutdown"));
        for mode in ["overflow", "loss"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "runtime_diagnostics::tests::worker_process"])
                .env("KOLBOO_RUNTIME_FIXTURE_DIR", &directory)
                .env("KOLBOO_RUNTIME_FIXTURE_MODE", mode)
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    #[test]
    fn lifecycle_checkpoint_distinguishes_unclean_and_clean_exits_and_owns_its_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("diagnostics");
        let (mut first, previous) = Recorder::open(path.clone(), TIME).unwrap();
        assert!(previous.is_none());
        assert_eq!(
            Recorder::open(path.clone(), TIME).err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        let id = first.state.run_id;
        first
            .write(Event::Phase(Phase::Recording), TIME + 1)
            .unwrap();
        first
            .write(Event::Phase(Phase::Transcribing), TIME + 2)
            .unwrap();
        assert_eq!(first.state.operation_id, 1);
        drop(first); // Model an abrupt process loss without a Shutdown event.
        let (mut second, previous) = Recorder::open(path.clone(), TIME + 3).unwrap();
        let previous = previous.unwrap();
        assert_eq!(previous.run_id, id);
        assert_eq!(previous.phase, Phase::Transcribing);
        assert_ne!(second.state.run_id, id);
        let exported = String::from_utf8(export_records(&path).unwrap()).unwrap();
        assert!(exported.contains("previous_run_unclean"));
        second.write(Event::Shutdown, TIME + 4).unwrap();
        drop(second);
        let (third, previous) = Recorder::open(path.clone(), TIME + 5).unwrap();
        assert!(previous.is_none());
        assert_eq!(third.state.phase, Phase::Startup);
        assert!(!third.state.clean_shutdown);
    }

    #[test]
    fn retention_is_bounded_by_age_and_bytes_and_preserves_unrelated_files() {
        let temp = tempfile::tempdir().unwrap();
        let (recorder, _) = Recorder::open(temp.path().into(), TIME).unwrap();
        let old = temp.path().join("events-1.jsonl");
        fs::write(&old, b"old").unwrap();
        let huge = temp.path().join("events-999.jsonl");
        File::create(&huge).unwrap().set_len(MAX_BYTES).unwrap();
        fs::write(temp.path().join("unrelated.txt"), b"private").unwrap();
        recorder.prune(TIME).unwrap();
        assert!(!old.exists());
        assert!(!huge.exists());
        assert!(temp.path().join("unrelated.txt").exists());
        assert!(temp.path().join("state.json").exists());
        assert_eq!(event_file_day("events-private.jsonl"), None);
        assert_eq!(event_file_day("events-1000.jsonl"), Some(1000));
    }

    #[test]
    fn export_rejects_raw_text_unknown_fields_symlinks_and_invalid_types() {
        let temp = tempfile::tempdir().unwrap();
        let (recorder, _) = Recorder::open(temp.path().into(), TIME).unwrap();
        let safe = serde_json::to_value(&recorder.state).unwrap();
        let mut dirty = safe.clone();
        dirty["prompt"] = "PRIVATE".into();
        let mut bad = safe.clone();
        bad["phase"] = "PRIVATE".into();
        let mut output = serde_json::to_vec(&safe).unwrap();
        output.push(b'\n');
        output.extend_from_slice(format!("PRIVATE\n{dirty}\n{bad}\n").as_bytes());
        fs::write(temp.path().join("events-1000.jsonl"), output).unwrap();
        fs::write(temp.path().join("settings.json"), b"PRIVATE").unwrap();
        fs::create_dir(temp.path().join("events-1001.jsonl")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            temp.path().join("settings.json"),
            temp.path().join("events-1002.jsonl"),
        )
        .unwrap();
        File::create(temp.path().join("events-1003.jsonl"))
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        let exported = String::from_utf8(export_records(temp.path()).unwrap()).unwrap();
        assert!(!exported.contains("PRIVATE"));
        assert_eq!(exported.lines().count(), 1);
        assert_eq!(
            serde_json::from_str::<Record>(exported.trim()).unwrap(),
            recorder.state
        );
        assert!(export_records(&temp.path().join("missing"))
            .unwrap()
            .is_empty());
        assert!(export_records(&temp.path().join("settings.json")).is_err());
    }

    #[test]
    fn malformed_checkpoint_does_not_turn_private_content_into_evidence() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("state.json"),
            b"PRIVATE arbitrary old content",
        )
        .unwrap();
        let (mut recorder, previous) = Recorder::open(temp.path().into(), TIME).unwrap();
        assert!(previous.is_none());
        assert_eq!(
            recorder.state.event,
            Event::Failure(Failure::DiagnosticStorage)
        );
        recorder.write(Event::Phase(Phase::Idle), TIME + 1).unwrap();
        recorder
            .write(Event::Phase(Phase::Transcribing), TIME + 2)
            .unwrap();
        assert_eq!(recorder.state.operation_id, 1);
        recorder.write(Event::Phase(Phase::Idle), TIME + 3).unwrap();
        recorder
            .write(Event::Phase(Phase::Recording), TIME + 4)
            .unwrap();
        assert_eq!(recorder.state.operation_id, 2);
        assert!(!String::from_utf8(export_records(temp.path()).unwrap())
            .unwrap()
            .contains("PRIVATE"));
        fs::create_dir(temp.path().join("blocked")).unwrap();
        assert!(Recorder::open(temp.path().join("state.json"), TIME).is_err());
        fs::remove_file(temp.path().join("state.json")).unwrap();
        fs::create_dir(temp.path().join("state.json")).unwrap();
        assert!(recorder.write(Event::Sample, TIME + 5).is_err());
    }

    #[test]
    fn closed_event_taxonomy_and_resource_metadata_are_content_free() {
        for (value, tag) in [
            (0, "startup"),
            (1, "idle"),
            (2, "recording"),
            (3, "transcribing"),
            (4, "routing"),
            (5, "rewriting"),
            (6, "error"),
            (u64::MAX, "startup"),
        ] {
            assert_eq!(phase_tag_for(value), tag);
        }
        use crate::pipeline::PipelineState as P;
        for (state, phase) in [
            (P::Idle, Phase::Idle),
            (P::Recording, Phase::Recording),
            (P::Transcribing, Phase::Transcribing),
            (P::Routing, Phase::Routing),
            (P::Rewriting, Phase::Rewriting),
            (P::Error, Phase::Error),
        ] {
            assert_eq!(Phase::from(state), phase);
        }
        for phase in [Phase::Startup, Phase::Idle, Phase::Error] {
            assert_eq!(sample_interval(phase), Duration::from_secs(60));
        }
        for phase in [
            Phase::Recording,
            Phase::Transcribing,
            Phase::Routing,
            Phase::Rewriting,
        ] {
            assert_eq!(sample_interval(phase), Duration::from_secs(2));
        }
        for reason in [
            Failure::AudioStream,
            Failure::AudioOutput,
            Failure::MainWindow,
            Failure::PresetWindow,
            Failure::QuickAskWindow,
            Failure::AudioHelperExited,
            Failure::AudioQueueOverflow,
            Failure::AudioPoolExhausted,
            Failure::AudioCallbackStalled,
            Failure::AudioRecoveryFailed,
            Failure::AudioStopTimedOut,
            Failure::Pipeline,
            Failure::DiagnosticStorage,
            Failure::LogQueueOverflow,
        ] {
            assert!(reason.report_code().is_some());
        }
        for reason in [
            Failure::AudioRecoveryStarted,
            Failure::AudioRecoverySucceeded,
        ] {
            assert!(reason.report_code().is_none());
        }
        assert_eq!(
            status_kib("VmRSS: 123 kB\nVmSwap: 4 kB", "VmRSS:"),
            Some(123 * 1024)
        );
        for input in [
            "",
            "VmRSS:",
            "VmRSS: invalid",
            "VmRSS: 18446744073709551615",
        ] {
            assert_eq!(status_kib(input, "VmRSS:"), None);
        }
        let temp = tempfile::tempdir().unwrap();
        let (mut recorder, _) = Recorder::open(temp.path().into(), now_ms()).unwrap();
        sample_resources(&mut recorder.state);
        #[cfg(target_os = "linux")]
        {
            assert!(recorder.state.rss_bytes.is_some());
            assert!(recorder.state.cpu_ticks.is_some());
        }
        assert!(now_ms() > 0);
    }

    #[test]
    fn output_and_window_failures_have_distinct_content_free_wire_codes() {
        for (failure, code) in [
            (Failure::AudioOutput, "audio_output"),
            (Failure::MainWindow, "main_window"),
            (Failure::PresetWindow, "preset_window"),
            (Failure::QuickAskWindow, "quick_ask_window"),
        ] {
            assert_eq!(failure.report_code(), Some(code));
            let wire = serde_json::to_string(&failure).unwrap();
            assert_eq!(wire, format!("\"{code}\""));
            assert_eq!(serde_json::from_str::<Failure>(&wire).unwrap(), failure);
        }
        assert_ne!(
            Failure::AudioStream.report_code(),
            Failure::AudioOutput.report_code()
        );
        assert!(serde_json::from_str::<Failure>("\"arbitrary private error\"").is_err());
    }

    #[test]
    fn bounded_export_and_storage_errors_are_not_silent_success() {
        let temp = tempfile::tempdir().unwrap();
        let (recorder, _) = Recorder::open(temp.path().into(), TIME).unwrap();
        let mut output = Vec::new();
        export_bounded(temp.path(), &mut output, 1).unwrap();
        assert!(output.is_empty());
        struct FailedWriter;
        impl Write for FailedWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::WriteZero))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(export_records_to(temp.path(), &mut FailedWriter).is_err());
        assert!(FailedWriter.flush().is_ok());
        drop(recorder);
        fs::remove_file(temp.path().join("state.json")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("state.json", temp.path().join("state.json")).unwrap();
            assert!(Recorder::open(temp.path().into(), TIME).is_err());
        }
    }

    #[test]
    fn worker_handles_owner_loss_expired_sample_and_storage_failure() {
        let temp = tempfile::tempdir().unwrap();
        let (recorder, _) = Recorder::open(temp.path().into(), now_ms()).unwrap();
        let (tx, rx) = mpsc::channel();
        // An already-expired sampling deadline exercises the actual timeout path.
        let path = temp.path().to_path_buf();
        let worker =
            std::thread::spawn(move || run_worker(recorder, rx, std::time::Instant::now()));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let state: Record =
                serde_json::from_slice(&fs::read(path.join("state.json")).unwrap()).unwrap();
            if state.event == Event::Sample {
                assert!(!state.clean_shutdown);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        drop(tx);
        worker.join().unwrap();
        let (recorder, previous) = Recorder::open(path.clone(), now_ms()).unwrap();
        assert!(
            previous.is_some(),
            "owner loss cannot be labeled a clean exit"
        );
        fs::remove_file(path.join("state.json")).unwrap();
        fs::create_dir(path.join("state.json")).unwrap();
        let (tx, rx) = mpsc::channel();
        tx.send(Event::Failure(Failure::AudioStream)).unwrap();
        tx.send(Event::Shutdown).unwrap();
        run_worker(recorder, rx, std::time::Instant::now() + SAMPLE_INTERVAL);
        assert!(path.join("state.json").is_dir());
        let records = String::from_utf8(export_records(&path).unwrap()).unwrap();
        assert!(records.contains("audio_stream"));
        start_best_effort(Err("PRIVATE".into()));
        let blocked = tempfile::NamedTempFile::new().unwrap();
        start_best_effort(Ok(blocked.path().into()));
    }
}
