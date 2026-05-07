import type { RewritePreset } from "./types";

export const DEFAULT_NEW_PRESET_NAME = "New preset";

function normalizeNullable<T>(value: T | null | undefined): T | null {
	return value ?? null;
}

/**
 * Keep preset hint normalization in one place so shared-preset edits, new-preset creation, and
 * editor-state hydration all agree on what counts as a real routing hint.
 */
export function normalizePresetRoutingHints(
	routingHints: RewritePreset["routing_hints"],
): string[] | null {
	const normalized = (routingHints ?? [])
		.filter((hint): hint is string => typeof hint === "string")
		.map((hint) => hint.trim())
		.filter(Boolean);

	return normalized.length === 0 ? null : normalized;
}

/**
 * Normalize a persisted preset into the editor/runtime shape we want to work with in TS.
 *
 * Important: this intentionally converts `undefined` optionals into explicit `null`s so the
 * preset editor can treat missing legacy fields the same way as an explicit “inherit” choice.
 */
export function normalizeRewritePreset(preset: RewritePreset): RewritePreset {
	return {
		id: preset.id,
		name: preset.name,
		description: normalizeNullable(preset.description),
		routing_hints: normalizePresetRoutingHints(preset.routing_hints),
		cleanup_prompt_sections: preset.cleanup_prompt_sections ?? null,
		rewrite_llm_enabled: preset.rewrite_llm_enabled ?? true,
		stt_provider: normalizeNullable(preset.stt_provider),
		stt_model: normalizeNullable(preset.stt_model),
		stt_language: normalizeNullable(preset.stt_language),
		stt_timeout_seconds: preset.stt_timeout_seconds ?? null,
		llm_provider: normalizeNullable(preset.llm_provider),
		llm_model: normalizeNullable(preset.llm_model),
		openai_reasoning_effort: normalizeNullable(preset.openai_reasoning_effort),
		gemini_thinking_budget: preset.gemini_thinking_budget ?? null,
		gemini_thinking_level: preset.gemini_thinking_level ?? null,
		anthropic_thinking_budget: preset.anthropic_thinking_budget ?? null,
		sound_enabled: preset.sound_enabled ?? null,
		playing_audio_handling: preset.playing_audio_handling ?? null,
		overlay_mode: preset.overlay_mode ?? null,
		widget_position: preset.widget_position ?? null,
		output_mode: preset.output_mode ?? null,
		output_hit_enter: preset.output_hit_enter ?? null,
	};
}

/**
 * Merge a preset patch through the canonical normalizer so every write path keeps the same
 * null/default semantics.
 */
export function mergeRewritePreset(
	preset: RewritePreset,
	patch: Partial<RewritePreset>,
): RewritePreset {
	return normalizeRewritePreset({
		...preset,
		...patch,
	});
}

/**
 * Create a new preset with the repo's canonical preset defaults.
 */
export function createRewritePreset(
	id: string,
	name = DEFAULT_NEW_PRESET_NAME,
): RewritePreset {
	return normalizeRewritePreset({
		id,
		name,
		description: null,
		routing_hints: null,
		cleanup_prompt_sections: null,
		rewrite_llm_enabled: true,
		stt_provider: null,
		stt_model: null,
		stt_language: null,
		stt_timeout_seconds: null,
		llm_provider: null,
		llm_model: null,
		openai_reasoning_effort: null,
		gemini_thinking_budget: null,
		gemini_thinking_level: null,
		anthropic_thinking_budget: null,
		sound_enabled: null,
		playing_audio_handling: null,
		overlay_mode: null,
		widget_position: null,
		output_mode: null,
		output_hit_enter: null,
	});
}
