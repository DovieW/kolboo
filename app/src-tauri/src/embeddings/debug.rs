//! Shared helpers for embeddings providers.

/// Create a safe preview of user-provided input for debug logs.
///
/// Returns:
/// - total input length (in Unicode scalar values, i.e. `.chars().count()`)
/// - preview (possibly truncated)
/// - whether the preview was truncated
pub(super) fn input_preview(input: &str, max_chars: usize) -> (usize, String, bool) {
    let input_len = input.chars().count();
    let mut preview: String = input.chars().take(max_chars).collect();
    let truncated = input_len > max_chars;
    if truncated {
        preview.push('…');
    }
    (input_len, preview, truncated)
}

#[cfg(test)]
mod tests {
    use super::input_preview;

    #[test]
    fn input_preview_truncates_with_ellipsis() {
        let input = "abcdef";
        let (_len, preview, truncated) = input_preview(input, 3);
        assert_eq!(preview, "abc…");
        assert!(truncated);
    }

    #[test]
    fn input_preview_no_truncation_includes_full_string() {
        let input = "abc";
        let (len, preview, truncated) = input_preview(input, 3);
        assert_eq!(len, 3);
        assert_eq!(preview, "abc");
        assert!(!truncated);
    }
}
