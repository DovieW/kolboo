use super::types::{WindowsTextContext, WindowsTextContextSource};

#[cfg(target_os = "windows")]
use crate::windows_uia::client::UiaClient;
#[cfg(target_os = "windows")]
use crate::windows_uia::com::initialize_com_mta;
#[cfg(target_os = "windows")]
use crate::windows_uia::safety::allow_context_capture;
#[cfg(target_os = "windows")]
use crate::windows_uia::snapshot::capture_snapshot;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Accessibility::{
    IUIAutomationTextPattern, IUIAutomationTextRange, UIA_TextPatternId,
};

#[cfg(target_os = "windows")]
use std::time::SystemTime;

pub fn truncate_with_flag(value: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), value.chars().count() > 0);
    }

    let mut chars = value.chars();
    let mut out = String::new();
    let mut count = 0usize;
    let mut truncated = false;

    while let Some(ch) = chars.next() {
        if count >= max_chars {
            truncated = true;
            break;
        }
        out.push(ch);
        count += 1;
    }

    if chars.next().is_some() {
        truncated = true;
    }

    (out, truncated)
}

fn is_accessibility_placeholder(value: &str) -> bool {
    let lowered = value.trim().to_lowercase();
    lowered.contains("editor is not accessible")
        && lowered.contains("screen reader")
        && lowered.contains("shift+alt+f1")
}

pub fn build_text_context(
    selection_text: Option<String>,
    surrounding_text: Option<String>,
    source: WindowsTextContextSource,
    max_chars: usize,
) -> WindowsTextContext {
    let mut truncated = false;
    let selection = selection_text.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() || is_accessibility_placeholder(trimmed) {
            None
        } else {
            let (next, was_truncated) = truncate_with_flag(trimmed, max_chars);
            truncated |= was_truncated;
            Some(next)
        }
    });

    let surrounding = surrounding_text.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() || is_accessibility_placeholder(trimmed) {
            None
        } else {
            let (next, was_truncated) = truncate_with_flag(trimmed, max_chars);
            truncated |= was_truncated;
            Some(next)
        }
    });

    WindowsTextContext {
        selection_text: selection,
        surrounding_text: surrounding,
        source,
        truncated,
        max_chars,
    }
}

#[cfg(target_os = "windows")]
fn clamp_text_request_len(max_chars: usize) -> i32 {
    let requested = max_chars.saturating_add(1);
    let capped = requested.min(i32::MAX as usize);
    capped as i32
}

#[cfg(target_os = "windows")]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn text_from_range(range: &IUIAutomationTextRange, max_chars: usize) -> Option<String> {
    let requested = clamp_text_request_len(max_chars);
    let text = unsafe { range.GetText(requested) }.ok()?;
    let text = text.to_string();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(target_os = "windows")]
fn selection_text_from_pattern(
    pattern: &IUIAutomationTextPattern,
    max_chars: usize,
) -> Option<String> {
    let ranges = unsafe { pattern.GetSelection() }.ok()?;
    let len = unsafe { ranges.Length() }.ok()?;
    if len <= 0 {
        return None;
    }
    let range = unsafe { ranges.GetElement(0) }.ok()?;
    text_from_range(&range, max_chars)
}

#[cfg(target_os = "windows")]
fn surrounding_text_from_pattern(
    pattern: &IUIAutomationTextPattern,
    max_chars: usize,
) -> Option<String> {
    let range = unsafe { pattern.DocumentRange() }.ok()?;
    text_from_range(&range, max_chars)
}

/// Capture selection + surrounding context via UIA TextPattern when available.
#[cfg(target_os = "windows")]
pub fn capture_focused_text_context(
    max_chars: usize,
) -> Result<Option<WindowsTextContext>, String> {
    log::debug!("UIA context capture: start (max_chars={})", max_chars);
    let _guard = initialize_com_mta()?;
    let client = UiaClient::new()?;
    let element = client.get_focused_element_with_retry(2, 40)?;

    let snapshot = capture_snapshot(&element, now_ms())?;
    if !allow_context_capture(&snapshot) {
        log::info!("UIA context capture: blocked by safety policy");
        return Ok(None);
    }

    let pattern: IUIAutomationTextPattern =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
            .map_err(|err| format!("UIA TextPattern unavailable: {err}"))?;

    let selection_text = selection_text_from_pattern(&pattern, max_chars);
    let surrounding_text = surrounding_text_from_pattern(&pattern, max_chars);

    let context = build_text_context(
        selection_text,
        surrounding_text,
        WindowsTextContextSource::Uia,
        max_chars,
    );

    log::debug!(
        "UIA context capture: done (selection_len={}, surrounding_len={}, truncated={})",
        context
            .selection_text
            .as_ref()
            .map(|s| s.chars().count())
            .unwrap_or(0),
        context
            .surrounding_text
            .as_ref()
            .map(|s| s.chars().count())
            .unwrap_or(0),
        context.truncated
    );

    if context.selection_text.is_none() && context.surrounding_text.is_none() {
        return Ok(None);
    }

    Ok(Some(context))
}

#[cfg(not(target_os = "windows"))]
pub fn capture_focused_text_context(
    _max_chars: usize,
) -> Result<Option<WindowsTextContext>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{build_text_context, truncate_with_flag};
    use crate::windows_uia::types::WindowsTextContextSource;

    #[test]
    fn truncate_with_flag_marks_truncation() {
        let (out, truncated) = truncate_with_flag("abcdef", 4);
        assert_eq!(out, "abcd");
        assert!(truncated);
    }

    #[test]
    fn build_text_context_truncates_both_fields() {
        let context = build_text_context(
            Some("hello world".to_string()),
            Some("surrounding text".to_string()),
            WindowsTextContextSource::Uia,
            5,
        );

        assert_eq!(context.selection_text.as_deref(), Some("hello"));
        assert_eq!(context.surrounding_text.as_deref(), Some("surro"));
        assert!(context.truncated);
        assert_eq!(context.max_chars, 5);
    }

    #[test]
    fn build_text_context_preserves_source() {
        let context = build_text_context(
            Some("hi".to_string()),
            None,
            WindowsTextContextSource::Uia,
            10,
        );
        assert_eq!(context.source, WindowsTextContextSource::Uia);
    }
}
