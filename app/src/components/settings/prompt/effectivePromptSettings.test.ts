import { describe, expect, it } from "vitest";
import type { RewriteProgramPromptProfile } from "../../../lib/tauri/types";
import { resolvePromptProfileFallbacks } from "./effectivePromptSettings";

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
});
