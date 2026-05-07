import { DEFAULT_SETTINGS_VALUES } from "../settingsDefaults";
import type {
	LocalWhisperLoadMode,
	MainWindowCloseBehavior,
	OutputMode,
	OverlayMode,
	OverlayMonitorTarget,
	QuickAskDismissMode,
	WidgetPosition,
} from "../types";

export function normalizeOutputMode(value: unknown): OutputMode {
	if (
		value === "paste" ||
		value === "paste_and_clipboard" ||
		value === "clipboard"
	) {
		return value;
	}

	// Legacy/disabled values:
	// - "keystrokes"
	// - "keystrokes_and_clipboard"
	// - "auto_paste"
	return DEFAULT_SETTINGS_VALUES.output_mode;
}

export function normalizeOverlayModeValue(value: unknown): OverlayMode | null {
	if (value === "always" || value === "never" || value === "recording_only") {
		return value;
	}
	return null;
}

export function normalizeOverlayMonitorTarget(
	value: unknown,
): OverlayMonitorTarget {
	if (value === "main" || value === "cursor" || value === "active_window") {
		return value;
	}

	// Legacy / typo-tolerant values.
	if (value === "activeWindow") return "active_window";

	return DEFAULT_SETTINGS_VALUES.overlay_monitor_target;
}

export function normalizeWidgetPosition(value: unknown): WidgetPosition | null {
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

export function normalizeLocalWhisperModelId(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	return trimmed.toLowerCase();
}

export function normalizeLocalWhisperLoadMode(
	value: unknown,
): LocalWhisperLoadMode {
	if (
		value === "manual" ||
		value === "on_transcribe" ||
		value === "on_launch"
	) {
		return value;
	}
	return DEFAULT_SETTINGS_VALUES.local_whisper_load_mode;
}

export function normalizeMainWindowCloseBehavior(
	value: unknown,
): MainWindowCloseBehavior {
	if (value === "minimize_to_tray" || value === "exit_program") return value;

	// Legacy value (kept for backward compatibility).
	if (value === "close_window") return "minimize_to_tray";

	return DEFAULT_SETTINGS_VALUES.main_window_close_behavior;
}

export function normalizeQuickAskDismissMode(
	value: unknown,
): QuickAskDismissMode {
	if (value === "manual" || value === "auto") return value;
	return DEFAULT_SETTINGS_VALUES.quick_ask_dismiss_mode;
}

export function normalizeQuickAskDismissModeOverride(
	value: unknown,
): QuickAskDismissMode | null {
	if (value === "manual" || value === "auto") return value;
	return null;
}

export function normalizeQuickAskConversationHistoryCount(
	raw: unknown,
): number {
	// Default to 3; keep it small to avoid runaway token usage.
	const n =
		typeof raw === "number" && Number.isFinite(raw)
			? raw
			: DEFAULT_SETTINGS_VALUES.quick_ask_conversation_history_count;
	// Allow fractional store values but normalize to an integer.
	const rounded = Math.round(n);
	return Math.min(20, Math.max(1, rounded));
}
