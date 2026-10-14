//! PipeWire/PulseAudio input identity and owned capture. No shell, global audio
//! routing changes, or process-wide PULSE_SOURCE environment mutations.
use super::{dispatch::CaptureDispatch, AudioCaptureError, AudioInputDeviceInfo};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub(super) const ID_PREFIX: &str = "mic:pulse:v1:";

#[derive(Deserialize)]
struct Source {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    monitor_source: String,
    #[serde(default)]
    properties: HashMap<String, serde_json::Value>,
    // PulseAudio uses monitor_of_sink; newer PipeWire uses monitor_source.
    monitor_of_sink: Option<serde_json::Value>,
}

fn input_sources(payload: &[u8]) -> Option<Vec<AudioInputDeviceInfo>> {
    let sources: Vec<Source> = serde_json::from_slice(payload).ok()?;
    let mut seen = HashSet::new();
    Some(
        sources
            .into_iter()
            .filter_map(|source| {
                let monitor = source.monitor_of_sink.as_ref().is_some_and(|value| {
                    !value.is_null() && value.as_u64() != Some(u32::MAX as u64)
                });
                if source.name.is_empty()
                    || source.name.ends_with(".monitor")
                    || !source.monitor_source.is_empty()
                    || monitor
                    || source
                        .properties
                        .get("device.class")
                        .and_then(|v| v.as_str())
                        == Some("monitor")
                    || !seen.insert(source.name.clone())
                {
                    return None;
                }
                Some(AudioInputDeviceInfo {
                    id: format!(
                        "{ID_PREFIX}{}",
                        URL_SAFE_NO_PAD.encode(source.name.as_bytes())
                    ),
                    name: if source.description.trim().is_empty() {
                        source.name
                    } else {
                        source.description
                    },
                })
            })
            .collect(),
    )
}

pub(super) fn source_name(id: &str) -> Option<String> {
    let name = String::from_utf8(URL_SAFE_NO_PAD.decode(id.strip_prefix(ID_PREFIX)?).ok()?).ok()?;
    (!name.is_empty() && !name.contains('\0')).then_some(name)
}

// A stalled sound server must not freeze native tray construction. Redirecting
// stdout to an anonymous file avoids both pipe deadlocks and unbounded RAM use.
fn query(mut command: Command, timeout: Duration) -> Option<Vec<u8>> {
    let mut output = tempfile::tempfile().ok()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().ok()?))
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !success || output.metadata().ok()?.len() > 1024 * 1024 {
        return None;
    }
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut bytes = Vec::new();
    output.take(1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

pub(super) fn list_devices() -> Option<Vec<AudioInputDeviceInfo>> {
    let mut command = Command::new("/usr/bin/pactl");
    command.args(["--format=json", "list", "sources"]);
    discover(std::path::Path::new("/usr/bin/parec").is_file(), command)
}

fn discover(helper_available: bool, command: Command) -> Option<Vec<AudioInputDeviceInfo>> {
    if !helper_available {
        return None;
    }
    input_sources(&query(command, Duration::from_secs(1))?)
}

pub(super) struct Capture {
    child: Child,
    worker: Option<JoinHandle<()>>,
}

impl Capture {
    pub(super) fn is_finished(&self) -> bool {
        self.worker
            .as_ref()
            .expect("capture owns its reader until drop")
            .is_finished()
    }

    pub(super) fn start(
        source: &str,
        config: &cpal::StreamConfig,
        dispatch: CaptureDispatch,
        start: Instant,
        clock: Arc<AtomicU64>,
    ) -> Result<Self, AudioCaptureError> {
        let mut command = Command::new("/usr/bin/parec");
        command.args([
            "--raw",
            "--format=float32le",
            "--latency-msec=20",
            "--client-name=Kolboo",
            "--stream-name=Microphone",
        ]);
        command.arg(format!("--rate={}", config.sample_rate));
        command.arg(format!("--channels={}", config.channels));
        command.arg(format!("--device={source}"));
        Self::start_command(command, dispatch, start, clock)
    }

    fn start_command(
        mut command: Command,
        dispatch: CaptureDispatch,
        start: Instant,
        clock: Arc<AtomicU64>,
    ) -> Result<Self, AudioCaptureError> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                AudioCaptureError::StreamBuild(
                    "Linux microphone capture helper is unavailable".into(),
                )
            })?;
        let mut stdout = child.stdout.take().expect("capture stdout is piped");
        let worker = std::thread::spawn(move || {
            let mut bytes = [0u8; 4096];
            let mut pending = Vec::with_capacity(4100);
            while let Ok(count) = stdout.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                pending.extend_from_slice(&bytes[..count]);
                let complete = pending.len() / 4 * 4;
                let samples: Vec<f32> = pending[..complete]
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().expect("four-byte sample")))
                    .collect();
                clock.store(start.elapsed().as_millis() as u64, Ordering::Relaxed);
                dispatch.enqueue(&samples, std::convert::identity);
                pending.drain(..complete);
            }
            // Unexpected EOF is never a successful recording with missing audio.
            dispatch.fail_current();
        });
        Ok(Self {
            child,
            worker: Some(worker),
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Closing the producer releases a blocked reader even during suspend or
        // server failure. Join before the shared capture queue is drained.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
