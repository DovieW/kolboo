#[derive(Debug, Clone)]
pub struct VerificationInput {
    pub method_error: Option<String>,
    pub target_matches: bool,
    pub timed_out: bool,
    pub clipboard_restored: Option<bool>,
}

pub fn verify_or_fallback(input: VerificationInput) -> Result<(), String> {
    if let Some(err) = input.method_error {
        return Err(err);
    }

    if input.timed_out {
        return Err("Insertion timed out".to_string());
    }

    if !input.target_matches {
        return Err("Insertion target mismatch".to_string());
    }

    if let Some(restored) = input.clipboard_restored {
        if !restored {
            return Err("Clipboard restore validation failed".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn succeeds_when_all_signals_ok() {
        let input = VerificationInput {
            method_error: None,
            target_matches: true,
            timed_out: false,
            clipboard_restored: Some(true),
        };
        assert!(verify_or_fallback(input).is_ok());
    }

    #[test]
    fn fails_on_method_error() {
        let input = VerificationInput {
            method_error: Some("boom".to_string()),
            target_matches: true,
            timed_out: false,
            clipboard_restored: None,
        };
        assert!(verify_or_fallback(input).is_err());
    }

    #[test]
    fn fails_on_timeout() {
        let input = VerificationInput {
            method_error: None,
            target_matches: true,
            timed_out: true,
            clipboard_restored: None,
        };
        assert!(verify_or_fallback(input).is_err());
    }

    #[test]
    fn fails_on_target_mismatch() {
        let input = VerificationInput {
            method_error: None,
            target_matches: false,
            timed_out: false,
            clipboard_restored: None,
        };
        assert!(verify_or_fallback(input).is_err());
    }

    #[test]
    fn fails_on_clipboard_restore_failure() {
        let input = VerificationInput {
            method_error: None,
            target_matches: true,
            timed_out: false,
            clipboard_restored: Some(false),
        };
        assert!(verify_or_fallback(input).is_err());
    }
}
