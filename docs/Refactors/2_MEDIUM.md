# Medium-priority refactors

- Consider extending `app/src-tauri/src/llm/http_json.rs` to support provider-specific network error mapping (e.g. Ollama’s `is_connect()` -> `LlmError::ProviderNotAvailable(...)`) so `llm/ollama.rs` can also use the shared helper without losing its nicer “Ollama not reachable” UX.
