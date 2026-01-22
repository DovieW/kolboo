use std::io;
use std::sync::{Arc, Mutex};

use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

struct SharedBufWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedBufWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("buffer mutex poisoned")
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuf {
    type Writer = SharedBufWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SharedBufWriter(self.0.clone())
    }
}

#[test]
fn json_logs_include_request_id_from_span() {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let make_writer = SharedBuf(buf.clone());

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .with_writer(make_writer)
        .with_target(false)
        .json()
        .flatten_event(true)
        .with_current_span(true)
        .with_span_list(true)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("request", request_id = "abc123");
        let _enter = span.enter();

        tracing::info!("hello");
    });

    let locked = buf.lock().expect("buffer mutex poisoned");
    let out = String::from_utf8_lossy(&locked);
    assert!(
        out.contains("\"request_id\":\"abc123\""),
        "expected request_id to appear in JSON log output; got: {out}"
    );
}
