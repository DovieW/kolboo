//! Pure prompt-formatting helpers for LLM-backed text flows.
//!
//! Keep this Module boring and side-effect free: callers decide which context sources are allowed,
//! bound text for logs/prompts, and pick providers. This Module only formats already-approved text
//! into the user messages expected by rewrite, Quick Ask, and Quick Replace flows.

/// Build a user message for the rewrite step that optionally includes clipboard + OCR context.
pub(crate) fn build_rewrite_user_message(
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
#[allow(dead_code)]
pub(crate) fn build_quick_ask_user_message(question: &str, clipboard_text: Option<&str>) -> String {
    // Keep this compatibility wrapper tiny. The richer context-aware builder below is the primary
    // Interface used by current Quick Ask execution, but this shape is still handy for callers that
    // only have clipboard context.
    build_quick_ask_user_message_with_context(question, None, None, clipboard_text, None)
}

/// Build a user message for Quick Ask that can include highlighted selection text and/or
/// clipboard text as optional context.
///
/// This keeps backward compatibility with older UIs that only supported clipboard context.
pub(crate) fn build_quick_ask_user_message_with_context(
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
/// Keep this exact prompt shape stable: Quick Replace's execution Module owns lifecycle and
/// provider selection, while this prompt Module characterizes only the string we send to the LLM.
pub(crate) fn build_quick_replace_user_message(
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
        // Privacy invariant: this builder only formats already-bounded context supplied by the
        // caller; it does not read clipboard/OCR/secrets itself.
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
        // This exact prompt shape is intentionally captured so Quick Replace rewrites remain stable
        // while ownership boundaries around context collection and normal output continue to evolve.
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
}
