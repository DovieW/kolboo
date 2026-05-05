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

/// Build a user message for the rewrite step that optionally includes clipboard + OCR context.
pub fn build_rewrite_user_message(
    transcript: &str,
    clipboard_text: Option<&str>,
    ocr_text: Option<&str>,
) -> String {
    let transcript = transcript.trim();
    let clipboard_text = clipboard_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let ocr_text = ocr_text.map(|s| s.trim()).filter(|s| !s.is_empty());

    if clipboard_text.is_none() && ocr_text.is_none() {
        return transcript.to_string();
    }

    let mut out = format!("Transcript:\n{}", transcript);

    if let Some(cb) = clipboard_text {
        out.push_str("\n\nClipboard context:\n");
        out.push_str(cb);
    }

    if let Some(ocr) = ocr_text {
        let labeled = crate::ocr::build_labeled_ocr_context(ocr);
        if !labeled.is_empty() {
            out.push_str("\n\n");
            out.push_str(labeled.as_str());
        }
    }

    out
}

/// Build a user message for Quick Ask that optionally includes clipboard context.
pub fn build_quick_ask_user_message(question: &str, clipboard_text: Option<&str>) -> String {
    build_quick_ask_user_message_with_context(question, None, None, clipboard_text, None)
}

/// Build a user message for Quick Ask that can include highlighted selection text and/or
/// clipboard text as optional context.
///
/// This keeps backward compatibility with older UIs that only supported clipboard context.
pub fn build_quick_ask_user_message_with_context(
    question: &str,
    selected_text: Option<&str>,
    surrounding_text: Option<&str>,
    clipboard_text: Option<&str>,
    ocr_text: Option<&str>,
) -> String {
    let question = question.trim();
    let selected_text = selected_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let surrounding_text = surrounding_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let clipboard_text = clipboard_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let ocr_text = ocr_text.map(|s| s.trim()).filter(|s| !s.is_empty());

    if selected_text.is_none()
        && surrounding_text.is_none()
        && clipboard_text.is_none()
        && ocr_text.is_none()
    {
        return question.to_string();
    }

    let mut out = format!("Question:\n{}", question);

    if let Some(sel) = selected_text {
        out.push_str("\n\nSelected text:\n");
        out.push_str(sel);
    }

    if let Some(surrounding) = surrounding_text {
        out.push_str("\n\nSurrounding text:\n");
        out.push_str(surrounding);
    }

    if let Some(cb) = clipboard_text {
        out.push_str("\n\nClipboard context:\n");
        out.push_str(cb);
    }

    if let Some(ocr) = ocr_text {
        let labeled = crate::ocr::build_labeled_ocr_context(ocr);
        if !labeled.is_empty() {
            out.push_str("\n\n");
            out.push_str(labeled.as_str());
        }
    }

    out
}

/// Build the Quick Replace LLM user prompt from structured context.
///
/// Keep this tiny and exact: Quick Replace's caller still owns lifecycle and
/// provider selection, while this helper characterizes the prompt string before
/// larger request-ownership refactors move more code out of `lib.rs`.
pub fn build_quick_replace_user_message(
    instructions: &str,
    selected_text: &str,
    surrounding_text: Option<&str>,
    clipboard_text: Option<&str>,
    ocr_text: Option<&str>,
) -> String {
    let instructions = instructions.trim();
    let selected_text = selected_text.trim();
    let surrounding_text = surrounding_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let clipboard_text = clipboard_text.map(|s| s.trim()).filter(|s| !s.is_empty());
    let ocr_text = ocr_text.map(|s| s.trim()).filter(|s| !s.is_empty());

    let mut out = format!(
        "INSTRUCTIONS:\n{}\n\nSELECTED TEXT:\n{}",
        instructions, selected_text
    );

    if let Some(surrounding) = surrounding_text {
        out.push_str("\n\nSURROUNDING TEXT:\n");
        out.push_str(surrounding);
    }

    if let Some(cb) = clipboard_text {
        out.push_str("\n\nCLIPBOARD CONTEXT:\n");
        out.push_str(cb);
    }

    if let Some(ocr) = ocr_text {
        let labeled = crate::ocr::build_labeled_ocr_context(ocr);
        if !labeled.is_empty() {
            out.push_str("\n\n");
            out.push_str(labeled.as_str());
        }
    }

    out.push_str("\n\nReturn only the updated text.");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_prompt_returns_trimmed_transcript_without_context() {
        assert_eq!(
            build_rewrite_user_message("  hello world  ", None, None),
            "hello world"
        );
    }

    #[test]
    fn rewrite_prompt_preserves_clipboard_and_ocr_context_order() {
        let prompt = build_rewrite_user_message(
            "  make it shorter  ",
            Some("  clipboard note  "),
            Some("  active window text  "),
        );

        assert_eq!(
            prompt,
            "Transcript:\nmake it shorter\n\nClipboard context:\nclipboard note\n\nOCR context from the currently active window:\n\nactive window text"
        );
    }

    #[test]
    fn quick_ask_prompt_returns_trimmed_question_without_context() {
        assert_eq!(
            build_quick_ask_user_message_with_context("  what changed?  ", None, None, None, None),
            "what changed?"
        );
    }

    #[test]
    fn quick_ask_prompt_characterizes_all_context_sections() {
        // Privacy invariant: this builder only formats already-bounded context
        // supplied by the caller; it does not read clipboard/OCR/secrets itself.
        let prompt = build_quick_ask_user_message_with_context(
            "  summarize this  ",
            Some(" selected sentence "),
            Some(" surrounding paragraph "),
            Some(" clipboard fact "),
            Some(" screen label "),
        );

        assert_eq!(
            prompt,
            "Question:\nsummarize this\n\nSelected text:\nselected sentence\n\nSurrounding text:\nsurrounding paragraph\n\nClipboard context:\nclipboard fact\n\nOCR context from the currently active window:\n\nscreen label"
        );
    }

    #[test]
    fn quick_ask_prompt_ignores_empty_context_sections() {
        let prompt = build_quick_ask_user_message_with_context(
            "question",
            Some("   "),
            Some("around"),
            Some("\n\t"),
            Some(""),
        );

        assert_eq!(prompt, "Question:\nquestion\n\nSurrounding text:\naround");
    }

    #[test]
    fn quick_replace_prompt_characterizes_current_inline_format() {
        // This exact prompt shape is intentionally captured before the Quick
        // Replace request lifecycle moves out of `lib.rs`.
        let prompt = build_quick_replace_user_message(
            "  make this friendlier  ",
            "  Dear user, no.  ",
            Some("  The previous sentence was formal.  "),
            Some("  Use a warm tone.  "),
            Some("  active document title  "),
        );

        assert_eq!(
            prompt,
            "INSTRUCTIONS:\nmake this friendlier\n\nSELECTED TEXT:\nDear user, no.\n\nSURROUNDING TEXT:\nThe previous sentence was formal.\n\nCLIPBOARD CONTEXT:\nUse a warm tone.\n\nOCR context from the currently active window:\n\nactive document title\n\nReturn only the updated text."
        );
    }

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
