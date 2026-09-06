//! Linux opt-in microphone + monitor capture. No shell command interpolation.
use super::{AudioBuffer, AudioCaptureError};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;

pub struct Capture {
    child: Child,
    worker: Option<JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
}

pub fn available() -> bool {
    cfg!(target_os = "linux")
        && Command::new("/usr/bin/ffmpeg")
            .args(["-hide_banner", "-devices"])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
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
        let sink = Command::new("pactl")
            .arg("get-default-sink")
            .output()
            .map_err(|_| {
                AudioCaptureError::StreamStart(
                    "Unable to find default computer audio output".into(),
                )
            })?;
        if !sink.status.success() {
            return Err(AudioCaptureError::StreamStart(
                "Computer audio output is unavailable".into(),
            ));
        }
        let monitor = format!("{}.monitor", String::from_utf8_lossy(&sink.stdout).trim());
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
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut ready_tx = Some(ready_tx);
            let mut bytes = [0u8; 4096];
            let mut pending = Vec::new();
            while let Ok(count) = stdout.read(&mut bytes) {
                if count == 0 {
                    break;
                }
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
            if !worker_stopping.load(Ordering::Relaxed) {
                if let Ok(mut buffer) = buffer.lock() {
                    buffer.journal_failed = true;
                }
            }
        });
        let capture = Self {
            child,
            worker: Some(worker),
            stopping,
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
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

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
