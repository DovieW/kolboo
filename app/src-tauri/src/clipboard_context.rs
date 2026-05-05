// Shared helpers for reading clipboard text and bounding context for LLM requests.
//
// This is intentionally separate from the selection "grab method" logic in
// `commands/text.rs`: here we only *read* clipboard text to provide extra context
// when the user explicitly enables it. Prompt formatting lives in `prompt_builders.rs`
// so transport/capping and message assembly do not blur together again.

#![allow(dead_code)]

#[cfg(desktop)]
use arboard::Clipboard;

/// Truncate a string to at most `max_chars` characters, appending an ellipsis when truncated.
fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn truncate_utf8_to_byte_cap(s: &str, cap_bytes: usize) -> &str {
    if s.len() <= cap_bytes {
        return s;
    }

    let mut idx = cap_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }

    &s[..idx]
}

/// Trim and bound context text before it is attached to prompts or logs.
///
/// This deliberately preserves the existing byte-cap behavior from `lib.rs` so
/// the Context Capture seam gets better Locality without changing prompt/log
/// output. Callers should pass already-approved context sources; this helper
/// only bounds text, it never reads clipboard, OCR, or OS selection state.
pub fn cap_context_text_for_prompt(text: &str, cap_bytes: usize) -> String {
    let text = text.trim();
    if text.len() > cap_bytes {
        format!(
            "{}\n\n… (truncated)",
            truncate_utf8_to_byte_cap(text, cap_bytes)
        )
    } else {
        text.to_string()
    }
}

pub fn cap_optional_context_text_for_prompt(
    text: Option<&str>,
    cap_bytes: usize,
) -> Option<String> {
    text.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| cap_context_text_for_prompt(s, cap_bytes))
}

fn normalize_clipboard_text(raw: &str) -> String {
    // Normalize newlines for more predictable prompts.
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// Read clipboard text (best-effort). Returns `None` when clipboard is unavailable, busy,
/// or contains no (non-whitespace) text.
///
/// This is a blocking call; prefer using `read_clipboard_text_best_effort_async` from async code.
#[cfg(desktop)]
pub fn read_clipboard_text_best_effort(max_chars: usize) -> Option<String> {
    let mut clipboard = Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let text = normalize_clipboard_text(&text);
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    Some(truncate_with_ellipsis(text, max_chars))
}

#[cfg(not(desktop))]
pub fn read_clipboard_text_best_effort(_max_chars: usize) -> Option<String> {
    None
}

/// Async wrapper for `read_clipboard_text_best_effort` that avoids blocking the async runtime.
#[cfg(desktop)]
pub async fn read_clipboard_text_best_effort_async(max_chars: usize) -> Option<String> {
    tokio::task::spawn_blocking(move || read_clipboard_text_best_effort(max_chars))
        .await
        .unwrap_or_default()
}

#[cfg(not(desktop))]
pub async fn read_clipboard_text_best_effort_async(_max_chars: usize) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_normalization_and_truncation_are_deterministic() {
        assert_eq!(normalize_clipboard_text("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(truncate_with_ellipsis("abcdef", 3), "abc…");
        assert_eq!(truncate_with_ellipsis("abcdef", 0), "");
    }

    #[test]
    fn cap_context_text_preserves_existing_truncated_suffix() {
        assert_eq!(
            cap_context_text_for_prompt("  abcdef  ", 3),
            "abc\n\n… (truncated)"
        );
        assert_eq!(cap_context_text_for_prompt("  abc  ", 10), "abc");
        assert_eq!(
            cap_optional_context_text_for_prompt(Some("  \n\t  "), 10),
            None
        );
    }

    #[test]
    fn cap_context_text_never_splits_utf8_codepoints() {
        // The cap lands in the middle of "é"; we should truncate before it.
        assert_eq!(cap_context_text_for_prompt("aébc", 2), "a\n\n… (truncated)");
    }
}
