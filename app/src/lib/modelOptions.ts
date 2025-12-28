export type ModelOption = { value: string; label: string };

// Model options for each STT provider.
// This is intentionally shared between Settings pickers and History filters so
// they always list the same models.
export const STT_MODELS: Record<string, ModelOption[]> = {
  aquavoice: [{ value: "avalon-1", label: "Avalon 1" }],
  groq: [
    { value: "whisper-large-v3-turbo", label: "Whisper Large V3 Turbo" },
    { value: "whisper-large-v3", label: "Whisper Large V3" },
  ],
  openai: [
    // { value: "gpt-audio", label: "GPT Audio" },
    // { value: "gpt-audio-mini", label: "GPT Audio Mini" },
    // { value: "gpt-4o-audio-preview", label: "GPT-4o Audio Preview" },
    // { value: "gpt-4o-mini-audio-preview", label: "GPT-4o Mini Audio Preview" },
    { value: "gpt-4o-transcribe", label: "GPT-4o Transcribe" },
    { value: "gpt-4o-mini-transcribe", label: "GPT-4o Mini Transcribe" },
    { value: "whisper-1", label: "Whisper-1" },
  ],
  deepgram: [
    { value: "nova-3", label: "Nova 3" },
    { value: "nova-2", label: "Nova 2" },
    { value: "nova", label: "Nova" },
    { value: "enhanced", label: "Enhanced" },
    { value: "base", label: "Base" },
  ],
  whisper: [], // Local whisper has its own model management
};

// Model options for each LLM provider.
export const LLM_MODELS: Record<string, ModelOption[]> = {
  groq: [
    { value: "llama-3.3-70b-versatile", label: "Llama 3.3 70B Versatile" },
    { value: "llama-3.1-8b-instant", label: "Llama 3.1 8B Instant" },
    { value: "meta-llama/llama-guard-4-12b", label: "Llama Guard 4 12B" },
    { value: "openai/gpt-oss-120b", label: "GPT-OSS 120B" },
    { value: "openai/gpt-oss-20b", label: "GPT-OSS 20B" },
  ],
  openai: [
    { value: "gpt-5.2", label: "GPT-5.2" },
    { value: "gpt-5.1", label: "GPT-5.1" },
    { value: "gpt-5", label: "GPT-5" },
    { value: "gpt-5-mini", label: "GPT-5 Mini" },
    { value: "gpt-5-nano", label: "GPT-5 Nano" },
    { value: "gpt-4.1", label: "GPT-4.1" },
    { value: "gpt-4.1-mini", label: "GPT-4.1 Mini" },
    { value: "gpt-4.1-nano", label: "GPT-4.1 Nano" },
    // { value: "gpt-4o-mini", label: "GPT-4o Mini" },
    // { value: "gpt-4o", label: "GPT-4o" },
    // { value: "gpt-4-turbo", label: "GPT-4 Turbo" },
  ],
  gemini: [
    // Gemini 3 (preview) - requested as explicit `models/...` IDs.
    { value: "models/gemini-3-pro-preview", label: "Gemini 3 Pro (Preview)" },
    { value: "models/gemini-3-flash-preview", label: "Gemini 3 Flash (Preview)" },

    // Gemini 2.5 (stable)
    { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    // Dovie requested "2.5 Flash Pro" and "2.5 Flash Mini"; closest stable IDs:
    // - Flash (thinking-capable) and Flash-Lite (smallest)
    { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
    { value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
  ],
  anthropic: [
    { value: "claude-sonnet-4-5", label: "Claude Sonnet 4.5" },
    { value: "claude-haiku-4-5", label: "Claude Haiku 4.5" },
    { value: "claude-opus-4-5", label: "Claude Opus 4.5" },
    { value: "claude-3-5-haiku-latest", label: "Claude 3.5 Haiku" },
    { value: "claude-3-5-sonnet-latest", label: "Claude 3.5 Sonnet" },
    { value: "claude-3-opus-latest", label: "Claude 3 Opus" },
  ],
  ollama: [], // Ollama models are dynamic based on what's installed
};

export function listAllSttModelKeys(): Array<{ key: string; label: string }> {
  const options: Array<{ key: string; label: string }> = [];
  for (const [provider, models] of Object.entries(STT_MODELS)) {
    for (const model of models) {
      options.push({
        key: `${provider}::${model.value}`,
        label: `${provider} / ${model.label}`,
      });
    }
  }
  options.sort((a, b) => a.label.localeCompare(b.label));
  return options;
}

export function listAllLlmModelKeys(): Array<{ key: string; label: string }> {
  const options: Array<{ key: string; label: string }> = [];
  for (const [provider, models] of Object.entries(LLM_MODELS)) {
    for (const model of models) {
      options.push({
        key: `${provider}::${model.value}`,
        label: `${provider} / ${model.label}`,
      });
    }
  }
  options.sort((a, b) => a.label.localeCompare(b.label));
  return options;
}
