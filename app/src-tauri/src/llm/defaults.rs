// Centralized defaults for LLM provider models.
//
// These are used when the user has not explicitly selected a model.
// Keep these in sync with the provider implementations' DEFAULT_MODEL constants.

/// Returns the default model id for a given LLM provider id.
pub fn default_llm_model_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("gpt-4o-mini"),
        "anthropic" => Some("claude-3-haiku-20240307"),
        "groq" => Some("llama-3.3-70b-versatile"),
        "cohere" => Some("command-r-08-2024"),
        "gemini" => Some("gemini-2.5-flash"),
        "ollama" => Some("llama3.2"),
        _ => None,
    }
}
