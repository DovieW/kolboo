//! Bounded ownership for the PulseAudio and FFmpeg adapters. A failed helper
//! may be quarantined, but must never hold the UI behind wait()/join().
use std::{
    io::{Read, Seek, SeekFrom},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

type ReaperSpawner = fn(Box<dyn FnOnce() -> bool + Send>) -> std::io::Result<JoinHandle<bool>>;

pub(super) fn retire(
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    budget: Duration,
) -> bool {
    retire_with(child, reader, budget, |task| {
        std::thread::Builder::new()
            .name("audio-helper-reaper".into())
            .spawn(task)
    })
}

fn retire_with(
    mut child: Option<Child>,
    reader: Option<JoinHandle<()>>,
    budget: Duration,
    spawn: ReaperSpawner,
) -> bool {
    if child.is_none() && reader.is_none() {
        return true;
    }
    if let Some(child) = child.as_mut() {
        let _ = child.kill();
    }
    let Ok(worker) = spawn(Box::new(move || {
        let process_done = child.is_none_or(|mut child| child.wait().is_ok());
        let reader_done = reader.is_none_or(|reader| reader.join().is_ok());
        process_done && reader_done
    })) else {
        return false;
    };
    let deadline = Instant::now() + budget;
    while !worker.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    if worker.is_finished() {
        worker.join().unwrap_or(false)
    } else {
        false
    }
}

pub(super) fn query(mut command: Command, timeout: Duration) -> Option<Vec<u8>> {
    const LIMIT: u64 = 1024 * 1024;
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
            Ok(None)
                if Instant::now() < deadline
                    && output.metadata().is_ok_and(|meta| meta.len() <= LIMIT) =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            _ => {
                retire(Some(child), None, Duration::ZERO);
                return None;
            }
        }
    };
    if !success || output.metadata().ok()?.len() > LIMIT {
        return None;
    }
    output.seek(SeekFrom::Start(0)).ok()?;
    let mut bytes = Vec::new();
    output.take(LIMIT + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= LIMIT).then_some(bytes)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn command(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }
    #[test]
    fn queries_bound_time_output_and_fail_closed() {
        assert_eq!(
            query(command("printf ready"), Duration::from_secs(2)).unwrap(),
            b"ready"
        );
        assert!(query(command("exit 1"), Duration::from_secs(2)).is_none());
        assert!(query(Command::new("/missing/kolboo-helper"), Duration::ZERO).is_none());
        assert!(query(command("exec sleep 600"), Duration::ZERO).is_none());
        assert!(query(command("head -c 1048577 /dev/zero"), Duration::from_secs(2)).is_none());
    }
    #[test]
    fn cleanup_never_waits_for_a_reader_holding_an_unavailable_buffer() {
        assert!(retire(None, None, Duration::ZERO));
        let (release, held) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            held.recv().unwrap();
            finished.send(()).unwrap();
        });
        let child = command("exec sleep 600").spawn().unwrap();
        assert!(!retire(Some(child), Some(reader), Duration::ZERO));
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(retire(
            Some(command("exec sleep 600").spawn().unwrap()),
            None,
            Duration::from_secs(2)
        ));
        assert!(!retire(
            None,
            Some(std::thread::spawn(|| panic!("synthetic reader failure"))),
            Duration::from_secs(2)
        ));
    }

    #[test]
    fn unavailable_reaper_reports_incomplete_cleanup_without_waiting() {
        let child = command("exec sleep 600").spawn().unwrap();
        assert!(!retire_with(Some(child), None, Duration::ZERO, |_| {
            Err(std::io::Error::other(
                "synthetic thread resource exhaustion",
            ))
        }));
    }
}
