use std::time::Duration;

pub use crate::http::{join_base_url, trim_base_url};

/// Default STT HTTP timeout used by provider constructors.
///
/// Note: the pipeline typically injects its own client (with proxy settings) via `with_client(...)`.
/// This constant is mainly for provider `new(...)` constructors and tests.
#[allow(dead_code)]
pub const DEFAULT_STT_HTTP_TIMEOUT: Duration = Duration::from_secs(60);
