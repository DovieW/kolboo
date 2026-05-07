import { normalizeSttLanguageOverride } from "../../sttLanguages";
import {
	normalizeRewritePreset as normalizeCanonicalRewritePreset,
	normalizePresetRoutingHints,
} from "../presetDefaults";
import type { CleanupPromptSectionsOverride, RewritePreset } from "../types";
import {
	normalizeOutputMode,
	normalizeOverlayModeValue,
	normalizeWidgetPosition,
} from "./appBehavior";
import { normalizePlayingAudioHandling } from "./audio";
import {
	normalizeAnthropicThinkingBudget,
	normalizeGeminiThinkingBudget,
	normalizeGeminiThinkingLevel,
	normalizeOpenAiReasoningEffort,
} from "./reasoning";

const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object" && !Array.isArray(value);
};

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
		// Preset UI settings are override-or-inherit fields, so malformed overlay
		// values must stay nullable instead of silently becoming a concrete default.
		overlay_mode:
			typeof p.overlay_mode === "string"
				? normalizeOverlayModeValue(p.overlay_mode)
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
