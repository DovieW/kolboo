import { normalizeSttLanguageOverride } from "../../sttLanguages";
import {
	normalizeRewritePreset as normalizeCanonicalRewritePreset,
	normalizePresetRoutingHints,
} from "../presetDefaults";
import type {
	CleanupPromptSectionsOverride,
	OpenAiReasoningEffort,
	OutputMode,
	OverlayMode,
	PlayingAudioHandling,
	RewritePreset,
	WidgetPosition,
} from "../types";

const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object" && !Array.isArray(value);
};

function normalizeOutputMode(value: unknown): OutputMode {
	if (
		value === "paste" ||
		value === "paste_and_clipboard" ||
		value === "clipboard"
	) {
		return value;
	}

	// Legacy/disabled values: "keystrokes", "keystrokes_and_clipboard",
	// and "auto_paste" now map to the safe paste path.
	return "paste";
}

function normalizePlayingAudioHandling(value: unknown): PlayingAudioHandling {
	if (
		value === "none" ||
		value === "mute" ||
		value === "pause" ||
		value === "mute_and_pause"
	) {
		return value;
	}

	if (typeof value === "boolean") {
		return value ? "mute" : "none";
	}

	return "none";
}

function normalizeOverlayMode(value: unknown): OverlayMode {
	if (value === "always" || value === "never" || value === "recording_only") {
		return value;
	}
	return "recording_only";
}

function normalizeWidgetPosition(value: unknown): WidgetPosition | null {
	if (
		value === "center" ||
		value === "top-left" ||
		value === "top-center" ||
		value === "top-right" ||
		value === "bottom-left" ||
		value === "bottom-center" ||
		value === "bottom-right"
	) {
		return value;
	}
	return null;
}

function normalizeOpenAiReasoningEffort(
	value: unknown,
): OpenAiReasoningEffort | null {
	if (typeof value !== "string") return null;
	const v = value.trim().toLowerCase();
	if (
		v === "none" ||
		v === "minimal" ||
		v === "low" ||
		v === "medium" ||
		v === "high" ||
		v === "xhigh"
	) {
		return v as OpenAiReasoningEffort;
	}
	return null;
}

function normalizeGeminiThinkingLevel(
	value: unknown,
): "minimal" | "low" | "medium" | "high" | null {
	if (typeof value !== "string") return null;
	const v = value.trim().toLowerCase();
	if (v === "minimal" || v === "low" || v === "medium" || v === "high") {
		return v;
	}
	return null;
}

function normalizeGeminiThinkingBudget(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	return Math.trunc(value);
}

function normalizeAnthropicThinkingBudget(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	const n = Math.trunc(value);
	if (n < 1024) return 1024;
	return Math.min(32768, n);
}

/**
 * Normalize legacy/persisted preset blobs before Settings exposes them.
 *
 * `presetDefaults.ts` remains the canonical typed preset normalizer used by UI
 * write paths. This raw reader is intentionally stricter at the persistence edge:
 * invalid preset ids are rejected, malformed legacy fields are converted to
 * explicit inherit/null values, and then the typed canonical normalizer finishes
 * the null/default semantics.
 */
export function normalizeRawRewritePreset(
	value: unknown,
): RewritePreset | null {
	const p = isRecord(value) ? value : null;
	if (!p) return null;
	const id = typeof p.id === "string" ? p.id : "";
	const name = typeof p.name === "string" ? p.name : "";
	if (!id) return null;

	const description =
		typeof p.description === "string" && p.description.trim().length > 0
			? p.description
			: null;
	const cleanup_prompt_sections = isRecord(p.cleanup_prompt_sections)
		? (p.cleanup_prompt_sections as CleanupPromptSectionsOverride)
		: null;

	return normalizeCanonicalRewritePreset({
		id,
		name,
		description,
		routing_hints: normalizePresetRoutingHints(
			Array.isArray(p.routing_hints) ? p.routing_hints : null,
		),
		cleanup_prompt_sections,
		rewrite_llm_enabled:
			typeof p.rewrite_llm_enabled === "boolean" ? p.rewrite_llm_enabled : true,
		stt_provider: typeof p.stt_provider === "string" ? p.stt_provider : null,
		stt_model: typeof p.stt_model === "string" ? p.stt_model : null,
		stt_language: normalizeSttLanguageOverride(p.stt_language),
		stt_timeout_seconds:
			typeof p.stt_timeout_seconds === "number" &&
			Number.isFinite(p.stt_timeout_seconds)
				? p.stt_timeout_seconds
				: null,
		llm_provider: typeof p.llm_provider === "string" ? p.llm_provider : null,
		llm_model: typeof p.llm_model === "string" ? p.llm_model : null,
		openai_reasoning_effort: normalizeOpenAiReasoningEffort(
			p.openai_reasoning_effort,
		),
		gemini_thinking_budget: normalizeGeminiThinkingBudget(
			p.gemini_thinking_budget,
		),
		gemini_thinking_level: normalizeGeminiThinkingLevel(
			p.gemini_thinking_level,
		),
		anthropic_thinking_budget: normalizeAnthropicThinkingBudget(
			p.anthropic_thinking_budget,
		),
		sound_enabled:
			typeof p.sound_enabled === "boolean" ? p.sound_enabled : null,
		playing_audio_handling:
			typeof p.playing_audio_handling === "string" ||
			typeof p.playing_audio_handling === "boolean"
				? normalizePlayingAudioHandling(p.playing_audio_handling)
				: null,
		overlay_mode:
			typeof p.overlay_mode === "string"
				? normalizeOverlayMode(p.overlay_mode)
				: null,
		widget_position: normalizeWidgetPosition(p.widget_position),
		output_mode:
			typeof p.output_mode === "string"
				? normalizeOutputMode(p.output_mode)
				: null,
		output_hit_enter:
			typeof p.output_hit_enter === "boolean" ? p.output_hit_enter : null,
	});
}
