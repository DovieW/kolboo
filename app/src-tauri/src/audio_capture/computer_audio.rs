//! Linux opt-in microphone + monitor capture. No shell command interpolation.
use super::helper_process::{query, retire};
use super::{AudioBuffer, AudioCaptureError};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;

pub struct Capture {
    child: Option<Child>,
    worker: Option<JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    clock: Arc<std::sync::atomic::AtomicU64>,
    start: std::time::Instant,
}

pub fn available() -> bool {
    cfg!(target_os = "linux")
        && query(
            {
                let mut command = Command::new("/usr/bin/ffmpeg");
                command.args(["-hide_banner", "-devices"]);
                command
            },
            std::time::Duration::from_secs(2),
        )
        .map(|output| {
            String::from_utf8_lossy(&output)
                .lines()
                .any(|line| line.contains("pulse"))
        })
        .unwrap_or(false)
}

impl Capture {
    pub fn start(
        buffer: Arc<Mutex<AudioBuffer>>,
        active: Arc<AtomicBool>,
    ) -> Result<Self, AudioCaptureError> {
        if !available() {
            return Err(AudioCaptureError::StreamStart(
                "Computer audio requires Linux FFmpeg with PulseAudio input support".into(),
            ));
        }
        let sink = query(
            {
                let mut command = Command::new("/usr/bin/pactl");
                command.arg("get-default-sink");
                command
            },
            std::time::Duration::from_secs(2),
        )
        .ok_or(AudioCaptureError::StreamStart(
            "Unable to find default computer audio output".into(),
        ))?;
        let monitor = format!("{}.monitor", String::from_utf8_lossy(&sink).trim());
        Self::start_sources(buffer, active, "default", &monitor)
    }

    fn start_sources(
        buffer: Arc<Mutex<AudioBuffer>>,
        active: Arc<AtomicBool>,
        microphone: &str,
        monitor: &str,
    ) -> Result<Self, AudioCaptureError> {
        let mut child = Command::new("/usr/bin/ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "pulse",
                "-i",
                microphone,
                "-f",
                "pulse",
                "-i",
                monitor,
                "-filter_complex",
                "amix=inputs=2:duration=shortest",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-f",
                "f32le",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                AudioCaptureError::StreamStart("Unable to start computer audio capture".into())
            })?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| AudioCaptureError::StreamStart("Capture pipe is unavailable".into()))?;
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = stopping.clone();
        let clock = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let worker_clock = clock.clone();
        let start = std::time::Instant::now();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut ready_tx = Some(ready_tx);
            let mut bytes = [0u8; 4096];
            let mut pending = Vec::new();
            while let Ok(count) = stdout.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                if worker_stopping.load(Ordering::Relaxed) {
                    break;
                }
                worker_clock.store(start.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                crate::runtime_diagnostics::audio_callback(0);
                if let Some(tx) = ready_tx.take() {
                    let _ = tx.send(());
                }
                pending.extend_from_slice(&bytes[..count]);
                let complete = pending.len() / 4 * 4;
                if active.load(Ordering::Relaxed) {
                    let samples: Vec<f32> = pending[..complete]
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                        .collect();
                    if let Ok(mut buffer) = buffer.lock() {
                        buffer.append(&samples);
                    }
                }
                pending.drain(..complete);
            }
            mark_unexpected_end(&worker_stopping, &buffer);
        });
        let capture = Self {
            child: Some(child),
            worker: Some(worker),
            stopping,
            clock,
            start,
        };
        if ready_rx
            .recv_timeout(std::time::Duration::from_secs(8))
            .is_err()
        {
            return Err(AudioCaptureError::StreamStart(
                "Computer audio did not start. Check the system microphone and output devices."
                    .into(),
            ));
        }
        Ok(capture)
    }

    pub(super) fn stop(&mut self) -> bool {
        self.stopping.store(true, Ordering::Relaxed);
        let stopped = retire(
            self.child.take(),
            self.worker.take(),
            std::time::Duration::from_secs(1),
        );
        if !stopped {
            crate::runtime_diagnostics::record(crate::runtime_diagnostics::Event::Failure(
                crate::runtime_diagnostics::Failure::AudioStopTimedOut,
            ));
        }
        stopped
    }
    pub(super) fn is_stalled(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
            || super::callbacks_stalled(
                self.start.elapsed().as_millis() as u64 + 1,
                self.clock.load(Ordering::Relaxed),
            )
    }
}

fn mark_unexpected_end(stopping: &AtomicBool, buffer: &Mutex<AudioBuffer>) {
    if !stopping.load(Ordering::Relaxed) {
        crate::runtime_diagnostics::record(crate::runtime_diagnostics::Event::Failure(
            crate::runtime_diagnostics::Failure::AudioHelperExited,
        ));
        if let Ok(mut buffer) = buffer.lock() {
            buffer.journal_failed = true;
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn native_helper_exit() {
        assert_eq!(
            std::env::var("KOLBOO_NATIVE_WINDOW_TEST").as_deref(),
            Ok("1")
        );
        let buffer = Arc::new(Mutex::new(AudioBuffer::new(16000, 1, 10.0)));
        assert!(available());
        let mut capture = Capture::start(buffer.clone(), Arc::new(AtomicBool::new(true))).unwrap();
        assert!(!capture.is_stalled());
        assert!(!buffer.lock().unwrap().journal_failed);
        capture.child.as_mut().unwrap().kill().unwrap(); // Actual unexpected helper loss, not a fake callback.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !capture.worker.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(capture.is_stalled());
        drop(capture);
        assert!(buffer.lock().unwrap().journal_failed);
        let buffer = Arc::new(Mutex::new(AudioBuffer::new(16000, 1, 10.0)));
        let capture = Capture::start(buffer.clone(), Arc::new(AtomicBool::new(true))).unwrap();
        capture.stopping.store(true, Ordering::Relaxed);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !capture.worker.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!buffer.lock().unwrap().journal_failed);
        drop(capture);
    }

    pub(crate) fn stalled_capture() -> (Capture, std::sync::mpsc::Sender<()>) {
        let (release, held) = std::sync::mpsc::channel();
        (
            Capture {
                child: None,
                worker: Some(std::thread::spawn(move || {
                    let _ = held.recv();
                })),
                stopping: Arc::new(AtomicBool::new(false)),
                clock: Arc::new(std::sync::atomic::AtomicU64::new(1)),
                start: std::time::Instant::now() - std::time::Duration::from_secs(3),
            },
            release,
        )
    }

    #[test]
    fn only_unexpected_helper_exit_invalidates_the_recording() {
        let buffer = Mutex::new(AudioBuffer::new(16000, 1, 10.0));
        mark_unexpected_end(&AtomicBool::new(true), &buffer);
        assert!(!buffer.lock().unwrap().journal_failed);
        mark_unexpected_end(&AtomicBool::new(false), &buffer);
        assert!(buffer.lock().unwrap().journal_failed);
    }

    #[test]
    #[ignore = "Manual PulseAudio test: requires an isolated Kolboo null sink, never a real microphone"]
    fn captures_and_pauses_isolated_monitor_audio() {
        let source = std::env::var("KOLBOO_TEST_MONITOR").expect("test monitor required");
        assert!(source.starts_with("kolboo_capture_test_") && source.ends_with(".monitor"));
        let buffer = Arc::new(Mutex::new(AudioBuffer::new(16000, 1, 10.0)));
        let active = Arc::new(AtomicBool::new(true));
        let capture =
            Capture::start_sources(buffer.clone(), active.clone(), &source, &source).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        active.store(false, Ordering::Relaxed);
        // Let the worker finish a block that may already have sampled the flag.
        std::thread::sleep(std::time::Duration::from_millis(150));
        let paused_samples = buffer.lock().unwrap().captured_samples;
        assert!(paused_samples > 0);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(buffer.lock().unwrap().captured_samples, paused_samples);
        active.store(true, Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_millis(500));
        drop(capture);
        let buffer = buffer.lock().unwrap();
        assert!(buffer.captured_samples > paused_samples);
        assert!(!buffer.journal_failed);
        assert!(!buffer.to_wav_bytes().unwrap().is_empty());
    }
}
