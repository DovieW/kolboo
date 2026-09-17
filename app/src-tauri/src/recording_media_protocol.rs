//! Private, range-capable playback for recordings owned by `RecordingStore`.
//!
//! The URL contains only a recording id. Filesystem paths never enter the
//! webview, and the handler cannot serve files outside the recording store.

use crate::recordings::RecordingStore;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tauri::http::{
    header::{
        ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
        CONTENT_TYPE,
    },
    Method, Request, Response, StatusCode,
};
use tauri::{AppHandle, Manager, Runtime};

const MAX_RANGE_BYTES: u64 = 1024 * 1024;

pub fn respond<R: Runtime>(app: &AppHandle<R>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(store) = app.try_state::<RecordingStore>() else {
        return empty_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    response_from_store(&store, &request)
}

fn response_from_store(store: &RecordingStore, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }

    let request_id = request.uri().path().trim_start_matches('/');
    let path = match store.wav_path_if_exists(request_id) {
        Ok(Some(path)) => path,
        Ok(None) => return empty_response(StatusCode::NOT_FOUND),
        Err(_) => return empty_response(StatusCode::BAD_REQUEST),
    };
    let length = match std::fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(_) => return empty_response(StatusCode::NOT_FOUND),
    };
    if length == 0 {
        return empty_response(StatusCode::NOT_FOUND);
    }

    let range = match request.headers().get("range") {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| parse_range(value, length))
        {
            Some(range) => Some(range),
            None => return unsatisfiable_response(length),
        },
        None => None,
    };

    let mut builder = Response::builder()
        .header(CONTENT_TYPE, "audio/wav")
        .header(ACCEPT_RANGES, "bytes")
        .header(CACHE_CONTROL, "private, no-store")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*");

    if let Some((start, requested_end)) = range {
        let end = requested_end.min(start.saturating_add(MAX_RANGE_BYTES - 1).min(length - 1));
        let response_length = end - start + 1;
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(CONTENT_RANGE, format!("bytes {start}-{end}/{length}"))
            .header(CONTENT_LENGTH, response_length);
        if request.method() == Method::HEAD {
            return builder
                .body(Vec::new())
                .unwrap_or_else(|_| empty_response(StatusCode::INTERNAL_SERVER_ERROR));
        }
        let body = match read_range(&path, start, response_length) {
            Ok(body) => body,
            Err(_) => return empty_response(StatusCode::INTERNAL_SERVER_ERROR),
        };
        return builder
            .body(body)
            .unwrap_or_else(|_| empty_response(StatusCode::INTERNAL_SERVER_ERROR));
    }

    builder = builder.header(CONTENT_LENGTH, length);
    let body = if request.method() == Method::HEAD {
        Vec::new()
    } else {
        match std::fs::read(path) {
            Ok(body) => body,
            Err(_) => return empty_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    };
    builder
        .body(body)
        .unwrap_or_else(|_| empty_response(StatusCode::INTERNAL_SERVER_ERROR))
}

fn read_range(path: &std::path::Path, start: u64, length: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut body = Vec::with_capacity(length as usize);
    file.take(length).read_to_end(&mut body)?;
    Ok(body)
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

fn empty_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CACHE_CONTROL, "private, no-store")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CONTENT_LENGTH, 0)
        .body(Vec::new())
        .expect("static media response is valid")
}

fn unsatisfiable_response(length: u64) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(CONTENT_RANGE, format!("bytes */{length}"))
        .header(ACCEPT_RANGES, "bytes")
        .header(CACHE_CONTROL, "private, no-store")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CONTENT_LENGTH, 0)
        .body(Vec::new())
        .expect("static media response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: Method, range: Option<&str>) -> Request<Vec<u8>> {
        let mut builder = Request::builder()
            .method(method)
            .uri("kolboo-media://localhost/recording-1");
        if let Some(range) = range {
            builder = builder.header("range", range);
        }
        builder.body(Vec::new()).unwrap()
    }

    #[test]
    fn serves_bounded_ranges_and_head_without_exposing_other_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = RecordingStore::new(temp.path().to_owned());
        let bytes = (0_u8..=31).collect::<Vec<_>>();
        store.save_wav("recording-1", &bytes).unwrap();

        let response = response_from_store(&store, &request(Method::GET, Some("bytes=4-9")));
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(response.headers()[CONTENT_RANGE], "bytes 4-9/32");
        assert_eq!(response.body(), &bytes[4..=9]);

        let response = response_from_store(&store, &request(Method::HEAD, None));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_LENGTH], "32");
        assert!(response.body().is_empty());

        let missing = Request::builder()
            .uri("kolboo-media://localhost/../settings.json")
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            response_from_store(&store, &missing).status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn rejects_invalid_and_multiple_ranges() {
        assert_eq!(parse_range("bytes=10-4", 20), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 20), None);
        assert_eq!(parse_range("bytes=-4", 20), Some((16, 19)));
        assert_eq!(parse_range("bytes=18-", 20), Some((18, 19)));
    }
}
