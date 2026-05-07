import { normalizeSttLanguageOverride } from "../../sttLanguages";
import type {
	ActiveWindowOcrMode,
	CleanupPromptSections,
	CleanupPromptSectionsOverride,
	ContextGrabMethod,
	RewritePreset,
	RewriteProgramPromptProfile,
} from "../types";
import {
	normalizeOutputMode,
	normalizeOverlayModeValue,
	normalizeQuickAskDismissModeOverride,
	normalizeWidgetPosition,
} from "./appBehavior";
import { normalizePlayingAudioHandling } from "./audio";
import { normalizeActiveWindowOcrModeOverride } from "./ocr";
import { normalizeRawRewritePreset } from "./presets";
import {
	normalizeAnthropicThinkingBudgetAllowOff,
	normalizeGeminiThinkingBudget,
	normalizeGeminiThinkingLevel,
	normalizeOpenAiReasoningEffort,
} from "./reasoning";
import { normalizeIntentRouterSettings } from "./routing";
import { isRecord, normalizeNonEmptyStringSetting } from "./shared";

type PromptSection = CleanupPromptSections["system"];

function normalizeContextGrabMethod(value: unknown): ContextGrabMethod | null {
	return value === "none" ||
		value === "ctrl_c" ||
		value === "ctrl_shift_c" ||
		value === "ctrl_insert" ||
		value === "clipboard_only"
		? value
		: null;
}

export function normalizePromptSection(value: unknown): PromptSection | null {
	if (value === null) return null;
	if (!isRecord(value)) return null;
	const content = typeof value.content === "string" ? value.content : null;

	return { content };
}

export function normalizeCleanupPromptSections(
	value: unknown,
): CleanupPromptSections | null {
	if (value === null || value === undefined) return null;
	if (!isRecord(value)) return null;
	const v = value;

	// New shape.
	if (Object.hasOwn(v, "system")) {
		const system = normalizePromptSection(v.system) ?? { content: null };
		return { system };
	}

	// Legacy shape: { main, advanced, dictionary }.
	// We keep only the old "main" section as the new System Prompt.
	if (Object.hasOwn(v, "main")) {
		const rawMain = v.main;
		const legacyContent =
			typeof rawMain === "string"
				? rawMain.trim().length > 0
					? rawMain
					: null
				: (normalizePromptSection(rawMain)?.content ?? null);
		return { system: { content: legacyContent } };
	}

	// Unknown/empty object => treat as unset.
	return null;
}

export function normalizeCleanupPromptSectionsOverride(
	value: unknown,
): CleanupPromptSectionsOverride | null {
	if (value === null || value === undefined) return null;
	if (!isRecord(value)) return null;

	const v = value;
	const out: CleanupPromptSectionsOverride = {};

	if (Object.hasOwn(v, "system")) {
		out.system = normalizePromptSection(v.system);
	}

	// If we didn't recognize anything (or it's effectively empty), treat as unset.
	if (out.system == null) return null;

	return out;
}

export function normalizeRewriteProfile(
	value: unknown,
): RewriteProgramPromptProfile | null {
	if (!isRecord(value)) return null;
	const p = value;
	const id = typeof p.id === "string" ? p.id : "";
	const name = typeof p.name === "string" ? p.name : "";

	const program_paths_raw = p.program_paths;
	const legacy_program_path = p.program_path;

	const program_paths = Array.isArray(program_paths_raw)
		? program_paths_raw.filter((x) => typeof x === "string")
		: typeof legacy_program_path === "string" && legacy_program_path.length > 0
			? [legacy_program_path]
			: [];

	const disabled = typeof p.disabled === "boolean" ? p.disabled : false;

	const cleanup_prompt_sections = normalizeCleanupPromptSectionsOverride(
		p.cleanup_prompt_sections,
	);
	const stt_provider =
		typeof p.stt_provider === "string" ? p.stt_provider : null;
	const stt_model = typeof p.stt_model === "string" ? p.stt_model : null;
	const stt_language = normalizeSttLanguageOverride(p.stt_language);
	const stt_timeout_seconds =
		typeof p.stt_timeout_seconds === "number" ? p.stt_timeout_seconds : null;
	const llm_provider =
		typeof p.llm_provider === "string" ? p.llm_provider : null;
	const llm_model = typeof p.llm_model === "string" ? p.llm_model : null;

	const openai_reasoning_effort = normalizeOpenAiReasoningEffort(
		p.openai_reasoning_effort,
	);
	const gemini_thinking_budget = normalizeGeminiThinkingBudget(
		p.gemini_thinking_budget,
	);
	const gemini_thinking_level = normalizeGeminiThinkingLevel(
		p.gemini_thinking_level,
	);
	const anthropic_thinking_budget = normalizeAnthropicThinkingBudgetAllowOff(
		p.anthropic_thinking_budget,
	);

	const quick_ask_provider =
		typeof p.quick_ask_provider === "string" ? p.quick_ask_provider : null;
	const quick_ask_model =
		typeof p.quick_ask_model === "string" ? p.quick_ask_model : null;
	const quick_ask_system_prompt = normalizeNonEmptyStringSetting(
		p.quick_ask_system_prompt,
	);
	const quick_ask_dismiss_mode = normalizeQuickAskDismissModeOverride(
		p.quick_ask_dismiss_mode,
	);

	const context_grab_method = normalizeContextGrabMethod(p.context_grab_method);

	const rewrite_include_clipboard_context =
		typeof p.rewrite_include_clipboard_context === "boolean"
			? p.rewrite_include_clipboard_context
			: null;
	const quick_replace_include_clipboard_context =
		typeof p.quick_replace_include_clipboard_context === "boolean"
			? p.quick_replace_include_clipboard_context
			: null;
	const quick_ask_include_clipboard_context =
		typeof p.quick_ask_include_clipboard_context === "boolean"
			? p.quick_ask_include_clipboard_context
			: null;

	const rewrite_active_window_ocr_mode: ActiveWindowOcrMode | null =
		normalizeActiveWindowOcrModeOverride(p.rewrite_active_window_ocr_mode);
	const quick_replace_active_window_ocr_mode: ActiveWindowOcrMode | null =
		normalizeActiveWindowOcrModeOverride(
			p.quick_replace_active_window_ocr_mode,
		);
	const quick_ask_active_window_ocr_mode: ActiveWindowOcrMode | null =
		normalizeActiveWindowOcrModeOverride(p.quick_ask_active_window_ocr_mode);

	const quick_replace_enabled =
		typeof p.quick_replace_enabled === "boolean"
			? p.quick_replace_enabled
			: null;
	const quick_replace_provider =
		typeof p.quick_replace_provider === "string"
			? p.quick_replace_provider
			: null;
	const quick_replace_model =
		typeof p.quick_replace_model === "string" ? p.quick_replace_model : null;
	const quick_replace_system_prompt = normalizeNonEmptyStringSetting(
		p.quick_replace_system_prompt,
	);

	const quick_ask_openai_reasoning_effort = normalizeOpenAiReasoningEffort(
		p.quick_ask_openai_reasoning_effort,
	);
	const quick_ask_gemini_thinking_budget = normalizeGeminiThinkingBudget(
		p.quick_ask_gemini_thinking_budget,
	);
	const quick_ask_gemini_thinking_level = normalizeGeminiThinkingLevel(
		p.quick_ask_gemini_thinking_level,
	);
	const quick_ask_anthropic_thinking_budget =
		normalizeAnthropicThinkingBudgetAllowOff(
			p.quick_ask_anthropic_thinking_budget,
		);
	const rewrite_llm_enabled =
		typeof p.rewrite_llm_enabled === "boolean" ? p.rewrite_llm_enabled : null;

	const sound_enabled =
		typeof p.sound_enabled === "boolean" ? p.sound_enabled : null;
	const playing_audio_handling_raw = p.playing_audio_handling;
	const legacy_auto_mute_audio = p.auto_mute_audio;

	const playing_audio_handling =
		typeof playing_audio_handling_raw === "string"
			? normalizePlayingAudioHandling(playing_audio_handling_raw)
			: typeof legacy_auto_mute_audio === "boolean"
				? legacy_auto_mute_audio
					? "mute"
					: "none"
				: null;

	const overlay_mode =
		typeof p.overlay_mode === "string"
			? normalizeOverlayModeValue(p.overlay_mode)
			: null;
	const widget_position = normalizeWidgetPosition(p.widget_position);
	const output_mode =
		typeof p.output_mode === "string"
			? normalizeOutputMode(p.output_mode)
			: null;
	const output_hit_enter =
		typeof p.output_hit_enter === "boolean" ? p.output_hit_enter : null;

	const presets_raw = p.presets;
	const presets: RewritePreset[] | null = Array.isArray(presets_raw)
		? presets_raw
				.map(normalizeRawRewritePreset)
				.filter((x): x is RewritePreset => x !== null)
		: null;

	const default_preset_id =
		typeof p.default_preset_id === "string" ? p.default_preset_id : null;
	const default_preset_description =
		typeof p.default_preset_description === "string"
			? p.default_preset_description
			: null;
	const active_preset_id =
		typeof p.active_preset_id === "string" ? p.active_preset_id : null;
	const router = p.router ? normalizeIntentRouterSettings(p.router) : null;

	if (!id) return null;

	return {
		id,
		name,
		program_paths,
		disabled,
		cleanup_prompt_sections,
		presets,
		default_preset_id,
		default_preset_description,
		router,
		active_preset_id,
		rewrite_llm_enabled,
		stt_provider,
		stt_model,
		stt_language,
		stt_timeout_seconds,
		llm_provider,
		llm_model,
		openai_reasoning_effort,
		gemini_thinking_budget,
		gemini_thinking_level,
		anthropic_thinking_budget,
		quick_ask_provider,
		quick_ask_model,
		quick_ask_system_prompt,
		quick_ask_dismiss_mode,
		context_grab_method,
		rewrite_include_clipboard_context,
		quick_replace_include_clipboard_context,
		quick_ask_include_clipboard_context,
		rewrite_active_window_ocr_mode,
		quick_replace_active_window_ocr_mode,
		quick_ask_active_window_ocr_mode,
		quick_replace_enabled,
		quick_replace_provider,
		quick_replace_model,
		quick_replace_system_prompt,
		quick_ask_openai_reasoning_effort,
		quick_ask_gemini_thinking_budget,
		quick_ask_gemini_thinking_level,
		quick_ask_anthropic_thinking_budget,
		sound_enabled,
		playing_audio_handling,
		overlay_mode,
		widget_position,
		output_mode,
		output_hit_enter,
	};
}
