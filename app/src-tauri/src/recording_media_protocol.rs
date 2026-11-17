//! Private, range-capable playback for recordings owned by `RecordingStore`.
//!
//! WebKitGTK does not reliably hand custom-scheme responses to its media
//! pipeline. This process-local HTTP server gives the media element an ordinary
//! byte-range stream without exposing filesystem paths or accepting arbitrary
//! files. The random token changes on every app start.

use crate::recordings::RecordingStore;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_RANGE_BYTES: u64 = 1024 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 8;
type Connections = Arc<Mutex<Vec<Option<TcpStream>>>>;
type RequestSpawner = fn(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>;

// Keep native resource failures at this boundary. The request/admission and
// shutdown state machine remains the same under descriptor/thread exhaustion.
#[derive(Clone, Copy)]
struct WorkerIo {
    spawn: RequestSpawner,
    duplicate: fn(&TcpStream) -> std::io::Result<TcpStream>,
    accept: fn(&TcpListener) -> std::io::Result<(TcpStream, SocketAddr)>,
}
const NATIVE_IO: WorkerIo = WorkerIo {
    spawn: spawn_request,
    duplicate: TcpStream::try_clone,
    accept: TcpListener::accept,
};

fn spawn_request(task: Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("recording-media-request".into())
        .spawn(task)
}

pub struct RecordingMediaServer {
    address: SocketAddr,
    token: String,
    shutdown: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
    connections: Connections,
}

impl RecordingMediaServer {
    pub fn start(recordings_dir: PathBuf) -> Result<Self, String> {
        Self::start_with_io(recordings_dir, NATIVE_IO)
    }

    fn start_with_io(recordings_dir: PathBuf, io: WorkerIo) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Could not start recording playback: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("Could not read recording playback address: {error}"))?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let worker_token = token.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let connections: Connections =
            Arc::new(Mutex::new((0..MAX_CONNECTIONS).map(|_| None).collect()));
        let worker_connections = connections.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("recording-media".to_string())
            .spawn(move || {
                serve(
                    listener,
                    recordings_dir,
                    worker_token,
                    worker_shutdown,
                    worker_connections,
                    io,
                    ready_tx,
                );
            })
            .map_err(|error| format!("Could not start recording playback worker: {error}"))?;

        let server = Self {
            address,
            token,
            shutdown,
            worker: Mutex::new(Some(worker)),
            connections,
        };
        match ready_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(true) => Ok(server),
            _ => Err("Recording playback workers could not start".into()),
        }
    }

    pub fn url_for(&self, request_id: &str) -> Result<String, String> {
        if !RecordingStore::is_safe_request_id(request_id) {
            return Err("Invalid recording id".to_string());
        }
        Ok(format!(
            "http://{}/{}/{}",
            self.address, self.token, request_id
        ))
    }
}

impl Drop for RecordingMediaServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        {
            let connections = self
                .connections
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for connection in connections.iter().flatten() {
                let _ = connection.shutdown(Shutdown::Both);
            }
        }
        // Wake the blocking accept: no timer or periodic wakeups while idle.
        if TcpStream::connect_timeout(&self.address, Duration::from_millis(50)).is_err() {
            // Resource exhaustion must not deadlock app shutdown on accept().
            // The worker owns its resources and exits on the next connection.
            return;
        }
        {
            let mut worker = self
                .worker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(worker) = worker.take() {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !worker.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(2));
                }
                if worker.is_finished() {
                    let _ = worker.join();
                }
            }
        }
    }
}

fn serve(
    listener: TcpListener,
    recordings_dir: PathBuf,
    token: String,
    shutdown: Arc<AtomicBool>,
    connections: Connections,
    io: WorkerIo,
    ready: mpsc::SyncSender<bool>,
) {
    let (send, receive) = mpsc::sync_channel::<TcpStream>(MAX_CONNECTIONS);
    let receive = Arc::new(Mutex::new(receive));
    let mut workers = Vec::new();
    for slot in 0..MAX_CONNECTIONS {
        let receive = receive.clone();
        let connections = connections.clone();
        let shutdown = shutdown.clone();
        let directory = recordings_dir.clone();
        let token = token.clone();
        let result = (io.spawn)(Box::new(move || loop {
            let stream = {
                let receiver = receive
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                receiver.recv()
            };
            let Ok(stream) = stream else {
                break;
            };
            // If duplicating the socket fails, close it rather than handing an
            // uninterruptible connection to a worker during resource exhaustion.
            if let Ok(copy) = (io.duplicate)(&stream) {
                {
                    let mut active = connections
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if shutdown.load(Ordering::Acquire) {
                        break;
                    }
                    active[slot] = Some(copy);
                }
                let _ = handle_connection(stream, &directory, &token);
                {
                    let mut active = connections
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    active[slot] = None;
                }
            }
        }));
        match result {
            Ok(worker) => workers.push(worker),
            Err(_) => break,
        }
    }
    if workers.len() != MAX_CONNECTIONS || ready.send(true).is_err() {
        drop(send);
        for worker in workers {
            let _ = worker.join();
        }
        return;
    }
    while !shutdown.load(Ordering::Acquire) {
        match (io.accept)(&listener) {
            Ok((stream, peer)) => {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                if !peer.ip().is_loopback() {
                    continue;
                }
                // Admission happens before parsing/authentication; excess sockets
                // are closed, never allowed to allocate additional worker threads.
                let _ = send.try_send(stream);
            }
            Err(error) => {
                log::warn!("Recording playback listener stopped: {error}");
                break;
            }
        }
    }
    drop(send);
    for worker in workers {
        let _ = worker.join();
    }
}

#[derive(Debug)]
struct MediaRequest {
    method: String,
    path: String,
    range: Option<String>,
}

fn handle_connection(
    mut stream: TcpStream,
    recordings_dir: &Path,
    expected_token: &str,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let request = match read_request(&stream) {
        Ok(request) => request,
        Err(_) => return write_empty(&mut stream, 400, "Bad Request", None),
    };
    if request.method != "GET" && request.method != "HEAD" {
        return write_empty(&mut stream, 405, "Method Not Allowed", None);
    }

    let path = request.path.split('?').next().unwrap_or(&request.path);
    let mut components = path.trim_start_matches('/').split('/');
    let token = components.next().unwrap_or_default();
    let request_id = components.next().unwrap_or_default();
    if token != expected_token
        || components.next().is_some()
        || !RecordingStore::is_safe_request_id(request_id)
    {
        return write_empty(&mut stream, 404, "Not Found", None);
    }

    let path = recordings_dir.join(format!("{request_id}.wav"));
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return write_empty(&mut stream, 404, "Not Found", None);
        }
        Err(_) => return write_empty(&mut stream, 500, "Internal Server Error", None),
    };
    let length = file.metadata()?.len();
    if length == 0 {
        return write_empty(&mut stream, 404, "Not Found", None);
    }

    let range = match request.range.as_deref() {
        Some(value) => match parse_range(value, length) {
            Some(range) => Some(range),
            None => {
                return write_empty(
                    &mut stream,
                    416,
                    "Range Not Satisfiable",
                    Some(format!("Content-Range: bytes */{length}\r\n")),
                );
            }
        },
        None => None,
    };

    let (status, reason, start, response_length, content_range) = match range {
        Some((start, requested_end)) => {
            let end = requested_end.min(start.saturating_add(MAX_RANGE_BYTES - 1).min(length - 1));
            (
                206,
                "Partial Content",
                start,
                end - start + 1,
                Some(format!("Content-Range: bytes {start}-{end}/{length}\r\n")),
            )
        }
        None => (200, "OK", 0, length, None),
    };

    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: audio/wav\r\n\
         Content-Length: {response_length}\r\n\
         Accept-Ranges: bytes\r\n\
         Cache-Control: private, no-store\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Cross-Origin-Resource-Policy: cross-origin\r\n\
         Connection: close\r\n"
    )?;
    if let Some(content_range) = content_range {
        stream.write_all(content_range.as_bytes())?;
    }
    stream.write_all(b"\r\n")?;
    if request.method == "HEAD" {
        return stream.flush();
    }

    file.seek(SeekFrom::Start(start))?;
    std::io::copy(&mut file.take(response_length), &mut stream)?;
    stream.flush()
}

fn read_request(stream: &TcpStream) -> Result<MediaRequest, String> {
    let mut reader = BufReader::new(HeaderReader {
        stream,
        deadline: Instant::now() + Duration::from_secs(3),
    });
    let mut first_line = String::new();
    read_bounded_line(&mut reader, &mut first_line)?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().ok_or("Missing method")?.to_string();
    let path = parts.next().ok_or("Missing path")?.to_string();
    if parts.next().is_none() || parts.next().is_some() {
        return Err("Invalid request line".to_string());
    }

    let mut total = first_line.len();
    let mut range = None;
    loop {
        let mut line = String::new();
        read_bounded_line(&mut reader, &mut line)?;
        total = total.saturating_add(line.len());
        if total > MAX_HEADER_BYTES {
            return Err("Headers too large".to_string());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_string());
            }
        }
    }
    Ok(MediaRequest {
        method,
        path,
        range,
    })
}

struct HeaderReader<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}
impl Read for HeaderReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(std::io::ErrorKind::TimedOut)?;
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(bytes)
    }
}

fn read_bounded_line(reader: &mut impl BufRead, line: &mut String) -> Result<(), String> {
    // Bound allocation before reading, not after an arbitrary-length line.
    let bytes = reader
        .take(MAX_HEADER_BYTES as u64 + 1)
        .read_line(line)
        .map_err(|error| error.to_string())?;
    if bytes == 0 || line.len() > MAX_HEADER_BYTES {
        return Err("Invalid request".to_string());
    }
    Ok(())
}

fn write_empty(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra_headers: Option<String>,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Length: 0\r\n\
         Cache-Control: private, no-store\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Cross-Origin-Resource-Policy: cross-origin\r\n\
         Connection: close\r\n"
    )?;
    if let Some(headers) = extra_headers {
        stream.write_all(headers.as_bytes())?;
    }
    stream.write_all(b"\r\n")?;
    stream.flush()
}

fn parse_range(value: &str, length: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        return Some((length.saturating_sub(suffix.min(length)), length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= length {
        return None;
    }
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (end >= start).then_some((start, end))
}

#[cfg(test)]
#[path = "recording_media_protocol/tests.rs"]
mod tests;
