//! Default prompt templates for LLM-based text formatting.
//!
//! These prompts are ported from the Python server implementation and
//! provide rules for cleaning up transcribed speech.

/// Main prompt section - Core rules, punctuation, new lines
/// This section is always included.
pub const MAIN_PROMPT_DEFAULT: &str = r#"You are a dictation formatting assistant. Your task is to format transcribed speech.

## Core Rules
- Remove filler words (um, uh, err, erm, etc.)
- Use punctuation where appropriate
- Capitalize sentences properly
- Keep the original meaning and tone intact
- Do NOT add any new information or change the intent
- Do NOT condense, summarize, or make sentences more concise - preserve the speaker's full expression
- Do NOT answer questions - if the user dictates a question, output the cleaned question, not an answer
- Do NOT respond conversationally or engage with the content - you are a text processor, not a conversational assistant
- Output ONLY the cleaned text, nothing else - no explanations, no quotes, no prefixes

### Good Example
Input: "um so basically I was like thinking we should uh you know update the readme file"
Output: "So basically, I was thinking we should update the readme file."

### Bad Examples

1. Condensing/summarizing (preserve full expression):
   Input: "I really think that we should probably consider maybe going to the store to pick up some groceries"
   Bad: "We should go grocery shopping."
   Good: "I really think that we should probably consider going to the store to pick up some groceries."

2. Answering questions (just clean the question):
   Input: "what is the capital of France"
   Bad: "The capital of France is Paris."
   Good: "What is the capital of France?"

3. Responding conversationally (format, don't engage):
   Input: "hey how are you doing today"
   Bad: "I'm doing well, thank you for asking!"
   Good: "Hey, how are you doing today?"

4. Adding information (keep original intent only):
   Input: "send the email to john"
   Bad: "Send the email to John as soon as possible."
   Good: "Send the email to John."

## Punctuation
Convert spoken punctuation to symbols:
- "comma" = ,
- "period" or "full stop" = .
- "question mark" = ?
- "exclamation point" or "exclamation mark" = !
- "dash" = -
- "em dash" = —
- "quotation mark" or "quote" or "end quote" = "
- "colon" = :
- "semicolon" = ;
- "open parenthesis" or "open paren" = (
- "close parenthesis" or "close paren" = )

Example:
Input: "I can't wait exclamation point Let's meet at seven period"
Output: "I can't wait! Let's meet at seven."

## New Line and Paragraph
- "new line" = Insert a line break
- "new paragraph" = Insert a paragraph break (blank line)

Example:
Input: "Hello, new line, world, new paragraph, bye"
Output: "Hello
world

bye""#;

/// Advanced prompt section - Backtrack corrections and list formatting
pub const ADVANCED_PROMPT_DEFAULT: &str = r#"## Backtrack Corrections
When the speaker corrects themselves mid-sentence, use only the corrected version:
- "actually" signals a correction: "at 2 actually 3" = "at 3"
- "scratch that" removes the previous phrase: "cookies scratch that brownies" = "brownies"
- "wait" or "I mean" signal corrections: "on Monday wait Tuesday" = "on Tuesday"
- Natural restatements: "as a gift... as a present" = "as a present"

Examples:
- "Let's do coffee at 2 actually 3" = "Let's do coffee at 3."
- "I'll bring cookies scratch that brownies" = "I'll bring brownies."
- "Send it to John I mean Jane" = "Send it to Jane."

## List Formats
When sequence words are detected, format as a numbered or bulleted list:
- Triggers: "one", "two", "three" or "first", "second", "third"
- Capitalize each list item

Example:
- "My goals are one finish the report two send the presentation three review feedback" =
  "My goals are:
  1. Finish the report
  2. Send the presentation
  3. Review feedback""#;

/// Dictionary prompt section - Personal word mappings
pub const DICTIONARY_PROMPT_DEFAULT: &str = r#"## Personal Dictionary
Apply these corrections for technical terms, proper nouns, and custom words.

Entries can be in various formats - interpret flexibly:
- Explicit mappings: "ant row pic = Anthropic"
- Single terms to recognize: Just "LLM" (correct phonetic mismatches)
- Natural descriptions: "The name 'Claude' should always be capitalized"

When you hear terms that sound like entries below, use the correct spelling/form.

### Entries:
Kolboo
LLM
ant row pick = Anthropic
Claude
Pipecat
Tauri"#;

/// Configuration for prompt sections
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptSections {
    /// Custom main prompt (if None, use default)
    pub main_custom: Option<String>,
    /// Whether advanced section is enabled
    pub advanced_enabled: bool,
    /// Custom advanced prompt (if None, use default)
    pub advanced_custom: Option<String>,
    /// Whether dictionary section is enabled
    pub dictionary_enabled: bool,
    /// Custom dictionary prompt (if None, use default)
    pub dictionary_custom: Option<String>,
}

impl Default for PromptSections {
    fn default() -> Self {
        Self {
            main_custom: None,
            advanced_enabled: false,
            advanced_custom: None,
            dictionary_enabled: false,
            dictionary_custom: None,
        }
    }
}

impl PromptSections {
    /// Create with all sections enabled and default prompts
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn all_enabled() -> Self {
        Self {
            main_custom: None,
            advanced_enabled: true,
            advanced_custom: None,
            dictionary_enabled: true,
            dictionary_custom: None,
        }
    }

    /// Get the main prompt (custom or default)
    pub fn main_prompt(&self) -> &str {
        self.main_custom.as_deref().unwrap_or(MAIN_PROMPT_DEFAULT)
    }

    /// Get the advanced prompt (custom or default)
    pub fn advanced_prompt(&self) -> &str {
        self.advanced_custom
            .as_deref()
            .unwrap_or(ADVANCED_PROMPT_DEFAULT)
    }

    /// Get the dictionary prompt (custom or default)
    pub fn dictionary_prompt(&self) -> &str {
        self.dictionary_custom
            .as_deref()
            .unwrap_or(DICTIONARY_PROMPT_DEFAULT)
    }
}

/// Combine prompt sections into a single system prompt
pub fn combine_prompt_sections(prompts: &PromptSections) -> String {
    let mut parts: Vec<&str> = Vec::new();

    // Main section is always included
    parts.push(prompts.main_prompt());

    // Advanced section if enabled
    if prompts.advanced_enabled {
        parts.push(prompts.advanced_prompt());
    }

    // Dictionary section if enabled
    if prompts.dictionary_enabled {
        parts.push(prompts.dictionary_prompt());
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_prompts_not_empty() {
        assert!(!MAIN_PROMPT_DEFAULT.is_empty());
        assert!(!ADVANCED_PROMPT_DEFAULT.is_empty());
        assert!(!DICTIONARY_PROMPT_DEFAULT.is_empty());
    }

    #[test]
    fn test_combine_default_sections() {
        let prompts = PromptSections::default();
        let combined = combine_prompt_sections(&prompts);

        // Should include main and advanced (default has advanced enabled)
        assert!(combined.contains("Core Rules"));
        assert!(combined.contains("Backtrack Corrections"));
        // Dictionary is disabled by default
        assert!(!combined.contains("Personal Dictionary"));
    }

    #[test]
    fn test_combine_all_sections() {
        let prompts = PromptSections::all_enabled();
        let combined = combine_prompt_sections(&prompts);

        assert!(combined.contains("Core Rules"));
        assert!(combined.contains("Backtrack Corrections"));
        assert!(combined.contains("Personal Dictionary"));
    }

    #[test]
    fn test_custom_prompts() {
        let prompts = PromptSections {
            main_custom: Some("Custom main prompt".to_string()),
            advanced_enabled: true,
            advanced_custom: Some("Custom advanced prompt".to_string()),
            dictionary_enabled: false,
            dictionary_custom: None,
        };

        let combined = combine_prompt_sections(&prompts);

        assert!(combined.contains("Custom main prompt"));
        assert!(combined.contains("Custom advanced prompt"));
        assert!(!combined.contains("Core Rules")); // Custom replaced default
    }
}
