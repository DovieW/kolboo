pub mod openai_compatible;

pub const OCR_PROMPT_DEFAULT: &str = "";
pub const OCR_MAX_TOKENS_DEFAULT: u32 = 512;
pub const OCR_TEMPERATURE_DEFAULT: f64 = 0.0;
pub const OCR_TOP_P_DEFAULT: f64 = 1.0;

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    pub provider: String,
    pub model: String,
}

fn normalize_ocr_text(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn truncate_ocr_text(raw: &str, max_chars: usize) -> (String, bool) {
    let normalized = normalize_ocr_text(raw);
    let trimmed = normalized.trim();
    if max_chars == 0 || trimmed.is_empty() {
        return (String::new(), trimmed.is_empty());
    }

    let total_chars = trimmed.chars().count();
    if total_chars <= max_chars {
        return (trimmed.to_string(), false);
    }

    let suffix = "… (truncated)";
    let suffix_chars = suffix.chars().count();
    let keep_chars = max_chars.saturating_sub(suffix_chars).max(1);
    let mut out = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= keep_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str(suffix);
    (out, true)
}

pub fn build_labeled_ocr_context(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    format!(
        "OCR context from the currently active window:\n\n{}",
        trimmed
    )
}
