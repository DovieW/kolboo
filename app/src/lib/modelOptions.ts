import type { ManagedModel } from "./tauri";

export type ModelOption = {
	value: string;
	label: string;
	pricing?: {
		input_per_million?: number;
		output_per_million?: number;
		cache_read_per_million?: number;
		source: "models.dev";
	};
};

export type ManagedModelByokTarget = { provider: string; model: string };

/** Display metadata never replaces the raw identifier sent to providers. */
export function configuredProviderModelOptions(
	provider:
		| {
				models?: string[] | null;
				model_names?: Record<string, string> | null;
		  }
		| undefined,
): ModelOption[] | undefined {
	return provider?.models?.map((value) => ({
		value,
		label:
			provider.model_names && Object.hasOwn(provider.model_names, value)
				? provider.model_names[value]?.trim() || value
				: value,
	}));
}

/** Keep an explicitly saved ID visible when discovery or the bundled list changes. */
export function retainSavedModelOption(
	options: ModelOption[],
	model: string | null,
): ModelOption[] {
	if (
		!model ||
		options.length === 0 ||
		options.some((option) => option.value === model)
	)
		return options;
	return [...options, { value: model, label: `${model} (saved selection)` }];
}

// Recognized model identifiers for privacy-safe analytics. This list is not
// authority to enable managed models; availability comes only from API Edge.
export const BUNDLED_MANAGED_MODELS: ManagedModel[] = [
	{
		id: "whisper-large-v3-turbo",
		display_name: "Whisper Large V3 Turbo",
		provider: "groq",
		capabilities: ["transcription"],
		default_for_provider: true,
	},
	{
		id: "whisper-large-v3",
		display_name: "Whisper Large V3",
		provider: "groq",
		capabilities: ["transcription"],
		default_for_provider: false,
	},
	{
		id: "gpt-4o-mini",
		display_name: "GPT-4o mini",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: true,
	},
	{
		id: "gpt-5-mini",
		display_name: "GPT-5 mini",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: false,
	},
	{
		id: "gpt-5",
		display_name: "GPT-5",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: false,
	},
	{
		id: "gpt-5.5",
		display_name: "GPT-5.5",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: false,
	},
];

export function publishedManagedModels(
	models: ManagedModel[] | null | undefined,
): ManagedModel[] {
	// Missing or empty discovery never silently enables bundled cloud models.
	return models ?? [];
}

export function managedModelByokTarget(
	model: ManagedModel,
): ManagedModelByokTarget | null {
	if (model.provider === "cloudflare") return null;
	if (model.provider === "google") {
		return {
			provider: "gemini",
			model:
				model.id === "gemini-3-flash"
					? "models/gemini-3-flash-preview"
					: model.id,
		};
	}
	return { provider: model.provider, model: model.id };
}

export function managedChatModelOptions(models: ManagedModel[]): ModelOption[] {
	return models
		.filter((model) => model.capabilities.includes("chat_completions"))
		.map((model) => ({
			value: model.id,
			label: `${model.display_name} · ${model.provider}`,
		}));
}

export function managedTranscriptionModelOptions(
	models: ManagedModel[],
	provider?: string | null,
): ModelOption[] {
	return models
		.filter(
			(model) =>
				model.capabilities.includes("transcription") &&
				(!provider || model.provider === provider),
		)
		.map((model) => ({
			value: model.id,
			label: model.display_name,
		}));
}

export function isManagedModelSelection(
	models: ManagedModel[],
	capability: "chat_completions" | "transcription",
	provider: string | null | undefined,
	model: string | null | undefined,
): boolean {
	if (!provider || !model) return false;
	return models.some(
		(entry) =>
			entry.provider === provider &&
			entry.id === model &&
			entry.capabilities.includes(capability),
	);
}

// Model options for embedding providers (used by intent router).
export const EMBEDDING_MODELS: Record<string, ModelOption[]> = {
	openai: [
		{ value: "text-embedding-3-small", label: "Text Embedding 3 Small" },
		{ value: "text-embedding-3-large", label: "Text Embedding 3 Large" },
	],
	cohere: [
		{ value: "embed-v4.0", label: "Embed v4" },
		{ value: "embed-multilingual-v3.0", label: "Embed Multilingual v3" },
		{ value: "embed-english-v3.0", label: "Embed English v3" },
	],
	fireworks: [
		{ value: "fireworks/qwen3-embedding-0p6b", label: "Qwen3 Embedding 0.6B" },
		{ value: "fireworks/qwen3-embedding-4b", label: "Qwen3 Embedding 4B" },
		{ value: "fireworks/qwen3-embedding-8b", label: "Qwen3 Embedding 8B" },
	],
};

// Model options for each STT provider.
// This is intentionally shared between Settings pickers and History filters so
// they always list the same models.
export const STT_MODELS: Record<string, ModelOption[]> = {
	aquavoice: [
		{ value: "avalon-v1.5", label: "Avalon v1.5" },
		{ value: "avalon-v1-en", label: "Avalon v1 (legacy)" },
	],
	assemblyai: [
		{
			value: "universal-3-6-pro",
			label: "Universal 3.6 Pro (Realtime · 32 languages)",
		},
		{ value: "universal-3-5-pro", label: "Universal 3.5 Pro (18 languages)" },
		{ value: "universal-2", label: "Universal 2 (99 languages)" },
		{
			value: "universal-streaming-english",
			label: "Universal Streaming English (Realtime)",
		},
		{
			value: "universal-streaming-multilingual",
			label: "Universal Streaming Multilingual (Realtime)",
		},
		{ value: "universal", label: "Universal (server default · legacy alias)" },
	],
	elevenlabs: [
		{ value: "scribe_v2", label: "Scribe v2" },
		{ value: "scribe_v1", label: "Scribe v1" },
	],
	groq: [
		{ value: "whisper-large-v3-turbo", label: "Whisper Large V3 Turbo" },
		{ value: "whisper-large-v3", label: "Whisper Large V3" },
	],
	openai: [
		{ value: "gpt-transcribe", label: "GPT Transcribe" },
		{ value: "gpt-live-transcribe", label: "GPT Live Transcribe (Realtime)" },
		{
			value: "gpt-4o-realtime-transcribe",
			label: "GPT-4o Realtime Transcribe",
		},
		{
			value: "gpt-4o-mini-realtime-transcribe",
			label: "GPT-4o Mini Realtime Transcribe",
		},
		{ value: "gpt-4o-transcribe", label: "GPT-4o Transcribe" },
		{
			value: "gpt-4o-transcribe-diarize",
			label: "GPT-4o Transcribe · speaker labels",
		},
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
	speechmatics: [
		{ value: "enhanced", label: "Enhanced" },
		{ value: "standard", label: "Standard" },
	],
	"whisper-server": [
		// Keep this list conservative: most Whisper-compatible servers accept/ignore
		// the model parameter, and supported values vary by implementation.
		{ value: "whisper-1", label: "Whisper-1" },
	],
	whisper: [], // Local whisper has its own model management
};

// Model options for each LLM provider.
export const LLM_MODELS: Record<string, ModelOption[]> = {
	managed: [],
	cerebras: [
		{ value: "qwen-3.8-27b", label: "Qwen 3.8 27B" },
		{ value: "gpt-oss-120b", label: "GPT-OSS 120B" },
	],
	groq: [
		{ value: "openai/gpt-oss-120b", label: "GPT-OSS 120B" },
		{ value: "openai/gpt-oss-20b", label: "GPT-OSS 20B" },
		{ value: "qwen/qwen3.8-27b", label: "Qwen 3.8 27B (Preview)" },
		{ value: "llama-3.3-70b-versatile", label: "Llama 3.3 70B (Enterprise)" },
		{ value: "llama-3.1-8b-instant", label: "Llama 3.1 8B (Enterprise)" },
	],
	fireworks: [
		{
			value: "accounts/fireworks/models/deepseek-v4p1-flash",
			label: "DeepSeek V4.1 Flash",
		},
		{ value: "accounts/fireworks/models/glm-5p3", label: "GLM 5.3" },
		{ value: "accounts/fireworks/models/kimi-k3", label: "Kimi K3" },
		{
			value: "accounts/fireworks/models/llama-v3p1-8b-instruct",
			label: "Llama 3.1 8B Instruct",
		},
		{
			value: "accounts/fireworks/models/llama-v3p1-70b-instruct",
			label: "Llama 3.1 70B Instruct",
		},
	],
	openai: [
		{ value: "gpt-6.1-sol", label: "GPT-6.1 Sol" },
		{ value: "gpt-6-astra", label: "GPT-6 Astra" },
		{ value: "gpt-6-luna", label: "GPT-6 Luna" },
		{ value: "gpt-5.2", label: "GPT-5.2" },
		{ value: "gpt-5.1", label: "GPT-5.1" },
		{ value: "gpt-5", label: "GPT-5" },
		{ value: "gpt-5-mini", label: "GPT-5 Mini" },
		{ value: "gpt-5-nano", label: "GPT-5 Nano" },
		{ value: "gpt-4.1", label: "GPT-4.1" },
		{ value: "gpt-4.1-mini", label: "GPT-4.1 Mini" },
		{ value: "gpt-4.1-nano", label: "GPT-4.1 Nano" },
		{ value: "gpt-4o-mini", label: "GPT-4o Mini" },
	],
	gemini: [
		{ value: "gemini-3.8-flash", label: "Gemini 3.8 Flash" },
		{ value: "gemini-3.7-flash", label: "Gemini 3.7 Flash" },
		{ value: "gemini-3.6-flash", label: "Gemini 3.6 Flash" },
		{ value: "gemini-3.5-flash-lite", label: "Gemini 3.5 Flash-Lite" },
		{ value: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro (Preview)" },
		{
			value: "models/gemini-3-flash-preview",
			label: "Gemini 3 Flash (Preview)",
		},

		// Gemini 2.5 (stable)
		{ value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
		{ value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
		{ value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
	],
	anthropic: [
		{ value: "claude-fable-5-1", label: "Claude Fable 5.1" },
		{ value: "claude-opus-5-5", label: "Claude Opus 5.5" },
		{ value: "claude-sonnet-5-5", label: "Claude Sonnet 5.5" },
		{ value: "claude-haiku-5-5", label: "Claude Haiku 5.5" },
		{ value: "claude-sonnet-4-5", label: "Claude Sonnet 4.5" },
		{ value: "claude-haiku-4-5", label: "Claude Haiku 4.5" },
		{ value: "claude-opus-4-5", label: "Claude Opus 4.5" },
	],
	cohere: [
		{ value: "command-a-plus-05-2026", label: "Command A+" },
		{ value: "command-a-03-2025", label: "Command A" },
		{ value: "command-r-plus-08-2024", label: "Command R+ (08/2024)" },
		{ value: "command-r-08-2024", label: "Command R (08/2024)" },
	],
	ollama: [], // Ollama models are dynamic based on what's installed
};

/**
 * Build a flat list of `{key, label}` entries from a provider→models map.
 * Each key is formatted as `provider::model.value`, label as `provider / model.label`.
 */
function listAllModelKeys(
	modelsByProvider: Record<string, ModelOption[]>,
): Array<{ key: string; label: string }> {
	const options: Array<{ key: string; label: string }> = [];
	for (const [provider, models] of Object.entries(modelsByProvider)) {
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

/**
 * Whether the given STT provider+model combination supports realtime
 * concurrent streaming (audio is sent during recording for near-instant
 * transcription when recording stops).
 */
export function isRealtimeSttModel(
	provider: string | null | undefined,
	model: string | null | undefined,
): boolean {
	if (!provider || !model) return false;
	const key = `${provider}::${model}`;
	return REALTIME_STT_MODELS.has(key);
}

const REALTIME_STT_MODELS = new Set([
	"assemblyai::universal-3-6-pro",
	"assemblyai::universal-streaming-english",
	"assemblyai::universal-streaming-multilingual",
	"deepgram::nova-3",
	"deepgram::nova-2",
	"deepgram::nova",
	"deepgram::enhanced",
	"deepgram::base",
	"elevenlabs::scribe_v2",
	"fireworks::fireworks-asr-large",
	"fireworks::fireworks-asr-v2",
	"openai::gpt-4o-realtime-transcribe",
	"openai::gpt-4o-mini-realtime-transcribe",
	"openai::gpt-live-transcribe",
	"speechmatics::enhanced",
	"speechmatics::standard",
]);

/** These protocols cannot perform a batch request for the same selected model. */
export function isStreamingOnlySttModel(provider: string, model: string): boolean {
	return (provider === "openai" || provider === "assemblyai" || provider === "fireworks") && isRealtimeSttModel(provider, model);
}

export function listAllSttModelKeys(): Array<{ key: string; label: string }> {
	return listAllModelKeys(STT_MODELS);
}

export function listAllLlmModelKeys(): Array<{ key: string; label: string }> {
	return listAllModelKeys(LLM_MODELS);
}
