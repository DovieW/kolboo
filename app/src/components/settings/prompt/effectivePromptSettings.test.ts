import { describe, expect, it } from "vitest";
import type {
	RewritePreset,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri/types";
import {
	resolvePresetRuntimeFallbackViews,
	resolvePromptProfileFallbacks,
} from "./effectivePromptSettings";

const defaultQuickReplaceSystemPrompt = "Apply the requested edit.";

function profile(
	overrides: Partial<RewriteProgramPromptProfile>,
): RewriteProgramPromptProfile {
	return {
		id: "default",
		name: "Default",
		program_paths: [],
		cleanup_prompt_sections: null,
		...overrides,
	};
}

function preset(overrides: Partial<RewritePreset>): RewritePreset {
	return {
		id: "preset-1",
		name: "Preset",
		routing_hints: null,
		cleanup_prompt_sections: null,
		rewrite_llm_enabled: true,
		...overrides,
	};
}

describe("prompt profile fallback resolution", () => {
	it("uses explicit Default profile values before global settings", () => {
		const fallbacks = resolvePromptProfileFallbacks({
			defaultProfile: profile({
				quick_replace_enabled: true,
				quick_replace_provider: "anthropic",
				quick_replace_model: "claude-sonnet",
				quick_replace_system_prompt: "Profile prompt",
				rewrite_include_clipboard_context: true,
				quick_replace_include_clipboard_context: true,
				quick_ask_include_clipboard_context: true,
				rewrite_active_window_ocr_mode: "manual",
				quick_replace_active_window_ocr_mode: "auto",
				quick_ask_active_window_ocr_mode: "manual",
				quick_ask_dismiss_mode: "auto",
			}),
			settings: {
				quick_replace_enabled: false,
				llm_provider: "openai",
				llm_model: "gpt-4o-mini",
				rewrite_active_window_ocr_mode: "off",
				quick_replace_active_window_ocr_mode: "off",
				quick_ask_active_window_ocr_mode: "off",
				quick_ask_dismiss_mode: "manual",
			},
			defaultQuickReplaceSystemPrompt,
		});

		expect(fallbacks).toMatchObject({
			baseQuickReplaceEnabled: true,
			baseQuickReplaceProvider: "anthropic",
			baseQuickReplaceModel: "claude-sonnet",
			baseQuickReplaceSystemPrompt: "Profile prompt",
			baseRewriteIncludeClipboardContext: true,
			baseQuickReplaceIncludeClipboardContext: true,
			baseQuickAskIncludeClipboardContext: true,
			baseRewriteActiveWindowOcrMode: "manual",
			baseQuickReplaceActiveWindowOcrMode: "auto",
			baseQuickAskActiveWindowOcrMode: "manual",
			baseQuickAskDismissMode: "auto",
		});
	});

	it("treats Default profile nulls as inheritance from global or defaults", () => {
		const fallbacks = resolvePromptProfileFallbacks({
			defaultProfile: profile({
				quick_replace_enabled: null,
				quick_replace_provider: null,
				quick_replace_model: null,
				quick_replace_system_prompt: null,
				rewrite_include_clipboard_context: null,
				quick_replace_include_clipboard_context: null,
				quick_ask_include_clipboard_context: null,
				rewrite_active_window_ocr_mode: null,
				quick_replace_active_window_ocr_mode: null,
				quick_ask_active_window_ocr_mode: null,
				quick_ask_dismiss_mode: null,
			}),
			settings: {
				quick_replace_enabled: true,
				llm_provider: "openai",
				llm_model: "gpt-4o-mini",
				rewrite_active_window_ocr_mode: "auto",
				quick_replace_active_window_ocr_mode: "manual",
				quick_ask_active_window_ocr_mode: "auto",
				quick_ask_dismiss_mode: "auto",
			},
			defaultQuickReplaceSystemPrompt,
		});

		expect(fallbacks).toMatchObject({
			baseQuickReplaceEnabled: true,
			baseQuickReplaceProvider: "openai",
			baseQuickReplaceModel: "gpt-4o-mini",
			baseQuickReplaceSystemPrompt: defaultQuickReplaceSystemPrompt,
			// Clipboard context currently has no global setting, so null inherits to the
			// hard default rather than trying to invent a broader setting surface.
			baseRewriteIncludeClipboardContext: false,
			baseQuickReplaceIncludeClipboardContext: false,
			baseQuickAskIncludeClipboardContext: false,
			baseRewriteActiveWindowOcrMode: "auto",
			baseQuickReplaceActiveWindowOcrMode: "manual",
			baseQuickAskActiveWindowOcrMode: "auto",
			baseQuickAskDismissMode: "auto",
		});
	});

	it("falls back safely when legacy profile values are malformed", () => {
		const fallbacks = resolvePromptProfileFallbacks({
			defaultProfile: profile({
				quick_replace_enabled: "yes" as never,
				quick_replace_provider: 42 as never,
				rewrite_active_window_ocr_mode: "screen" as never,
				quick_ask_dismiss_mode: "later" as never,
			}),
			settings: {
				quick_replace_enabled: false,
				llm_provider: "openai",
				llm_model: "gpt-4o-mini",
				rewrite_active_window_ocr_mode: "manual",
				quick_ask_dismiss_mode: "auto",
			},
			defaultQuickReplaceSystemPrompt,
		});

		expect(fallbacks.baseQuickReplaceEnabled).toBe(false);
		expect(fallbacks.baseQuickReplaceProvider).toBe("openai");
		expect(fallbacks.baseRewriteActiveWindowOcrMode).toBe("manual");
		expect(fallbacks.baseQuickAskDismissMode).toBe("auto");
	});

	it("resolves preset runtime values through preset, profile, global, and defaults", () => {
		const views = resolvePresetRuntimeFallbackViews({
			profile: profile({
				stt_provider: "groq",
				stt_model: "whisper-large-v3",
				stt_language: "en",
				stt_timeout_seconds: 18,
				llm_provider: "anthropic",
				llm_model: "claude-sonnet",
			}),
			preset: preset({
				stt_provider: null,
				stt_model: "whisper-large-v3-turbo",
				stt_timeout_seconds: null,
				llm_provider: "openai",
				llm_model: null,
			}),
			settings: {
				stt_provider: "openai",
				stt_model: "gpt-4o-transcribe",
				stt_language: "es",
				stt_timeout_seconds: 10,
				llm_provider: "gemini",
				llm_model: "gemini-2.5-pro",
			},
			defaultSttTimeout: 30,
			defaultSttLanguage: "auto",
		});

		expect(views.sttProvider).toEqual({
			value: "groq",
			source: "profile",
			explicitNull: true,
		});
		expect(views.sttModel).toEqual({
			value: "whisper-large-v3-turbo",
			source: "preset",
			explicitNull: false,
		});
		expect(views.sttLanguage).toEqual({
			value: "en",
			source: "profile",
			explicitNull: false,
		});
		expect(views.sttTimeoutSeconds).toEqual({
			value: 18,
			source: "profile",
			explicitNull: true,
		});
		expect(views.llmProvider).toEqual({
			value: "openai",
			source: "preset",
			explicitNull: false,
		});
		expect(views.llmModel).toEqual({
			value: "claude-sonnet",
			source: "profile",
			explicitNull: true,
		});
	});

	it("falls back safely for malformed preset runtime values", () => {
		const views = resolvePresetRuntimeFallbackViews({
			profile: profile({
				stt_provider: "groq",
				stt_timeout_seconds: 12,
			}),
			preset: preset({
				stt_provider: 42 as never,
				stt_timeout_seconds: "soon" as never,
				llm_provider: false as never,
			}),
			settings: {
				stt_provider: "openai",
				stt_timeout_seconds: 9,
				llm_provider: "gemini",
			},
			defaultSttTimeout: 30,
			defaultSttLanguage: "auto",
		});

		expect(views.sttProvider.value).toBe("groq");
		expect(views.sttProvider.source).toBe("profile");
		expect(views.sttTimeoutSeconds.value).toBe(12);
		expect(views.llmProvider.value).toBe("gemini");
		expect(views.llmProvider.source).toBe("global");
	});
});
