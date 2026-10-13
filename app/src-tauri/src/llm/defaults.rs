// Centralized defaults for LLM provider models.
//
// These are used when the user has not explicitly selected a model.
// Keep these in sync with the provider implementations' DEFAULT_MODEL constants.

/// Returns the default model id for a given LLM provider id.
pub fn default_llm_model_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "managed" => Some("gpt-4o-mini"),
        "openai" => Some("gpt-4o-mini"),
        "cerebras" => Some("gpt-oss-120b"),
        "anthropic" => Some("claude-haiku-5-5"),
        "groq" => Some("openai/gpt-oss-20b"),
        "fireworks" => Some("accounts/fireworks/models/llama-v3p1-8b-instruct"),
        "cohere" => Some("command-r-08-2024"),
        "gemini" => Some("gemini-3.5-flash-lite"),
        "ollama" => Some("llama3.2"),
        _ => None,
    }
}
