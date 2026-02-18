use super::{ManagedError, ManagedErrorCategory};

#[allow(dead_code)]
pub fn map_http_status_to_managed_error(status: u16, message: impl Into<String>) -> ManagedError {
    let category = match status {
        401 | 403 => ManagedErrorCategory::Unauthorized,
        402 | 429 => ManagedErrorCategory::OverQuota,
        409 | 412 => ManagedErrorCategory::Ineligible,
        _ => ManagedErrorCategory::TemporarilyUnavailable,
    };

    ManagedError {
        category,
        code: format!("HTTP_{status}"),
        message: message.into(),
        request_id: None,
        retry_after_seconds: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_429_to_over_quota() {
        let err = map_http_status_to_managed_error(429, "quota reached");
        assert_eq!(err.category, ManagedErrorCategory::OverQuota);
        assert_eq!(err.code, "HTTP_429");
    }

    #[test]
    fn maps_403_to_unauthorized() {
        let err = map_http_status_to_managed_error(403, "forbidden");
        assert_eq!(err.category, ManagedErrorCategory::Unauthorized);
    }
}
