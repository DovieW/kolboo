import { isInheritedSettingValue } from "../../../lib/tauri/settingsViews";
import type {
	ActiveWindowOcrMode,
	AppSettings,
	QuickAskDismissMode,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri/types";

export type PromptProfileFallbacks = {
	baseQuickReplaceEnabled: boolean;
	baseQuickReplaceProvider: string | null;
	baseQuickReplaceModel: string | null;
	baseQuickReplaceSystemPrompt: string;
	baseRewriteIncludeClipboardContext: boolean;
	baseQuickReplaceIncludeClipboardContext: boolean;
	baseQuickAskIncludeClipboardContext: boolean;
	baseRewriteActiveWindowOcrMode: ActiveWindowOcrMode;
	baseQuickReplaceActiveWindowOcrMode: ActiveWindowOcrMode;
	baseQuickAskActiveWindowOcrMode: ActiveWindowOcrMode;
	baseQuickAskDismissMode: QuickAskDismissMode;
};

type PromptFallbackSettings = Partial<
	Pick<
		AppSettings,
		| "quick_replace_enabled"
		| "llm_provider"
		| "llm_model"
		| "rewrite_active_window_ocr_mode"
		| "quick_replace_active_window_ocr_mode"
		| "quick_ask_active_window_ocr_mode"
		| "quick_ask_dismiss_mode"
	>
>;

export function profileSettingIsInherited(
	profile: RewriteProgramPromptProfile,
	key: keyof RewriteProgramPromptProfile,
): boolean {
	// Profile/preset UI treats both missing and explicit null as intentional
	// inheritance. Keep this as a thin production adapter over the shared Settings
	// View rule so future provenance UI has one null/missing vocabulary.
	return isInheritedSettingValue(profile, key);
}

function boolOrDefault(value: boolean | null | undefined, fallback: boolean) {
	return typeof value === "boolean" ? value : fallback;
}

function quickAskDismissModeOrDefault(
	value: string | null | undefined,
	fallback: QuickAskDismissMode,
): QuickAskDismissMode {
	return value === "auto" || value === "manual" ? value : fallback;
}

export function resolvePromptProfileFallbacks({
	defaultProfile,
	settings,
	defaultQuickReplaceSystemPrompt,
}: {
	defaultProfile: RewriteProgramPromptProfile | null;
	settings: PromptFallbackSettings | undefined;
	defaultQuickReplaceSystemPrompt: string;
}): PromptProfileFallbacks {
	// Quick Replace inherits from the Default profile. If Default has never been
	// configured, keep the legacy global fallback so older settings.json shapes
	// remain behavior-compatible.
	const baseQuickReplaceEnabled = boolOrDefault(
		defaultProfile?.quick_replace_enabled,
		settings?.quick_replace_enabled ?? false,
	);

	return {
		baseQuickReplaceEnabled,
		baseQuickReplaceProvider:
			defaultProfile?.quick_replace_provider ?? settings?.llm_provider ?? null,
		baseQuickReplaceModel:
			defaultProfile?.quick_replace_model ?? settings?.llm_model ?? null,
		baseQuickReplaceSystemPrompt:
			defaultProfile?.quick_replace_system_prompt ??
			defaultQuickReplaceSystemPrompt,
		baseRewriteIncludeClipboardContext: boolOrDefault(
			defaultProfile?.rewrite_include_clipboard_context,
			false,
		),
		baseQuickReplaceIncludeClipboardContext: boolOrDefault(
			defaultProfile?.quick_replace_include_clipboard_context,
			false,
		),
		baseQuickAskIncludeClipboardContext: boolOrDefault(
			defaultProfile?.quick_ask_include_clipboard_context,
			false,
		),
		baseRewriteActiveWindowOcrMode:
			defaultProfile?.rewrite_active_window_ocr_mode ??
			settings?.rewrite_active_window_ocr_mode ??
			"off",
		baseQuickReplaceActiveWindowOcrMode:
			defaultProfile?.quick_replace_active_window_ocr_mode ??
			settings?.quick_replace_active_window_ocr_mode ??
			"off",
		baseQuickAskActiveWindowOcrMode:
			defaultProfile?.quick_ask_active_window_ocr_mode ??
			settings?.quick_ask_active_window_ocr_mode ??
			"off",
		baseQuickAskDismissMode: quickAskDismissModeOrDefault(
			defaultProfile?.quick_ask_dismiss_mode,
			quickAskDismissModeOrDefault(settings?.quick_ask_dismiss_mode, "manual"),
		),
	};
}
