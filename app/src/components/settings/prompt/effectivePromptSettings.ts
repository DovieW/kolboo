import {
	inheritedSettingView,
	isInheritedSettingValue,
} from "../../../lib/tauri/settingsViews";
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

function boolOrNull(value: unknown): boolean | null {
	return typeof value === "boolean" ? value : null;
}

function stringOrNull(value: unknown): string | null {
	return typeof value === "string" ? value : null;
}

function activeWindowOcrModeOrNull(value: unknown): ActiveWindowOcrMode | null {
	return value === "off" || value === "auto" || value === "manual"
		? value
		: null;
}

function quickAskDismissModeOrNull(value: unknown): QuickAskDismissMode | null {
	return value === "auto" || value === "manual" ? value : null;
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
	// Quick Replace inherits from the Default profile. Route every fallback through
	// Settings View helpers so profile UI, effective state, and future provenance
	// labels keep one null/missing/malformed vocabulary.
	const quickReplaceEnabledView = inheritedSettingView({
		globalValue: settings?.quick_replace_enabled,
		profile: defaultProfile,
		key: "quick_replace_enabled",
		defaultValue: false,
		normalize: boolOrNull,
	});
	const quickReplaceProviderView = inheritedSettingView<string | null>({
		globalValue: settings?.llm_provider,
		profile: defaultProfile,
		key: "quick_replace_provider",
		defaultValue: null,
		normalize: stringOrNull,
	});
	const quickReplaceModelView = inheritedSettingView<string | null>({
		globalValue: settings?.llm_model,
		profile: defaultProfile,
		key: "quick_replace_model",
		defaultValue: null,
		normalize: stringOrNull,
	});
	const quickReplaceSystemPromptView = inheritedSettingView({
		globalValue: undefined,
		profile: defaultProfile,
		key: "quick_replace_system_prompt",
		defaultValue: defaultQuickReplaceSystemPrompt,
		normalize: stringOrNull,
	});
	const rewriteIncludeClipboardContextView = inheritedSettingView({
		globalValue: undefined,
		profile: defaultProfile,
		key: "rewrite_include_clipboard_context",
		defaultValue: false,
		normalize: boolOrNull,
	});
	const quickReplaceIncludeClipboardContextView = inheritedSettingView({
		globalValue: undefined,
		profile: defaultProfile,
		key: "quick_replace_include_clipboard_context",
		defaultValue: false,
		normalize: boolOrNull,
	});
	const quickAskIncludeClipboardContextView = inheritedSettingView({
		globalValue: undefined,
		profile: defaultProfile,
		key: "quick_ask_include_clipboard_context",
		defaultValue: false,
		normalize: boolOrNull,
	});
	const rewriteActiveWindowOcrModeView = inheritedSettingView({
		globalValue: settings?.rewrite_active_window_ocr_mode,
		profile: defaultProfile,
		key: "rewrite_active_window_ocr_mode",
		defaultValue: "off" as const,
		normalize: activeWindowOcrModeOrNull,
	});
	const quickReplaceActiveWindowOcrModeView = inheritedSettingView({
		globalValue: settings?.quick_replace_active_window_ocr_mode,
		profile: defaultProfile,
		key: "quick_replace_active_window_ocr_mode",
		defaultValue: "off" as const,
		normalize: activeWindowOcrModeOrNull,
	});
	const quickAskActiveWindowOcrModeView = inheritedSettingView({
		globalValue: settings?.quick_ask_active_window_ocr_mode,
		profile: defaultProfile,
		key: "quick_ask_active_window_ocr_mode",
		defaultValue: "off" as const,
		normalize: activeWindowOcrModeOrNull,
	});
	const quickAskDismissModeView = inheritedSettingView({
		globalValue: quickAskDismissModeOrDefault(
			settings?.quick_ask_dismiss_mode,
			"manual",
		),
		profile: defaultProfile,
		key: "quick_ask_dismiss_mode",
		defaultValue: "manual" as const,
		normalize: quickAskDismissModeOrNull,
	});

	return {
		baseQuickReplaceEnabled: quickReplaceEnabledView.value,
		baseQuickReplaceProvider: quickReplaceProviderView.value,
		baseQuickReplaceModel: quickReplaceModelView.value,
		baseQuickReplaceSystemPrompt: quickReplaceSystemPromptView.value,
		baseRewriteIncludeClipboardContext:
			rewriteIncludeClipboardContextView.value,
		baseQuickReplaceIncludeClipboardContext:
			quickReplaceIncludeClipboardContextView.value,
		baseQuickAskIncludeClipboardContext:
			quickAskIncludeClipboardContextView.value,
		baseRewriteActiveWindowOcrMode: rewriteActiveWindowOcrModeView.value,
		baseQuickReplaceActiveWindowOcrMode:
			quickReplaceActiveWindowOcrModeView.value,
		baseQuickAskActiveWindowOcrMode: quickAskActiveWindowOcrModeView.value,
		baseQuickAskDismissMode: quickAskDismissModeView.value,
	};
}
