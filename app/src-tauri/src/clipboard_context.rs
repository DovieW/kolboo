// Shared helpers for including clipboard text as optional context in LLM requests.
//
// This is intentionally separate from the selection "grab method" logic in
// `commands/text.rs`: here we only *read* clipboard text to provide extra context
// when the user explicitly enables it.

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
    match tokio::task::spawn_blocking(move || read_clipboard_text_best_effort(max_chars)).await {
        Ok(v) => v,
        Err(_) => None,
    }
}

#[cfg(not(desktop))]
pub async fn read_clipboard_text_best_effort_async(_max_chars: usize) -> Option<String> {
    None
}

/// Build a user message for the rewrite step that optionally includes clipboard context.
pub fn build_rewrite_user_message(transcript: &str, clipboard_text: Option<&str>) -> String {
    if let Some(cb) = clipboard_text {
        // Keep the transcript prominent. Clipboard is explicitly labeled as context.
        format!(
            "Transcript:\n{}\n\nClipboard context:\n{}",
            transcript.trim(),
            cb.trim()
        )
    } else {
        transcript.to_string()
    }
}

/// Build a user message for Quick Ask that optionally includes clipboard context.
pub fn build_quick_ask_user_message(question: &str, clipboard_text: Option<&str>) -> String {
    build_quick_ask_user_message_with_context(question, None, clipboard_text)
}

/// Build a user message for Quick Ask that can include highlighted selection text and/or
/// clipboard text as optional context.
///
/// This keeps backward compatibility with older UIs that only supported clipboard context.
pub fn build_quick_ask_user_message_with_context(
    question: &str,
    selected_text: Option<&str>,
    clipboard_text: Option<&str>,
) -> String {
    let question = question.trim();
    let selected_text = selected_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let clipboard_text = clipboard_text.map(|s| s.trim()).filter(|s| !s.is_empty());

    if selected_text.is_none() && clipboard_text.is_none() {
        return question.to_string();
    }

    let mut out = format!("Question:\n{}", question);

    if let Some(sel) = selected_text {
        out.push_str("\n\nSelected text:\n");
        out.push_str(sel);
    }

    if let Some(cb) = clipboard_text {
        out.push_str("\n\nClipboard context:\n");
        out.push_str(cb);
    }

    out
}
