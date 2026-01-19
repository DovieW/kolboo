import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { DEFAULT_ACCENT_HEX, normalizeHexColor } from "../accentColor";
import {
	DEFAULT_HOLD_HOTKEY,
	DEFAULT_PASTE_LAST_HOTKEY,
	DEFAULT_QUICK_ASK_HOLD_HOTKEY,
	DEFAULT_QUICK_ASK_TOGGLE_HOTKEY,
	DEFAULT_RETRY_HOTKEY,
	DEFAULT_TOGGLE_HOTKEY,
} from "../hotkeyDefaults";
import { type HotkeyConfig, normalizeHotkeyConfig } from "../hotkeys";
import { emitTyped } from "./events";
import type {
	AppSettings,
	AudioCue,
	CleanupPromptSections,
	CleanupPromptSectionsOverride,
	ContextGrabMethod,
	IntentRouterSettings,
	LocalWhisperLoadMode,
	MainWindowCloseBehavior,
	OpenAiReasoningEffort,
	OutputMode,
	OverlayMode,
	OverlayMonitorTarget,
	PlayingAudioHandling,
	ProxySettings,
	RewritePreset,
	RewriteProgramPromptProfile,
	SettingsGuideState,
	TranscriptionRetentionUnit,
	WidgetPosition,
} from "./types";

const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object" && !Array.isArray(value);
};

function normalizeIntentRouterStrategy(
	value: unknown,
): IntentRouterSettings["strategy"] {
	if (value === "off" || value === "embeddings" || value === "llm")
		return value;
	return "off";
}

function normalizeIntentRouterSettings(value: unknown): IntentRouterSettings {
	const v = isRecord(value) ? value : {};
	const enabled = typeof v.enabled === "boolean" ? v.enabled : false;
	const strategy = normalizeIntentRouterStrategy(v.strategy);

	const embedding_provider =
		v.embedding_provider === "openai" ||
		v.embedding_provider === "cohere" ||
		v.embedding_provider === "fireworks"
			? (v.embedding_provider as "openai" | "cohere" | "fireworks")
			: null;
	const embedding_model =
		typeof v.embedding_model === "string" ? v.embedding_model : null;

	const pick_highest_score =
		typeof v.pick_highest_score === "boolean" ? v.pick_highest_score : null;

	const similarity_threshold =
		typeof v.similarity_threshold === "number" &&
		Number.isFinite(v.similarity_threshold)
			? v.similarity_threshold
			: null;
	const similarity_margin =
		typeof v.similarity_margin === "number" &&
		Number.isFinite(v.similarity_margin)
			? v.similarity_margin
			: null;

	const llm_provider =
		typeof v.llm_provider === "string" ? v.llm_provider : null;
	const llm_model = typeof v.llm_model === "string" ? v.llm_model : null;

	const openai_reasoning_effort = normalizeOpenAiReasoningEffort(
		v.openai_reasoning_effort,
	);
	const gemini_thinking_budget = normalizeGeminiThinkingBudget(
		v.gemini_thinking_budget,
	);
	const gemini_thinking_level = normalizeGeminiThinkingLevel(
		v.gemini_thinking_level,
	);
	const anthropic_thinking_budget = normalizeAnthropicThinkingBudget(
		v.anthropic_thinking_budget,
	);

	const llm_system_prompt =
		typeof v.llm_system_prompt === "string" ? v.llm_system_prompt : null;

	return {
		enabled,
		strategy,
		embedding_provider,
		embedding_model,
		pick_highest_score,
		similarity_threshold,
		similarity_margin,
		llm_provider,
		llm_model,
		openai_reasoning_effort,
		gemini_thinking_budget,
		gemini_thinking_level,
		anthropic_thinking_budget,
		llm_system_prompt,
	};
}

function normalizeRewritePreset(value: unknown): RewritePreset | null {
	const p = isRecord(value) ? value : null;
	if (!p) return null;
	const id = typeof p.id === "string" ? p.id : "";
	const name = typeof p.name === "string" ? p.name : "";
	if (!id) return null;

	const description =
		typeof p.description === "string" && p.description.trim().length > 0
			? p.description
			: null;
	const routing_hints = Array.isArray(p.routing_hints)
		? p.routing_hints
				.map((x) => (typeof x === "string" ? x.trim() : ""))
				.filter(Boolean)
		: null;

	const cleanup_prompt_sections =
		p.cleanup_prompt_sections && typeof p.cleanup_prompt_sections === "object"
			? // NOTE: This is normalized again inside getSettings(). Here we only ensure
				// it's either a well-formed override or null.
				(p.cleanup_prompt_sections as CleanupPromptSectionsOverride)
			: null;

	// Backward compatible: older settings may omit this field or write null.
	// Backend defaults missing/null to true.
	const rewrite_llm_enabled =
		typeof p.rewrite_llm_enabled === "boolean" ? p.rewrite_llm_enabled : true;
	const stt_provider =
		typeof p.stt_provider === "string" ? p.stt_provider : null;
	const stt_model = typeof p.stt_model === "string" ? p.stt_model : null;
	const stt_timeout_seconds =
		typeof p.stt_timeout_seconds === "number" &&
		Number.isFinite(p.stt_timeout_seconds)
			? p.stt_timeout_seconds
			: null;
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
	const anthropic_thinking_budget = normalizeAnthropicThinkingBudget(
		p.anthropic_thinking_budget,
	);

	const sound_enabled =
		typeof p.sound_enabled === "boolean" ? p.sound_enabled : null;
	const playing_audio_handling =
		typeof p.playing_audio_handling === "string"
			? normalizePlayingAudioHandling(p.playing_audio_handling)
			: null;
	const overlay_mode =
		typeof p.overlay_mode === "string"
			? normalizeOverlayMode(p.overlay_mode)
			: null;
	const widget_position =
		typeof p.widget_position === "string" &&
		(p.widget_position === "center" ||
			p.widget_position === "top-left" ||
			p.widget_position === "top-center" ||
			p.widget_position === "top-right" ||
			p.widget_position === "bottom-left" ||
			p.widget_position === "bottom-center" ||
			p.widget_position === "bottom-right")
			? (p.widget_position as WidgetPosition)
			: null;
	const output_mode =
		typeof p.output_mode === "string"
			? normalizeOutputMode(p.output_mode)
			: null;
	const output_hit_enter =
		typeof p.output_hit_enter === "boolean" ? p.output_hit_enter : null;

	return {
		id,
		name,
		description,
		routing_hints,
		cleanup_prompt_sections,
		rewrite_llm_enabled,
		stt_provider,
		stt_model,
		stt_timeout_seconds,
		llm_provider,
		llm_model,
		openai_reasoning_effort,
		gemini_thinking_budget,
		gemini_thinking_level,
		anthropic_thinking_budget,
		sound_enabled,
		playing_audio_handling,
		overlay_mode,
		widget_position,
		output_mode,
		output_hit_enter,
	};
}

function normalizeOutputMode(value: unknown): OutputMode {
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
	return "paste";
}

function normalizeOverlayMode(value: unknown): OverlayMode {
	if (value === "always" || value === "never" || value === "recording_only") {
		return value;
	}
	return "recording_only";
}

function normalizeOverlayMonitorTarget(value: unknown): OverlayMonitorTarget {
	if (value === "main" || value === "cursor" || value === "active_window") {
		return value;
	}

	// Legacy / typo-tolerant values
	if (value === "activeWindow") return "active_window";

	return "main";
}

function normalizeLocalWhisperModelId(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	return trimmed.toLowerCase();
}

function normalizeLocalWhisperLoadMode(value: unknown): LocalWhisperLoadMode {
	if (
		value === "manual" ||
		value === "on_transcribe" ||
		value === "on_launch"
	) {
		return value;
	}
	return "manual";
}

function normalizeMainWindowCloseBehavior(
	value: unknown,
): MainWindowCloseBehavior {
	if (value === "minimize_to_tray" || value === "exit_program") return value;

	// Legacy value (kept for backward compatibility)
	if (value === "close_window") return "minimize_to_tray";

	// Default for unknown/missing values
	return "minimize_to_tray";
}

function normalizeQuickAskConversationHistoryCount(raw: unknown): number {
	// Default to 3; keep it small to avoid runaway token usage.
	const n = typeof raw === "number" && Number.isFinite(raw) ? raw : 3;
	// Allow fractional store values but normalize to an integer.
	const rounded = Math.round(n);
	return Math.min(20, Math.max(1, rounded));
}

function normalizeProxyMode(value: unknown): ProxySettings["mode"] {
	if (value === "no_proxy" || value === "system" || value === "manual") {
		return value;
	}
	return "system";
}

function normalizeManualProxySettings(value: unknown): ProxySettings["manual"] {
	const v = isRecord(value) ? value : {};

	const proxy_url = typeof v.proxy_url === "string" ? v.proxy_url : "";
	const no_proxy =
		typeof v.no_proxy === "string" ? v.no_proxy : "localhost,127.0.0.1";
	const username = typeof v.username === "string" ? v.username : "";
	const password = typeof v.password === "string" ? v.password : "";

	return { proxy_url, no_proxy, username, password };
}

function normalizeProxySettings(value: unknown): ProxySettings {
	const v = isRecord(value) ? value : {};
	const mode = normalizeProxyMode(v.mode);
	const manual = normalizeManualProxySettings(v.manual);

	const normalizeTrustedCaCertFormat = (value: unknown) =>
		value === "der" ? "der" : "pem";

	const normalizeTrustedCaCertificate = (
		value: unknown,
	): ProxySettings["trusted_ca_certificates"][number] | null => {
		if (!isRecord(value)) return null;
		const x = value;
		const id = typeof x.id === "string" ? x.id : "";
		const file_name = typeof x.file_name === "string" ? x.file_name : "";
		const format = normalizeTrustedCaCertFormat(x.format);
		const data_base64 = typeof x.data_base64 === "string" ? x.data_base64 : "";
		if (!id || !data_base64) return null;
		return { id, file_name, format, data_base64 };
	};

	const trusted_ca_certificates: ProxySettings["trusted_ca_certificates"] =
		Array.isArray(v.trusted_ca_certificates)
			? (v.trusted_ca_certificates as unknown[])
					.map(normalizeTrustedCaCertificate)
					.filter(
						(c): c is ProxySettings["trusted_ca_certificates"][number] =>
							c !== null,
					)
			: [];

	const danger_accept_invalid_certs =
		typeof v.danger_accept_invalid_certs === "boolean"
			? v.danger_accept_invalid_certs
			: false;

	return {
		mode,
		manual,
		trusted_ca_certificates,
		danger_accept_invalid_certs,
	};
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

	// Legacy boolean (auto_mute_audio) migration:
	// - true  => mute
	// - false => none
	if (typeof value === "boolean") {
		return value ? "mute" : "none";
	}

	// Default for fresh installs / missing setting
	return "none";
}

function normalizeAudioCue(value: unknown): AudioCue {
	if (
		value === "kolboo" ||
		value === "maraca" ||
		value === "clave" ||
		value === "legacy"
	) {
		return value;
	}

	// Default for fresh installs / missing setting
	return "kolboo";
}

function normalizeNoiseGateStrength(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 0;
	const rounded = Math.round(value);
	return Math.min(100, Math.max(0, rounded));
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
	if (v === "minimal" || v === "low" || v === "medium" || v === "high")
		return v;
	return null;
}

function normalizeGeminiThinkingBudget(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Keep it integer-ish (Gemini expects an integer token budget).
	return Math.trunc(value);
}

function normalizeAnthropicThinkingBudget(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Keep it integer-ish; Anthropic expects an integer token budget.
	const n = Math.trunc(value);
	// The cookbook notes a minimum budget of 1024 for extended thinking.
	if (n < 1024) return 1024;
	// Defensive cap; actual max varies by model.
	return Math.min(32768, n);
}

function normalizeAnthropicThinkingBudgetAllowOff(
	value: unknown,
): number | null {
	// For per-profile overrides we want an explicit "off" state even if the
	// Default/global setting enables thinking. Represent that as 0.
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	const n = Math.trunc(value);
	if (n <= 0) return 0;
	if (n < 1024) return 1024;
	return Math.min(32768, n);
}

function normalizeNoiseGateThresholdDbfs(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Clamp to the UI range.
	return Math.min(-30, Math.max(-75, value));
}

function noiseGateStrengthToThresholdDbfs(strength: number): number | null {
	const s = normalizeNoiseGateStrength(strength);
	if (s <= 0) return null;
	// Map 1..100 => -75..-30 (same range as the Rust mapping).
	const t = -75 + (s / 100) * 45;
	return Math.min(-30, Math.max(-75, t));
}

function noiseGateThresholdDbfsToStrength(
	thresholdDbfs: number | null,
): number {
	if (thresholdDbfs == null) return 0;
	const t = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
	if (t == null) return 0;
	const s = ((t + 75) / 45) * 100;
	// Never return 0 when enabled; old UI treated 0 as off.
	return Math.min(100, Math.max(1, Math.round(s)));
}

function normalizeMaxSavedRecordings(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 1000;
	const rounded = Math.round(value);
	// 1..100000 (defensive)
	return Math.min(100000, Math.max(1, rounded));
}

function normalizeTranscriptionRetentionUnit(
	value: unknown,
): TranscriptionRetentionUnit {
	if (value === "days" || value === "hours") return value;
	return "days";
}

function normalizeTranscriptionRetentionValue(
	value: unknown,
	unit: TranscriptionRetentionUnit,
): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 0;
	const clamped = Math.max(0, value);

	if (unit === "days") {
		const rounded = Math.round(clamped);
		// 0..36500 days (~100 years) defensive cap
		return Math.min(36500, Math.max(0, rounded));
	}

	// hours: allow decimals (e.g. 0.5). Cap at ~100 years worth of hours.
	const maxHours = 36500 * 24;
	return Math.min(maxHours, clamped);
}

function normalizeTranscriptionRetentionDeleteRecordings(
	value: unknown,
): boolean {
	return typeof value === "boolean" ? value : false;
}

function normalizeStatsRetentionMaxBytes(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 50_000_000;
	const rounded = Math.round(value);
	// 1MB..5GB (defensive)
	return Math.min(5_000_000_000, Math.max(1_000_000, rounded));
}

function normalizeRequestLogsRetentionMode(
	value: unknown,
): AppSettings["request_logs_retention_mode"] {
	return value === "time" || value === "amount" ? value : "amount";
}

function normalizeRequestLogsRetentionAmount(value: unknown): number {
	// Keep this modest to avoid runaway memory in the backend.
	if (typeof value !== "number" || !Number.isFinite(value)) return 50;
	const rounded = Math.round(value);
	// 1..1000 defensive
	return Math.min(1000, Math.max(1, rounded));
}

function normalizeRequestLogsRetentionDays(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 7;
	const rounded = Math.round(value);
	// 0..36500 (~100 years) defensive
	return Math.min(36500, Math.max(0, rounded));
}

export const defaultToggleHotkey = DEFAULT_TOGGLE_HOTKEY;
export const defaultHoldHotkey = DEFAULT_HOLD_HOTKEY;
export const defaultPasteLastHotkey = DEFAULT_PASTE_LAST_HOTKEY;
export const defaultRetryHotkey = DEFAULT_RETRY_HOTKEY;
export const defaultQuickAskHoldHotkey = DEFAULT_QUICK_ASK_HOLD_HOTKEY;
export const defaultQuickAskToggleHotkey = DEFAULT_QUICK_ASK_TOGGLE_HOTKEY;

let storeInstance: Store | null = null;

const SETTINGS_GUIDE_STATE_KEY = "settings_guide_state";
const SETTINGS_VERSION_KEY = "settings_version";
// Bump when adding settings migrations; keep TS/Rust/tests in sync.
const SETTINGS_VERSION_DEFAULT = 1;

function normalizeSettingsGuideState(value: unknown): SettingsGuideState {
	if (value === "pending" || value === "skipped" || value === "completed") {
		return value;
	}
	return "pending";
}

function normalizeSettingsVersion(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return SETTINGS_VERSION_DEFAULT;
	}
	const normalized = Math.trunc(value);
	if (normalized < SETTINGS_VERSION_DEFAULT) {
		return SETTINGS_VERSION_DEFAULT;
	}
	return normalized;
}

async function getStore(): Promise<Store> {
	if (!storeInstance) {
		storeInstance = await Store.load("settings.json");
	}
	return storeInstance;
}

export const tauriSettingsAPI = {
	async getSettings(): Promise<AppSettings> {
		const store = await getStore();

		const rawSettingsVersion = await store.get(SETTINGS_VERSION_KEY);
		const settingsVersion = normalizeSettingsVersion(rawSettingsVersion);
		if (rawSettingsVersion !== settingsVersion) {
			await store.set(SETTINGS_VERSION_KEY, settingsVersion);
			await store.save();
		}

		// Keep a tiny subset of settings mirrored in localStorage so the UI can apply
		// critical visuals (accent color) before the async store read completes.
		// This reduces first-paint flicker on startup.
		const tryWriteLocalStorage = (key: string, value: string | null) => {
			try {
				if (typeof window === "undefined") return;
				if (!window.localStorage) return;
				if (value === null) {
					window.localStorage.removeItem(key);
				} else {
					window.localStorage.setItem(key, value);
				}
			} catch {
				// ignore (private mode / disabled storage)
			}
		};

		const LOCAL_ACCENT_COLOR_KEY = "tv_accent_color";

		const normalizePromptSection = (value: unknown) => {
			if (value === null) return null;
			if (!isRecord(value)) return null;
			const content = typeof value.content === "string" ? value.content : null;

			return { content };
		};

		const normalizeCleanupPromptSections = (
			value: unknown,
		): CleanupPromptSections | null => {
			if (value === null || value === undefined) return null;
			if (!isRecord(value)) return null;
			const v = value;

			// New shape
			if (Object.hasOwn(v, "system")) {
				const system = normalizePromptSection(v.system) ?? { content: null };
				return { system };
			}

			// Legacy shape: { main, advanced, dictionary }
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
		};

		const normalizeCleanupPromptSectionsOverride = (
			value: unknown,
		): CleanupPromptSectionsOverride | null => {
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
		};

		const normalizeRewriteProfile = (
			p: unknown,
		): RewriteProgramPromptProfile | null => {
			if (!isRecord(p)) return null;
			const id = typeof p.id === "string" ? p.id : "";
			const name = typeof p.name === "string" ? p.name : "";

			const program_paths_raw = p.program_paths;
			const legacy_program_path = p.program_path;

			const program_paths = Array.isArray(program_paths_raw)
				? program_paths_raw.filter((x) => typeof x === "string")
				: typeof legacy_program_path === "string" &&
						legacy_program_path.length > 0
					? [legacy_program_path]
					: [];

			const cleanup_prompt_sections = normalizeCleanupPromptSectionsOverride(
				p.cleanup_prompt_sections,
			);
			const stt_provider =
				typeof p.stt_provider === "string" ? p.stt_provider : null;
			const stt_model = typeof p.stt_model === "string" ? p.stt_model : null;
			const stt_timeout_seconds =
				typeof p.stt_timeout_seconds === "number"
					? p.stt_timeout_seconds
					: null;
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
			const anthropic_thinking_budget =
				normalizeAnthropicThinkingBudgetAllowOff(p.anthropic_thinking_budget);

			const quick_ask_provider =
				typeof p.quick_ask_provider === "string" ? p.quick_ask_provider : null;
			const quick_ask_model =
				typeof p.quick_ask_model === "string" ? p.quick_ask_model : null;
			const quick_ask_system_prompt_raw = p.quick_ask_system_prompt;
			const quick_ask_system_prompt =
				typeof quick_ask_system_prompt_raw === "string" &&
				quick_ask_system_prompt_raw.trim().length > 0
					? quick_ask_system_prompt_raw
					: null;

			const context_grab_method_raw = p.context_grab_method;
			const context_grab_method: ContextGrabMethod | null =
				context_grab_method_raw === "none" ||
				context_grab_method_raw === "ctrl_c" ||
				context_grab_method_raw === "ctrl_shift_c" ||
				context_grab_method_raw === "ctrl_insert" ||
				context_grab_method_raw === "clipboard_only"
					? (context_grab_method_raw as ContextGrabMethod)
					: null;

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

			const quick_replace_enabled =
				typeof p.quick_replace_enabled === "boolean"
					? p.quick_replace_enabled
					: null;
			const quick_replace_provider =
				typeof p.quick_replace_provider === "string"
					? p.quick_replace_provider
					: null;
			const quick_replace_model =
				typeof p.quick_replace_model === "string"
					? p.quick_replace_model
					: null;
			const quick_replace_system_prompt_raw = p.quick_replace_system_prompt;
			const quick_replace_system_prompt =
				typeof quick_replace_system_prompt_raw === "string" &&
				quick_replace_system_prompt_raw.trim().length > 0
					? quick_replace_system_prompt_raw
					: null;

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
				typeof p.rewrite_llm_enabled === "boolean"
					? p.rewrite_llm_enabled
					: null;

			const sound_enabled =
				typeof p.sound_enabled === "boolean" ? p.sound_enabled : null;
			const playing_audio_handling_raw = p.playing_audio_handling;
			const legacy_auto_mute_audio = p.auto_mute_audio;

			const playing_audio_handling: PlayingAudioHandling | null =
				typeof playing_audio_handling_raw === "string"
					? normalizePlayingAudioHandling(playing_audio_handling_raw)
					: typeof legacy_auto_mute_audio === "boolean"
						? legacy_auto_mute_audio
							? "mute"
							: "none"
						: null;

			const overlay_mode =
				p.overlay_mode === "always" ||
				p.overlay_mode === "never" ||
				p.overlay_mode === "recording_only"
					? (p.overlay_mode as OverlayMode)
					: null;

			const widget_position =
				p.widget_position === "center" ||
				p.widget_position === "top-left" ||
				p.widget_position === "top-center" ||
				p.widget_position === "top-right" ||
				p.widget_position === "bottom-left" ||
				p.widget_position === "bottom-center" ||
				p.widget_position === "bottom-right"
					? (p.widget_position as WidgetPosition)
					: null;

			const output_mode =
				typeof p.output_mode === "string"
					? normalizeOutputMode(p.output_mode)
					: null;

			const output_hit_enter =
				typeof p.output_hit_enter === "boolean" ? p.output_hit_enter : null;

			const presets_raw = p.presets;
			const presets: RewritePreset[] | null = Array.isArray(presets_raw)
				? presets_raw
						.map(normalizeRewritePreset)
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
				cleanup_prompt_sections,

				presets,
				default_preset_id,
				default_preset_description,
				router,
				active_preset_id,

				rewrite_llm_enabled,
				stt_provider,
				stt_model,
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
				context_grab_method,
				rewrite_include_clipboard_context,
				quick_replace_include_clipboard_context,
				quick_ask_include_clipboard_context,
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
		};

		const rawProfiles =
			(await store.get<unknown>("rewrite_program_prompt_profiles")) ?? [];
		const rewrite_program_prompt_profiles: RewriteProgramPromptProfile[] =
			Array.isArray(rawProfiles)
				? rawProfiles
						.map(normalizeRewriteProfile)
						.filter((p): p is RewriteProgramPromptProfile => p !== null)
				: [];

		// Backward compatibility:
		// - Legacy key: quick_ask_hotkey (hold-to-record)
		// - New keys: quick_ask_hold_hotkey + quick_ask_toggle_hotkey
		// IMPORTANT: explicit null means "disabled" and must NOT fall back.
		const rawQuickAskHold = await store.get("quick_ask_hold_hotkey");
		const rawQuickAskHoldEffective =
			rawQuickAskHold === undefined
				? await store.get("quick_ask_hotkey")
				: rawQuickAskHold;

		const settings: AppSettings = {
			settings_version: settingsVersion,
			toggle_hotkey: normalizeHotkeyConfig(
				await store.get("toggle_hotkey"),
				defaultToggleHotkey,
			),
			hold_hotkey: normalizeHotkeyConfig(
				await store.get("hold_hotkey"),
				defaultHoldHotkey,
			),
			paste_last_hotkey: normalizeHotkeyConfig(
				await store.get("paste_last_hotkey"),
				defaultPasteLastHotkey,
			),
			retry_hotkey: normalizeHotkeyConfig(
				await store.get("retry_hotkey"),
				defaultRetryHotkey,
			),
			quick_ask_hold_hotkey: normalizeHotkeyConfig(
				rawQuickAskHoldEffective,
				defaultQuickAskHoldHotkey,
			),
			quick_ask_toggle_hotkey: normalizeHotkeyConfig(
				await store.get("quick_ask_toggle_hotkey"),
				defaultQuickAskToggleHotkey,
			),

			hotkey_debug_enabled:
				(await store.get<boolean>("hotkey_debug_enabled")) ?? false,

			selected_mic_id:
				(await store.get<string | null>("selected_mic_id")) ?? null,
			sound_enabled: (await store.get<boolean>("sound_enabled")) ?? true,
			audio_cue: normalizeAudioCue(await store.get("audio_cue")),
			accent_color: await (async () => {
				const raw = (await store.get<string | null>("accent_color")) ?? null;
				const normalized = normalizeHexColor(raw);

				// If unset/invalid, default to the app's default accent.
				// (Tangerine is an explicit option in the UI, not the implicit default.)
				if (!normalized) {
					await store.set("accent_color", DEFAULT_ACCENT_HEX);
					await store.save();
					return DEFAULT_ACCENT_HEX;
				}

				return normalized;
			})(),
			rewrite_llm_enabled:
				(await store.get<boolean>("rewrite_llm_enabled")) ?? false,
			quick_replace_enabled:
				(await store.get<boolean>("quick_replace_enabled")) ?? false,
			cleanup_prompt_sections: await (async () => {
				const raw = await store.get<unknown>("cleanup_prompt_sections");
				const normalized = normalizeCleanupPromptSections(raw);

				// If we had legacy/invalid shapes, write back the normalized value to
				// avoid runtime errors and keep the store clean.
				const rawIsObject = raw && typeof raw === "object";
				const rawHasSystem = rawIsObject ? Object.hasOwn(raw, "system") : false;
				const rawHasLegacyMain = rawIsObject
					? Object.hasOwn(raw, "main")
					: false;

				const rawJson = rawIsObject ? JSON.stringify(raw) : null;
				const normalizedJson = normalized ? JSON.stringify(normalized) : null;

				if (
					normalized &&
					(rawHasLegacyMain ||
						(rawIsObject && !rawHasSystem) ||
						rawJson !== normalizedJson)
				) {
					await store.set("cleanup_prompt_sections", normalized);
					await store.save();
				}

				return normalized;
			})(),
			rewrite_program_prompt_profiles,
			stt_provider: (await store.get<string | null>("stt_provider")) ?? null,
			stt_model: (await store.get<string | null>("stt_model")) ?? null,
			stt_transcription_prompt:
				(await store.get<string | null>("stt_transcription_prompt")) ?? null,
			aquavoice_base_url:
				(await store.get<string | null>("aquavoice_base_url")) ?? null,
			whisper_server_base_url:
				(await store.get<string | null>("whisper_server_base_url")) ?? null,
			ollama_url: (await store.get<string | null>("ollama_url")) ?? null,
			local_whisper_model_id: normalizeLocalWhisperModelId(
				await store.get("local_whisper_model_id"),
			),
			local_whisper_load_mode: normalizeLocalWhisperLoadMode(
				await store.get("local_whisper_load_mode"),
			),
			proxy_settings: normalizeProxySettings(await store.get("proxy_settings")),
			llm_provider: (await store.get<string | null>("llm_provider")) ?? null,
			llm_model: (await store.get<string | null>("llm_model")) ?? null,

			quick_ask_provider:
				(await store.get<string | null>("quick_ask_provider")) ?? null,
			quick_ask_model:
				(await store.get<string | null>("quick_ask_model")) ?? null,
			quick_ask_system_prompt:
				(await store.get<string | null>("quick_ask_system_prompt")) ?? null,

			quick_ask_include_selected_text:
				(await store.get<boolean>("quick_ask_include_selected_text")) ?? false,

			quick_ask_conversation_history_enabled:
				(await store.get<boolean>("quick_ask_conversation_history_enabled")) ??
				true,
			quick_ask_conversation_history_count:
				normalizeQuickAskConversationHistoryCount(
					await store.get("quick_ask_conversation_history_count"),
				),

			quick_ask_openai_reasoning_effort: normalizeOpenAiReasoningEffort(
				await store.get("quick_ask_openai_reasoning_effort"),
			),
			quick_ask_anthropic_thinking_budget: normalizeAnthropicThinkingBudget(
				await store.get("quick_ask_anthropic_thinking_budget"),
			),
			quick_ask_gemini_thinking_budget: normalizeGeminiThinkingBudget(
				await store.get("quick_ask_gemini_thinking_budget"),
			),
			quick_ask_gemini_thinking_level: normalizeGeminiThinkingLevel(
				await store.get("quick_ask_gemini_thinking_level"),
			),
			cerebras_free_tier:
				(await store.get<boolean>("cerebras_free_tier")) ?? true,
			groq_free_tier: (await store.get<boolean>("groq_free_tier")) ?? true,
			elevenlabs_free_tier:
				(await store.get<boolean>("elevenlabs_free_tier")) ?? true,
			cohere_free_tier: (await store.get<boolean>("cohere_free_tier")) ?? true,
			assemblyai_free_tier:
				(await store.get<boolean>("assemblyai_free_tier")) ?? true,
			speechmatics_free_tier:
				(await store.get<boolean>("speechmatics_free_tier")) ?? true,
			openai_reasoning_effort: normalizeOpenAiReasoningEffort(
				await store.get("openai_reasoning_effort"),
			),
			anthropic_thinking_budget: normalizeAnthropicThinkingBudget(
				await store.get("anthropic_thinking_budget"),
			),
			gemini_thinking_budget: normalizeGeminiThinkingBudget(
				await store.get("gemini_thinking_budget"),
			),
			gemini_thinking_level: normalizeGeminiThinkingLevel(
				await store.get("gemini_thinking_level"),
			),
			playing_audio_handling: normalizePlayingAudioHandling(
				(await store.get("playing_audio_handling")) ??
					// Legacy key for migration:
					(await store.get<boolean>("auto_mute_audio")) ??
					// If neither exists, default to none
					"none",
			),
			stt_timeout_seconds:
				(await store.get<number | null>("stt_timeout_seconds")) ?? null,
			overlay_mode:
				(await store.get<OverlayMode>("overlay_mode")) ?? "recording_only",
			overlay_show_detailed_loading:
				(await store.get<boolean>("overlay_show_detailed_loading")) ?? false,
			overlay_monitor_target: normalizeOverlayMonitorTarget(
				(await store.get("overlay_monitor_target")) ?? "main",
			),
			widget_position:
				(await store.get<WidgetPosition>("widget_position")) ?? "bottom-center",
			output_mode: normalizeOutputMode(await store.get("output_mode")),
			output_hit_enter: (await store.get<boolean>("output_hit_enter")) ?? false,
			output_clipboard_privacy_mode:
				(await store.get<boolean>("output_clipboard_privacy_mode")) ?? false,

			main_window_close_behavior: normalizeMainWindowCloseBehavior(
				await store.get("main_window_close_behavior"),
			),

			quiet_audio_gate_enabled:
				(await store.get<boolean>("quiet_audio_gate_enabled")) ?? true,
			quiet_audio_min_duration_secs:
				(await store.get<number>("quiet_audio_min_duration_secs")) ?? 0.15,
			quiet_audio_rms_dbfs_threshold:
				(await store.get<number>("quiet_audio_rms_dbfs_threshold")) ?? -60,
			quiet_audio_peak_dbfs_threshold:
				(await store.get<number>("quiet_audio_peak_dbfs_threshold")) ?? -50,
			quiet_audio_require_speech:
				(await store.get<boolean>("quiet_audio_require_speech")) ?? false,

			hot_mic_enabled: (await store.get<boolean>("hot_mic_enabled")) ?? false,
			hot_mic_pre_roll_ms:
				(await store.get<number>("hot_mic_pre_roll_ms")) ?? 1500,
			mic_auto_recover_enabled:
				(await store.get<boolean>("mic_auto_recover_enabled")) ?? false,

			noise_gate_threshold_dbfs: await (async () => {
				const configured = normalizeNoiseGateThresholdDbfs(
					await store.get("noise_gate_threshold_dbfs"),
				);
				if (configured != null) return configured;

				// Legacy fallback
				const legacyStrength = normalizeNoiseGateStrength(
					await store.get("noise_gate_strength"),
				);
				return noiseGateStrengthToThresholdDbfs(legacyStrength);
			})(),

			audio_downmix_to_mono:
				(await store.get<boolean>("audio_downmix_to_mono")) ?? true,
			audio_resample_to_16khz:
				(await store.get<boolean>("audio_resample_to_16khz")) ?? false,
			audio_highpass_enabled:
				(await store.get<boolean>("audio_highpass_enabled")) ?? true,
			audio_agc_enabled:
				(await store.get<boolean>("audio_agc_enabled")) ?? false,
			audio_noise_suppression_enabled:
				(await store.get<boolean>("audio_noise_suppression_enabled")) ?? false,

			max_saved_recordings: normalizeMaxSavedRecordings(
				await store.get("max_saved_recordings"),
			),

			request_logs_retention_mode: normalizeRequestLogsRetentionMode(
				await store.get("request_logs_retention_mode"),
			),
			request_logs_retention_amount: normalizeRequestLogsRetentionAmount(
				await store.get("request_logs_retention_amount"),
			),
			request_logs_retention_days: normalizeRequestLogsRetentionDays(
				await store.get("request_logs_retention_days"),
			),

			// Time retention: new (unit+value), with legacy fallback to transcription_retention_days.
			...(await (async () => {
				const rawUnit = await store.get("transcription_retention_unit");
				const rawValue = await store.get("transcription_retention_value");

				// Legacy installs only have days.
				if (rawUnit == null && rawValue == null) {
					const legacyDays = normalizeTranscriptionRetentionValue(
						await store.get("transcription_retention_days"),
						"days",
					);
					return {
						transcription_retention_unit: "days" as const,
						transcription_retention_value: legacyDays,
					};
				}

				const unit = normalizeTranscriptionRetentionUnit(rawUnit);
				const value = normalizeTranscriptionRetentionValue(rawValue, unit);
				return {
					transcription_retention_unit: unit,
					transcription_retention_value: value,
				};
			})()),
			transcription_retention_delete_recordings:
				normalizeTranscriptionRetentionDeleteRecordings(
					await store.get("transcription_retention_delete_recordings"),
				),

			// Stats retention (persisted on disk).
			...(await (async () => {
				const rawUnit = await store.get("stats_retention_unit");
				const rawValue = await store.get("stats_retention_value");

				const unit = normalizeTranscriptionRetentionUnit(rawUnit ?? "days");
				const value = normalizeTranscriptionRetentionValue(
					rawValue ?? 30,
					unit,
				);

				return {
					stats_retention_unit: unit,
					stats_retention_value: value,
				};
			})()),
			stats_retention_max_bytes: normalizeStatsRetentionMaxBytes(
				await store.get("stats_retention_max_bytes"),
			),

			// Backups
			github_backup_gist_id:
				(await store.get<string | null>("github_backup_gist_id")) ?? null,
		};

		// Mirror the accent so index.html can apply it synchronously at next launch.
		tryWriteLocalStorage(LOCAL_ACCENT_COLOR_KEY, settings.accent_color ?? null);

		return settings;
	},

	async reloadSettingsFromDisk(): Promise<void> {
		// @tauri-apps/plugin-store doesn't expose an instance reload API.
		// Recreate the Store instance so future reads come from disk.
		storeInstance = await Store.load("settings.json");
	},

	async updateAccentColor(color: string | null): Promise<void> {
		const store = await getStore();
		const normalized = normalizeHexColor(color);

		try {
			if (typeof window !== "undefined" && window.localStorage) {
				const LOCAL_ACCENT_COLOR_KEY = "tv_accent_color";
				if (!normalized) {
					window.localStorage.removeItem(LOCAL_ACCENT_COLOR_KEY);
				} else {
					window.localStorage.setItem(LOCAL_ACCENT_COLOR_KEY, normalized);
				}
			}
		} catch {
			// ignore
		}

		if (!normalized) {
			await store.delete("accent_color");
		} else {
			await store.set("accent_color", normalized);
		}

		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		// Include the new accent in the payload so the overlay can update immediately
		// without waiting for a disk reload.
		await emitTyped("settings-changed", { accent_color: normalized ?? null });
	},

	async updateMainWindowCloseBehavior(
		behavior: MainWindowCloseBehavior,
	): Promise<void> {
		const store = await getStore();
		const normalized = normalizeMainWindowCloseBehavior(behavior);
		await store.set("main_window_close_behavior", normalized);
		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {
			main_window_close_behavior: normalized,
		});
	},

	async updateGithubBackupGistId(gistId: string | null): Promise<void> {
		const store = await getStore();
		const trimmed = (gistId ?? "").trim();

		if (!trimmed) {
			await store.delete("github_backup_gist_id");
		} else {
			await store.set("github_backup_gist_id", trimmed);
		}

		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {
			github_backup_gist_id: trimmed || null,
		});
	},

	async updateToggleHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("toggle_hotkey", hotkey);
		await store.save();
	},

	async updateHoldHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("hold_hotkey", hotkey);
		await store.save();
	},

	async updatePasteLastHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("paste_last_hotkey", hotkey);
		await store.save();
	},

	async updateRetryHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("retry_hotkey", hotkey);
		await store.save();
	},

	async updateQuickAskHoldHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_hold_hotkey", hotkey);
		await store.save();
	},

	async updateQuickAskToggleHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_toggle_hotkey", hotkey);
		await store.save();
	},

	/**
	 * Legacy alias (pre split): Quick Ask hotkey (hold-to-record).
	 *
	 * Writes both keys for backward compatibility.
	 */
	async updateQuickAskHotkey(hotkey: HotkeyConfig | null): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_hotkey", hotkey);
		await store.set("quick_ask_hold_hotkey", hotkey);
		await store.save();
	},

	async updateQuickAskProvider(provider: string | null): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_provider", provider);
		await store.save();
	},

	async updateQuickAskModel(model: string | null): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_model", model);
		await store.save();
	},

	async updateQuickAskSystemPrompt(prompt: string | null): Promise<void> {
		const store = await getStore();
		const normalized = typeof prompt === "string" ? prompt.trim() : "";
		await store.set(
			"quick_ask_system_prompt",
			normalized.length > 0 ? normalized : null,
		);
		await store.save();
	},

	async updateQuickAskIncludeSelectedText(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_include_selected_text", Boolean(enabled));
		await store.save();
	},

	async updateQuickAskConversationHistoryEnabled(
		enabled: boolean,
	): Promise<void> {
		const store = await getStore();
		await store.set("quick_ask_conversation_history_enabled", Boolean(enabled));
		await store.save();
	},

	async updateQuickAskConversationHistoryCount(count: number): Promise<void> {
		const store = await getStore();
		const normalized = normalizeQuickAskConversationHistoryCount(count);
		await store.set("quick_ask_conversation_history_count", normalized);
		await store.save();
	},

	async updateQuickAskOpenAiReasoningEffort(
		effort: OpenAiReasoningEffort | null,
	): Promise<void> {
		const store = await getStore();
		if (effort == null) {
			await store.delete("quick_ask_openai_reasoning_effort");
		} else {
			await store.set(
				"quick_ask_openai_reasoning_effort",
				normalizeOpenAiReasoningEffort(effort),
			);
		}
		await store.save();
	},

	async updateQuickAskAnthropicThinkingBudget(
		budget: number | null,
	): Promise<void> {
		const store = await getStore();
		if (budget == null) {
			await store.delete("quick_ask_anthropic_thinking_budget");
		} else {
			await store.set(
				"quick_ask_anthropic_thinking_budget",
				normalizeAnthropicThinkingBudget(budget),
			);
		}
		await store.save();
	},

	async updateQuickAskGeminiThinkingBudget(
		budget: number | null,
	): Promise<void> {
		const store = await getStore();
		if (budget == null) {
			await store.delete("quick_ask_gemini_thinking_budget");
		} else {
			await store.set(
				"quick_ask_gemini_thinking_budget",
				normalizeGeminiThinkingBudget(budget),
			);
		}
		await store.save();
	},

	async updateQuickAskGeminiThinkingLevel(
		level: "minimal" | "low" | "medium" | "high" | null,
	): Promise<void> {
		const store = await getStore();
		if (level == null) {
			await store.delete("quick_ask_gemini_thinking_level");
		} else {
			await store.set(
				"quick_ask_gemini_thinking_level",
				normalizeGeminiThinkingLevel(level),
			);
		}
		await store.save();
	},

	async updateSelectedMic(micId: string | null): Promise<void> {
		const store = await getStore();
		await store.set("selected_mic_id", micId);
		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {});
	},

	async updateSoundEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("sound_enabled", enabled);
		await store.save();
	},

	async updateHotkeyDebugEnabled(enabled: boolean): Promise<void> {
		// Update backend runtime flag immediately so debug events can start flowing
		// without waiting for store writes / reloads.
		await invoke("set_hotkey_debug_enabled_runtime", { enabled: !!enabled });

		const store = await getStore();
		await store.set("hotkey_debug_enabled", !!enabled);
		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		// Without this, a secondary window with a stale Store instance can later
		// save another setting and inadvertently clobber this flag back to the
		// default value.
		await emitTyped("settings-changed", { hotkey_debug_enabled: !!enabled });
	},

	async updateAudioCue(cue: AudioCue): Promise<void> {
		const store = await getStore();
		await store.set("audio_cue", normalizeAudioCue(cue));
		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {});
	},

	async updateRewriteLlmEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("rewrite_llm_enabled", enabled);
		await store.save();
	},

	async updateCleanupPromptSections(
		sections: CleanupPromptSections | null,
	): Promise<void> {
		const store = await getStore();
		await store.set("cleanup_prompt_sections", sections);
		await store.save();
	},

	async updateRewriteProgramPromptProfiles(
		profiles: RewriteProgramPromptProfile[],
	): Promise<void> {
		const store = await getStore();

		// Normalize a couple of legacy/nullable shapes before writing so the backend
		// can deserialize reliably.
		const sanitized = profiles.map((profile) => {
			const presets = (profile.presets ?? []).map((preset) => ({
				...preset,
				routing_hints: preset.routing_hints ?? [],
			}));

			return {
				...profile,
				presets,
			};
		});

		await store.set("rewrite_program_prompt_profiles", sanitized);
		await store.save();

		// Notify other windows (overlay/hover) to refresh cached settings.
		await emitTyped("settings-changed", {});
	},

	async updateSTTProvider(provider: string | null): Promise<void> {
		const store = await getStore();
		await store.set("stt_provider", provider);
		await store.save();
	},

	async updateCerebrasFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("cerebras_free_tier", !!enabled);
		await store.save();
	},

	async updateGroqFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("groq_free_tier", !!enabled);
		await store.save();
	},

	async updateElevenLabsFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("elevenlabs_free_tier", !!enabled);
		await store.save();
	},

	async updateCohereFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("cohere_free_tier", !!enabled);
		await store.save();
	},

	async updateAssemblyAiFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("assemblyai_free_tier", !!enabled);
		await store.save();
	},

	async updateSpeechmaticsFreeTier(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("speechmatics_free_tier", !!enabled);
		await store.save();
	},

	async updateSTTModel(model: string | null): Promise<void> {
		const store = await getStore();
		await store.set("stt_model", model);
		await store.save();
	},

	async updateSTTTranscriptionPrompt(prompt: string | null): Promise<void> {
		const store = await getStore();
		await store.set("stt_transcription_prompt", prompt);
		await store.save();
	},

	async updateWhisperServerBaseUrl(baseUrl: string | null): Promise<void> {
		const store = await getStore();
		const normalized = baseUrl?.trim() ? baseUrl.trim() : null;
		await store.set("whisper_server_base_url", normalized);
		await store.save();
	},

	async updateOllamaUrl(baseUrl: string | null): Promise<void> {
		const store = await getStore();
		const normalized = baseUrl?.trim() ? baseUrl.trim() : null;
		await store.set("ollama_url", normalized);
		await store.save();
	},

	async updateLocalWhisperModelId(modelId: string | null): Promise<void> {
		const store = await getStore();
		const normalized = modelId?.trim() ? modelId.trim().toLowerCase() : null;
		await store.set("local_whisper_model_id", normalized);
		await store.save();
	},

	async updateLocalWhisperLoadMode(mode: LocalWhisperLoadMode): Promise<void> {
		const store = await getStore();
		await store.set(
			"local_whisper_load_mode",
			normalizeLocalWhisperLoadMode(mode),
		);
		await store.save();
	},

	async updateProxySettings(proxySettings: ProxySettings): Promise<void> {
		const store = await getStore();
		await store.set("proxy_settings", normalizeProxySettings(proxySettings));
		await store.save();
	},

	async updateLLMProvider(provider: string | null): Promise<void> {
		const store = await getStore();
		await store.set("llm_provider", provider);
		await store.save();
	},

	async updateLLMModel(model: string | null): Promise<void> {
		const store = await getStore();
		await store.set("llm_model", model);
		await store.save();
	},

	async updateOpenAiReasoningEffort(
		effort: OpenAiReasoningEffort | null,
	): Promise<void> {
		const store = await getStore();
		if (effort == null) {
			await store.delete("openai_reasoning_effort");
		} else {
			await store.set(
				"openai_reasoning_effort",
				normalizeOpenAiReasoningEffort(effort),
			);
		}
		await store.save();
	},

	async updateAnthropicThinkingBudget(budget: number | null): Promise<void> {
		const store = await getStore();
		if (budget == null) {
			await store.delete("anthropic_thinking_budget");
		} else {
			await store.set(
				"anthropic_thinking_budget",
				normalizeAnthropicThinkingBudget(budget),
			);
		}
		await store.save();
	},

	async updateGeminiThinkingBudget(budget: number | null): Promise<void> {
		const store = await getStore();
		if (budget == null) {
			await store.delete("gemini_thinking_budget");
		} else {
			await store.set(
				"gemini_thinking_budget",
				normalizeGeminiThinkingBudget(budget),
			);
		}
		await store.save();
	},

	async updateGeminiThinkingLevel(
		level: "minimal" | "low" | "medium" | "high" | null,
	): Promise<void> {
		const store = await getStore();
		if (level == null) {
			await store.delete("gemini_thinking_level");
		} else {
			await store.set(
				"gemini_thinking_level",
				normalizeGeminiThinkingLevel(level),
			);
		}
		await store.save();
	},

	async updatePlayingAudioHandling(
		handling: PlayingAudioHandling,
	): Promise<void> {
		const store = await getStore();
		await store.set("playing_audio_handling", handling);
		await store.save();
	},

	async updateSTTTimeout(timeoutSeconds: number | null): Promise<void> {
		const store = await getStore();
		await store.set("stt_timeout_seconds", timeoutSeconds);
		await store.save();
	},

	async updateOverlayMode(mode: OverlayMode): Promise<void> {
		const store = await getStore();
		await store.set("overlay_mode", mode);
		await store.save();
		// Apply the mode immediately
		await invoke("set_overlay_mode", { mode });

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {});
	},

	async updateOverlayShowDetailedLoading(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("overlay_show_detailed_loading", !!enabled);
		await store.save();

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {
			overlay_show_detailed_loading: !!enabled,
		});
	},

	async updateOverlayMonitorTarget(
		target: OverlayMonitorTarget,
	): Promise<void> {
		const store = await getStore();
		const normalized = normalizeOverlayMonitorTarget(target);

		await store.set("overlay_monitor_target", normalized);
		await store.save();

		// Best-effort: immediately re-snap overlay windows to the selected monitor.
		// This uses the user's saved widget_position.
		try {
			const raw = await store.get("widget_position");
			const position =
				raw === "center" ||
				raw === "top-left" ||
				raw === "top-center" ||
				raw === "top-right" ||
				raw === "bottom-left" ||
				raw === "bottom-center" ||
				raw === "bottom-right"
					? (raw as WidgetPosition)
					: ("bottom-center" as WidgetPosition);
			await invoke("set_widget_position", { position });
		} catch {
			// ignore
		}

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", { overlay_monitor_target: normalized });
	},

	async updateWidgetPosition(position: WidgetPosition): Promise<void> {
		const store = await getStore();
		await store.set("widget_position", position);
		await store.save();
		// Apply the position immediately
		await invoke("set_widget_position", { position });

		// Notify other windows (overlay) to refresh cached settings.
		await emitTyped("settings-changed", {});
	},

	async updateOutputMode(mode: OutputMode): Promise<void> {
		const store = await getStore();
		await store.set("output_mode", mode);
		await store.save();
	},

	async updateOutputHitEnter(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("output_hit_enter", enabled);
		await store.save();
	},

	async updateQuietAudioGateEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("quiet_audio_gate_enabled", enabled);
		await store.save();
	},

	async updateQuietAudioMinDurationSecs(seconds: number): Promise<void> {
		const store = await getStore();
		await store.set("quiet_audio_min_duration_secs", seconds);
		await store.save();
	},

	async updateQuietAudioRmsDbfsThreshold(dbfs: number): Promise<void> {
		const store = await getStore();
		await store.set("quiet_audio_rms_dbfs_threshold", dbfs);
		await store.save();
	},

	async updateQuietAudioPeakDbfsThreshold(dbfs: number): Promise<void> {
		const store = await getStore();
		await store.set("quiet_audio_peak_dbfs_threshold", dbfs);
		await store.save();
	},

	async updateQuietAudioRequireSpeech(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("quiet_audio_require_speech", enabled);
		await store.save();
	},

	async updateHotMicEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("hot_mic_enabled", !!enabled);
		await store.save();
	},

	async updateHotMicPreRollMs(ms: number): Promise<void> {
		const store = await getStore();
		const normalized = Number.isFinite(ms) ? Math.max(0, Math.round(ms)) : 0;
		await store.set("hot_mic_pre_roll_ms", normalized);
		await store.save();
	},

	async updateMicAutoRecoverEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("mic_auto_recover_enabled", !!enabled);
		await store.save();
	},

	async updateNoiseGateThresholdDbfs(
		thresholdDbfs: number | null,
	): Promise<void> {
		const store = await getStore();
		const normalized = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
		await store.set("noise_gate_threshold_dbfs", normalized);
		// Best-effort legacy key for downgrade compatibility.
		await store.set(
			"noise_gate_strength",
			noiseGateThresholdDbfsToStrength(normalized),
		);
		await store.save();
	},

	async updateNoiseGateStrength(strength: number): Promise<void> {
		const store = await getStore();
		const normalizedStrength = normalizeNoiseGateStrength(strength);
		await store.set("noise_gate_strength", normalizedStrength);
		// Keep the new key in sync for newer builds.
		await store.set(
			"noise_gate_threshold_dbfs",
			noiseGateStrengthToThresholdDbfs(normalizedStrength),
		);
		await store.save();
	},

	async updateAudioDownmixToMono(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("audio_downmix_to_mono", enabled);
		await store.save();
	},

	async updateAudioResampleTo16khz(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("audio_resample_to_16khz", enabled);
		await store.save();
	},

	async updateAudioHighpassEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("audio_highpass_enabled", enabled);
		await store.save();
	},

	async updateAudioAgcEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("audio_agc_enabled", enabled);
		await store.save();
	},

	async updateAudioNoiseSuppressionEnabled(enabled: boolean): Promise<void> {
		const store = await getStore();
		await store.set("audio_noise_suppression_enabled", enabled);
		await store.save();
	},

	async updateMaxSavedRecordings(max: number): Promise<void> {
		const store = await getStore();
		await store.set("max_saved_recordings", normalizeMaxSavedRecordings(max));
		await store.save();
	},

	async updateRequestLogsRetention(params: {
		mode: AppSettings["request_logs_retention_mode"];
		amount: number;
		days: number;
	}): Promise<void> {
		const store = await getStore();

		const mode = normalizeRequestLogsRetentionMode(params.mode);
		const amount = normalizeRequestLogsRetentionAmount(params.amount);
		const days = normalizeRequestLogsRetentionDays(params.days);

		await store.set("request_logs_retention_mode", mode);
		await store.set("request_logs_retention_amount", amount);
		await store.set("request_logs_retention_days", days);
		await store.save();
	},

	async updateTranscriptionRetentionDays(days: number): Promise<void> {
		const store = await getStore();
		const normalized = normalizeTranscriptionRetentionValue(days, "days");
		// Legacy key (kept for backward compatibility)
		await store.set("transcription_retention_days", normalized);
		// New keys
		await store.set("transcription_retention_unit", "days");
		await store.set("transcription_retention_value", normalized);
		await store.save();
	},

	async updateTranscriptionRetention(params: {
		unit: TranscriptionRetentionUnit;
		value: number;
	}): Promise<void> {
		const store = await getStore();
		const unit = normalizeTranscriptionRetentionUnit(params.unit);
		const value = normalizeTranscriptionRetentionValue(params.value, unit);

		await store.set("transcription_retention_unit", unit);
		await store.set("transcription_retention_value", value);

		// Best-effort: keep the legacy days key in sync when unit is days.
		// (If unit is hours, we leave the legacy key untouched to avoid silently
		// changing semantics for older builds.)
		if (unit === "days") {
			await store.set("transcription_retention_days", value);
		}

		await store.save();
	},

	async updateTranscriptionRetentionDeleteRecordings(
		enabled: boolean,
	): Promise<void> {
		const store = await getStore();
		await store.set(
			"transcription_retention_delete_recordings",
			normalizeTranscriptionRetentionDeleteRecordings(enabled),
		);
		await store.save();
	},

	async updateStatsRetention(params: {
		unit: TranscriptionRetentionUnit;
		value: number;
		max_bytes?: number;
	}): Promise<void> {
		const store = await getStore();
		const unit = normalizeTranscriptionRetentionUnit(params.unit);
		const value = normalizeTranscriptionRetentionValue(params.value, unit);

		await store.set("stats_retention_unit", unit);
		await store.set("stats_retention_value", value);

		if (typeof params.max_bytes === "number") {
			await store.set(
				"stats_retention_max_bytes",
				normalizeStatsRetentionMaxBytes(params.max_bytes),
			);
		}

		await store.save();
	},

	async getSettingsGuideState(): Promise<SettingsGuideState> {
		const store = await getStore();
		const raw = await store.get(SETTINGS_GUIDE_STATE_KEY);
		const state = normalizeSettingsGuideState(raw);

		try {
			if (typeof window !== "undefined" && window.localStorage) {
				window.localStorage.setItem("tv_settings_guide_state", state);
			}
		} catch {
			// ignore
		}

		return state;
	},

	async setSettingsGuideState(state: SettingsGuideState): Promise<void> {
		const store = await getStore();
		await store.set(
			SETTINGS_GUIDE_STATE_KEY,
			normalizeSettingsGuideState(state),
		);
		await store.save();

		try {
			if (typeof window !== "undefined" && window.localStorage) {
				window.localStorage.setItem("tv_settings_guide_state", state);
			}
		} catch {
			// ignore
		}

		// Notify other windows that persisted state changed.
		await emitTyped("settings-changed", { [SETTINGS_GUIDE_STATE_KEY]: state });
	},

	async resetHotkeysToDefaults(): Promise<void> {
		const store = await getStore();
		await store.set("toggle_hotkey", DEFAULT_TOGGLE_HOTKEY);
		await store.set("hold_hotkey", DEFAULT_HOLD_HOTKEY);
		await store.set("paste_last_hotkey", DEFAULT_PASTE_LAST_HOTKEY);
		await store.set("retry_hotkey", DEFAULT_RETRY_HOTKEY);
		await store.set("quick_ask_hold_hotkey", DEFAULT_QUICK_ASK_HOLD_HOTKEY);
		await store.set("quick_ask_toggle_hotkey", DEFAULT_QUICK_ASK_TOGGLE_HOTKEY);
		// Legacy alias (pre split): keep in sync.
		await store.set("quick_ask_hotkey", DEFAULT_QUICK_ASK_HOLD_HOTKEY);
		await store.save();
	},
};
