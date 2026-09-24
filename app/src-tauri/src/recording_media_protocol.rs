//! Private, range-capable playback for recordings owned by `RecordingStore`.
//!
//! WebKitGTK does not reliably hand custom-scheme responses to its media
//! pipeline. This process-local HTTP server gives the media element an ordinary
//! byte-range stream without exposing filesystem paths or accepting arbitrary
//! files. The random token changes on every app start.

use crate::recordings::RecordingStore;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_RANGE_BYTES: u64 = 1024 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;

pub struct RecordingMediaServer {
    address: SocketAddr,
    token: String,
    shutdown: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl RecordingMediaServer {
    pub fn start(recordings_dir: PathBuf) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("Could not start recording playback: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("Could not read recording playback address: {error}"))?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let worker_token = token.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let worker = thread::Builder::new()
            .name("recording-media".to_string())
            .spawn(move || {
                serve(listener, recordings_dir, worker_token, worker_shutdown);
            })
            .map_err(|error| format!("Could not start recording playback worker: {error}"))?;

        Ok(Self {
            address,
            token,
            shutdown,
            worker: Mutex::new(Some(worker)),
        })
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
        // Wake the blocking accept: no timer or periodic wakeups while idle.
        if TcpStream::connect_timeout(&self.address, Duration::from_millis(50)).is_err() {
            // Resource exhaustion must not deadlock app shutdown on accept().
            // The worker owns its resources and exits on the next connection.
            return;
        }
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn serve(listener: TcpListener, recordings_dir: PathBuf, token: String, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if shutdown.load(Ordering::Acquire) {
                    return;
                }
                if !peer.ip().is_loopback() {
                    continue;
                }
                let directory = recordings_dir.clone();
                let expected_token = token.clone();
                let _ = thread::Builder::new()
                    .name("recording-media-request".to_string())
                    .spawn(move || {
                        if let Err(error) = handle_connection(stream, &directory, &expected_token) {
                            log::debug!("Recording playback request ended: {error}");
                        }
                    });
            }
            Err(error) => {
                log::warn!("Recording playback listener stopped: {error}");
                return;
            }
        }
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
    let mut reader = BufReader::new(stream);
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

fn read_bounded_line(reader: &mut BufReader<&TcpStream>, line: &mut String) -> Result<(), String> {
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
