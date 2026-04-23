import { invoke } from "@tauri-apps/api/core";
import { Store } from "@tauri-apps/plugin-store";
import { DEFAULT_ACCENT_HEX, normalizeHexColor } from "../accentColor";
import { evaluateTokenExchangeDecision } from "../auth/tokenExchangeGate";
import {
	DEFAULT_HOLD_HOTKEY,
	DEFAULT_PASTE_LAST_HOTKEY,
	DEFAULT_QUICK_ASK_HOLD_HOTKEY,
	DEFAULT_QUICK_ASK_TOGGLE_HOTKEY,
	DEFAULT_RETRY_HOTKEY,
	DEFAULT_TOGGLE_HOTKEY,
} from "../hotkeyDefaults";
import {
	createHotkeyShortcutId,
	type HotkeyConfig,
	type HotkeyShortcutCard,
	HotkeyShortcutCardsSchema,
	type HotkeyType,
	normalizeHotkeyConfig,
} from "../hotkeys";
import {
	DEFAULT_STT_LANGUAGE,
	normalizeSttLanguage,
	normalizeSttLanguageOverride,
} from "../sttLanguages";
import { emitTyped } from "./events";
import type {
  ActiveWindowOcrMode,
  AppSettings,
  AudioCue,
  CleanupPromptSections,
  CleanupPromptSectionsOverride,
  ContextGrabMethod,
  IntentRouterSettings,
  LicenseState,
  LocalWhisperLoadMode,
  MainWindowCloseBehavior,
  OcrAuthMode,
  OcrAutoCaptureTiming,
  OcrResizeFilter,
  OpenAiReasoningEffort,
  OutputMode,
  OverlayMode,
  OverlayMonitorTarget,
  PlayingAudioHandling,
  PolicyState,
  ProxySettings,
  QuickAskDismissMode,
  RewritePreset,
  RewriteProgramPromptProfile,
  SettingsGuideState,
  TokenExchangeTriggerSet,
  TranscriptionRetentionUnit,
  WidgetPosition,
} from "./types";

const isRecord = (value: unknown): value is Record<string, unknown> => {
	return value != null && typeof value === "object" && !Array.isArray(value);
};

function normalizePolicySource(value: unknown): PolicyState["source"] {
	if (
		value === "none" ||
		value === "file" ||
		value === "cloud" ||
		value === "cached" ||
		value === "degraded_expired"
	)
		return value;
	return "none";
}

function normalizePolicyTimestamp(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

function normalizePolicyState(value: unknown): PolicyState {
	const v = isRecord(value) ? value : {};
	const source = normalizePolicySource(v.source);
	const eligible = typeof v.eligible === "boolean" ? v.eligible : false;
	const active_policy_id =
		typeof v.active_policy_id === "string" ? v.active_policy_id : null;
	const active_version =
		typeof v.active_version === "number" && Number.isFinite(v.active_version)
			? Math.max(0, Math.trunc(v.active_version))
			: null;
	const last_sync_at = normalizePolicyTimestamp(v.last_sync_at);
	const last_success_at = normalizePolicyTimestamp(v.last_success_at);
	const last_updated = normalizePolicyTimestamp(v.last_updated);
	const expires_at = normalizePolicyTimestamp(v.expires_at);
	const failure_reason =
		typeof v.failure_reason === "string" ? v.failure_reason : null;
	const enforced_count =
		typeof v.enforced_count === "number" && Number.isFinite(v.enforced_count)
			? Math.max(0, Math.trunc(v.enforced_count))
			: null;
	const version =
		typeof v.version === "string"
			? v.version
			: typeof v.version === "number" && Number.isFinite(v.version)
				? String(Math.trunc(v.version))
				: null;

	const now = Date.now();
	const expiresAtMs = expires_at == null ? null : Date.parse(expires_at);
	const expired =
		expiresAtMs != null && Number.isFinite(expiresAtMs) && expiresAtMs < now;

	const baseValid = typeof v.is_valid === "boolean" ? v.is_valid : true;
	const is_valid = source === "none" ? true : baseValid && !expired;

	const enforced_fields: PolicyState["enforced_fields"] = Array.isArray(
		v.enforced_fields,
	)
		? v.enforced_fields
				.map((field): PolicyState["enforced_fields"][number] | null => {
					if (!isRecord(field)) return null;
					const path = typeof field.path === "string" ? field.path.trim() : "";
					if (!path) return null;
					const reason = typeof field.reason === "string" ? field.reason : null;
					return { path, reason };
				})
				.filter(
					(field): field is PolicyState["enforced_fields"][number] =>
						field !== null,
				)
		: [];

	return {
		source,
		eligible,
		is_valid,
		active_policy_id,
		active_version,
		last_sync_at,
		last_success_at,
		last_updated,
		expires_at,
		failure_reason,
		enforced_count: enforced_count ?? enforced_fields.length,
		version,
		enforced_fields,
	};
}

function normalizeLicenseTier(value: unknown): LicenseState["tier"] {
	if (value === "enterprise" || value === "personal" || value === "community") {
		return value;
	}
	return "community";
}

function normalizeLicenseStatus(value: unknown): LicenseState["status"] {
	if (
		value === "signed_out" ||
		value === "active" ||
		value === "grace" ||
		value === "expired"
	) {
		return value;
	}
	return "signed_out";
}

function normalizeLicenseTimestamp(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

function normalizeLicenseState(value: unknown): LicenseState {
	const nowIso = new Date().toISOString();
	const v = isRecord(value) ? value : {};

	const tier = normalizeLicenseTier(v.tier);
	const status = normalizeLicenseStatus(v.status);
	const user_id = typeof v.user_id === "string" ? v.user_id : null;
	const email = typeof v.email === "string" ? v.email : null;

	const org = isRecord(v.org)
		? (() => {
				const org_id =
					typeof v.org.org_id === "string" ? v.org.org_id.trim() : "";
				const org_name =
					typeof v.org.org_name === "string" ? v.org.org_name.trim() : "";
				if (!org_id || !org_name) return null;
				return { org_id, org_name };
			})()
		: null;

	const usage = isRecord(v.usage)
		? {
				stt_seconds_used:
					typeof v.usage.stt_seconds_used === "number" &&
					Number.isFinite(v.usage.stt_seconds_used)
						? Math.max(0, Math.trunc(v.usage.stt_seconds_used))
						: 0,
				llm_tokens_used:
					typeof v.usage.llm_tokens_used === "number" &&
					Number.isFinite(v.usage.llm_tokens_used)
						? Math.max(0, Math.trunc(v.usage.llm_tokens_used))
						: 0,
				requests_today:
					typeof v.usage.requests_today === "number" &&
					Number.isFinite(v.usage.requests_today)
						? Math.max(0, Math.trunc(v.usage.requests_today))
						: 0,
			}
		: {
				stt_seconds_used: 0,
				llm_tokens_used: 0,
				requests_today: 0,
			};

	const limits = isRecord(v.limits)
		? {
				stt_seconds_monthly:
					typeof v.limits.stt_seconds_monthly === "number" &&
					Number.isFinite(v.limits.stt_seconds_monthly)
						? Math.max(0, Math.trunc(v.limits.stt_seconds_monthly))
						: 0,
				llm_tokens_monthly:
					typeof v.limits.llm_tokens_monthly === "number" &&
					Number.isFinite(v.limits.llm_tokens_monthly)
						? Math.max(0, Math.trunc(v.limits.llm_tokens_monthly))
						: 0,
				requests_per_day:
					typeof v.limits.requests_per_day === "number" &&
					Number.isFinite(v.limits.requests_per_day)
						? Math.max(0, Math.trunc(v.limits.requests_per_day))
						: 0,
			}
		: {
				stt_seconds_monthly: 0,
				llm_tokens_monthly: 0,
				requests_per_day: 0,
			};

	return {
		tier,
		status,
		user_id,
		email,
		org,
		expires_at: normalizeLicenseTimestamp(v.expires_at),
		cached_at: normalizeLicenseTimestamp(v.cached_at) ?? nowIso,
		last_validated_at: normalizeLicenseTimestamp(v.last_validated_at),
		usage,
		limits,
	};
}

function normalizeTokenExchangeReviewedAt(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Date.parse(trimmed);
  if (Number.isNaN(parsed)) return null;
  return new Date(parsed).toISOString();
}

function normalizeTokenExchangeTriggerSet(
  value: unknown,
): TokenExchangeTriggerSet {
  const v = isRecord(value) ? value : {};
  const multi_idp_required = Boolean(v.multi_idp_required);
  const kill_switch_required = Boolean(v.kill_switch_required);
  const embedded_claims_required = Boolean(v.embedded_claims_required);
  const desktop_idp_agnostic_required = Boolean(
    v.desktop_idp_agnostic_required,
  );
  const reviewed_at = normalizeTokenExchangeReviewedAt(v.reviewed_at);

  return {
    multi_idp_required,
    kill_switch_required,
    embedded_claims_required,
    desktop_idp_agnostic_required,
    reviewed_at,
    decision: evaluateTokenExchangeDecision({
      multi_idp_required,
      kill_switch_required,
      embedded_claims_required,
      desktop_idp_agnostic_required,
    }),
  };
}

export function resolveManagedInferenceMode(state: {
	license_state?: Pick<LicenseState, "tier" | "status"> | null;
	policy_state?: Pick<PolicyState, "source" | "eligible" | "is_valid"> | null;
}): "managed" | "byok" {
	const tier = state.license_state?.tier ?? "community";
	const status = state.license_state?.status ?? "signed_out";

	if (tier === "personal") {
		return status === "active" || status === "grace" ? "managed" : "byok";
	}

	if (tier === "enterprise") {
		const policy = state.policy_state;
		if (!policy) return "byok";
		if (!policy.is_valid) return "byok";
		if (policy.source === "none") return "byok";
		return policy.eligible ? "managed" : "byok";
	}

	return "byok";
}

// ============================================================================
// OCR normalization helpers
// ============================================================================

function normalizeActiveWindowOcrMode(value: unknown): ActiveWindowOcrMode {
	if (value === "off" || value === "auto" || value === "manual") return value;
	return "off";
}

function normalizeOcrAuthMode(value: unknown): OcrAuthMode {
	if (value === "none" || value === "bearer_api_key") return value;
	return "none";
}

function normalizeOcrAutoCaptureTiming(value: unknown): OcrAutoCaptureTiming {
	if (value === "on_stop" || value === "on_start") return value;
	return "on_start";
}

function normalizeOcrResizeFilter(value: unknown): OcrResizeFilter {
	if (
		value === "nearest" ||
		value === "triangle" ||
		value === "catmullrom" ||
		value === "lanczos3"
	)
		return value;
	return "nearest";
}

// ============================================================================

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
	const stt_language = normalizeSttLanguageOverride(p.stt_language);
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
		stt_language,
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

function normalizeQuickAskDismissMode(value: unknown): QuickAskDismissMode {
	if (value === "manual" || value === "auto") return value;
	return "manual";
}

function normalizeQuickAskDismissModeOverride(
	value: unknown,
): QuickAskDismissMode | null {
	if (value === "manual" || value === "auto") return value;
	return null;
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

function normalizeRetentionMode(
	value: unknown,
	fallback: "amount" | "time" = "amount",
): "amount" | "time" {
	return value === "time" || value === "amount" ? value : fallback;
}

function normalizeTranscriptionRetentionAmount(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 1000;
	const rounded = Math.round(value);
	// 1..100000 (defensive)
	return Math.min(100000, Math.max(1, rounded));
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
const SETTINGS_VERSION_LATEST = 8;
// Legacy fixtures/settings files may predate `settings_version` being written.
// For UI normalization and tests, treat a missing/invalid version as the last
// pre-versioning schema we can reasonably assume.
const SETTINGS_VERSION_ASSUME_IF_MISSING = 3;

const HOTKEY_TYPES: HotkeyType[] = [
	"toggle",
	"hold",
	"paste_last",
	"retry",
	"quick_ask_hold",
	"quick_ask_toggle",
];

type LegacyHotkeySettings = {
	toggle_hotkey: HotkeyConfig | null;
	hold_hotkey: HotkeyConfig | null;
	paste_last_hotkey: HotkeyConfig | null;
	retry_hotkey: HotkeyConfig | null;
	quick_ask_hold_hotkey: HotkeyConfig | null;
	quick_ask_toggle_hotkey: HotkeyConfig | null;
};

function normalizeHotkeyShortcutCards(
	value: unknown,
): HotkeyShortcutCard[] | null {
	const result = HotkeyShortcutCardsSchema.safeParse(value);
	return result.success ? result.data : null;
}

function buildShortcutCardsFromLegacy(
	legacy: LegacyHotkeySettings,
): HotkeyShortcutCard[] {
	const byType: Record<HotkeyType, HotkeyConfig | null> = {
		toggle: legacy.toggle_hotkey,
		hold: legacy.hold_hotkey,
		paste_last: legacy.paste_last_hotkey,
		retry: legacy.retry_hotkey,
		quick_ask_hold: legacy.quick_ask_hold_hotkey,
		quick_ask_toggle: legacy.quick_ask_toggle_hotkey,
	};

	return HOTKEY_TYPES.flatMap((type) => {
		const hotkey = byType[type];
		if (!hotkey) return [];
		return [
			{
				id: createHotkeyShortcutId(),
				type,
				hotkey,
			},
		];
	});
}

function getFirstHotkeyByType(
	cards: HotkeyShortcutCard[],
	type: HotkeyType,
): HotkeyConfig | null {
	for (const card of cards) {
		if (card.type !== type) continue;
		if (card.hotkey) return card.hotkey;
	}

	return null;
}

function normalizeSettingsGuideState(value: unknown): SettingsGuideState {
	if (value === "pending" || value === "skipped" || value === "completed") {
		return value;
	}
	return "pending";
}

function normalizeSettingsVersion(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return SETTINGS_VERSION_ASSUME_IF_MISSING;
	}

	const normalized = Math.trunc(value);
	if (normalized < 1) {
		return SETTINGS_VERSION_ASSUME_IF_MISSING;
	}
	if (normalized > SETTINGS_VERSION_LATEST) {
		return SETTINGS_VERSION_LATEST;
	}
	return normalized;
}

async function getStore(): Promise<Store> {
	if (!storeInstance) {
		storeInstance = await Store.load("settings.json");
	}
	return storeInstance;
}

async function reloadSettingsStoreFromDisk(): Promise<void> {
	// @tauri-apps/plugin-store doesn't expose an instance reload API.
	// Recreate the Store instance so future reads come from disk.
	storeInstance = await Store.load("settings.json");
}

type PolicyConstraintViolation = {
	path: string;
	reason: string | null;
};

export type PolicyPathEnforcement = {
	path: string;
	enforced: boolean;
	reason: string | null;
};

const POLICY_PATH_ALIASES: Readonly<Record<string, string[]>> = {
	quick_ask_hotkey: ["quick_ask_hold_hotkey"],
	quick_ask_hold_hotkey: ["quick_ask_hotkey"],
	transcription_retention_days: [
		"transcription_retention_value",
		"transcription_retention_unit",
	],
	transcription_retention_value: ["transcription_retention_days"],
};

function clonePatchRecord(
	patch: Record<string, unknown> | undefined,
): Record<string, unknown> {
	if (!patch) return {};
	return { ...patch };
}

function canonicalPolicyPath(path: string): string {
	return path.trim();
}

export function getPolicyPathEnforcement(
	policy: PolicyState | null | undefined,
	path: string,
): PolicyPathEnforcement {
	const normalizedPath = canonicalPolicyPath(path);
	if (!normalizedPath || !policy) {
		return {
			path: normalizedPath,
			enforced: false,
			reason: null,
		};
	}

	if (policy.source === "none" || !policy.is_valid) {
		return {
			path: normalizedPath,
			enforced: false,
			reason: null,
		};
	}

	const aliases = new Set<string>([
		normalizedPath,
		...(POLICY_PATH_ALIASES[normalizedPath] ?? []),
	]);
	for (const [canonical, linked] of Object.entries(POLICY_PATH_ALIASES)) {
		if (linked.includes(normalizedPath)) {
			aliases.add(canonical);
			for (const alias of linked) aliases.add(alias);
		}
	}

	for (const field of policy.enforced_fields) {
		const fieldPath = canonicalPolicyPath(field.path);
		if (!fieldPath) continue;
		if (aliases.has(fieldPath)) {
			return {
				path: normalizedPath,
				enforced: true,
				reason: field.reason ?? null,
			};
		}
	}

	return {
		path: normalizedPath,
		enforced: false,
		reason: null,
	};
}

function patchTouchesPolicyPath(
	patch: Record<string, unknown>,
	deleteKeys: string[],
	path: string,
): boolean {
	if (Object.hasOwn(patch, path)) return true;
	if (deleteKeys.includes(path)) return true;

	const aliases = POLICY_PATH_ALIASES[path] ?? [];
	for (const alias of aliases) {
		if (Object.hasOwn(patch, alias)) return true;
		if (deleteKeys.includes(alias)) return true;
	}

	return false;
}

function removePolicyPathFromPatch(
	patch: Record<string, unknown>,
	deleteKeys: string[],
	path: string,
): void {
	delete patch[path];
	for (const alias of POLICY_PATH_ALIASES[path] ?? []) {
		delete patch[alias];
	}

	const blocked = new Set([path, ...(POLICY_PATH_ALIASES[path] ?? [])]);
	for (let i = deleteKeys.length - 1; i >= 0; i -= 1) {
		const key = deleteKeys[i];
		if (typeof key !== "string") continue;
		if (blocked.has(key)) {
			deleteKeys.splice(i, 1);
		}
	}
}

function didPolicyStateChangeRaw(
	raw: unknown,
	normalized: PolicyState,
): boolean {
	if (raw == null) return false;

	const normalizedComparable = JSON.stringify(normalized);
	if (!isRecord(raw)) return true;

	const source = normalizePolicySource(raw.source);
	const eligible = typeof raw.eligible === "boolean" ? raw.eligible : false;
	const active_policy_id =
		typeof raw.active_policy_id === "string" ? raw.active_policy_id : null;
	const active_version =
		typeof raw.active_version === "number" &&
		Number.isFinite(raw.active_version)
			? Math.max(0, Math.trunc(raw.active_version))
			: null;
	const last_sync_at = normalizePolicyTimestamp(raw.last_sync_at);
	const last_success_at = normalizePolicyTimestamp(raw.last_success_at);
	const last_updated = normalizePolicyTimestamp(raw.last_updated);
	const expires_at = normalizePolicyTimestamp(raw.expires_at);
	const failure_reason =
		typeof raw.failure_reason === "string" ? raw.failure_reason : null;
	const enforced_count =
		typeof raw.enforced_count === "number" &&
		Number.isFinite(raw.enforced_count)
			? Math.max(0, Math.trunc(raw.enforced_count))
			: null;
	const version = typeof raw.version === "string" ? raw.version : null;
	const is_valid = typeof raw.is_valid === "boolean" ? raw.is_valid : true;
	const enforced_fields: PolicyState["enforced_fields"] = Array.isArray(
		raw.enforced_fields,
	)
		? raw.enforced_fields
				.map((field): PolicyState["enforced_fields"][number] | null => {
					if (!isRecord(field)) return null;
					const path = typeof field.path === "string" ? field.path.trim() : "";
					if (!path) return null;
					const reason = typeof field.reason === "string" ? field.reason : null;
					return { path, reason };
				})
				.filter(
					(field): field is PolicyState["enforced_fields"][number] =>
						field !== null,
				)
		: [];

	const rawComparable = JSON.stringify({
		source,
		eligible,
		is_valid,
		active_policy_id,
		active_version,
		last_sync_at,
		last_success_at,
		last_updated,
		expires_at,
		failure_reason,
		enforced_count: enforced_count ?? enforced_fields.length,
		version,
		enforced_fields,
	});
	return rawComparable !== normalizedComparable;
}

async function preparePolicyAwarePatch(params: {
	patch?: Record<string, unknown>;
	deleteKeys?: string[];
}): Promise<{
	patch: Record<string, unknown>;
	deleteKeys: string[];
	violations: PolicyConstraintViolation[];
	policyNormalized: boolean;
}> {
	const store = await getStore();
	const rawPolicyState = await store.get("policy_state");
	const normalizedPolicyState = normalizePolicyState(rawPolicyState);

	const patch = clonePatchRecord(params.patch);
	const deleteKeys = [...(params.deleteKeys ?? [])];

	const policyNormalized = didPolicyStateChangeRaw(
		rawPolicyState,
		normalizedPolicyState,
	);
	if (policyNormalized) {
		patch.policy_state = normalizedPolicyState;
	}

	const policyIsEnforcing =
		normalizedPolicyState.source !== "none" && normalizedPolicyState.is_valid;
	const violations: PolicyConstraintViolation[] = [];

	if (policyIsEnforcing) {
		for (const field of normalizedPolicyState.enforced_fields) {
			const path = canonicalPolicyPath(field.path);
			if (!path) continue;
			if (!patchTouchesPolicyPath(patch, deleteKeys, path)) continue;

			violations.push({
				path,
				reason: field.reason ?? null,
			});
			removePolicyPathFromPatch(patch, deleteKeys, path);
		}
	}

	return {
		patch,
		deleteKeys,
		violations,
		policyNormalized,
	};
}

async function applySettingsPatch(params: {
	patch?: Record<string, unknown>;
	deleteKeys?: string[];
}): Promise<void> {
	const prepared = await preparePolicyAwarePatch(params);
	const hasPatch = Object.keys(prepared.patch).length > 0;
	const hasDeletes = prepared.deleteKeys.length > 0;
	const modeStateTouched =
		Object.hasOwn(prepared.patch, "policy_state") ||
		Object.hasOwn(prepared.patch, "license_state") ||
		prepared.deleteKeys.includes("policy_state") ||
		prepared.deleteKeys.includes("license_state");

	if (hasPatch || hasDeletes) {
		await invoke("settings_apply_patch", {
			patch: prepared.patch,
			deleteKeys: prepared.deleteKeys,
		});
	}

	if (
		modeStateTouched ||
		prepared.policyNormalized ||
		prepared.violations.length > 0
	) {
		// Keep runtime behavior aligned whenever policy normalization/constraints
		// affect effective settings (including managed mode transitions driven by
		// policy/license state updates).
		await invoke("sync_pipeline_config");
		await emitTyped("settings-changed", {
			managed_mode_updated: modeStateTouched,
			policy_normalized: prepared.policyNormalized,
			policy_constraints_applied: prepared.violations.length > 0,
			policy_violations: prepared.violations,
		});
	}

	await reloadSettingsStoreFromDisk();
}

export const tauriSettingsAPI = {
  async getSettings(): Promise<AppSettings> {
    const store = await getStore();

    const rawSettingsVersion = await store.get(SETTINGS_VERSION_KEY);
    const settingsVersion = normalizeSettingsVersion(rawSettingsVersion);
    // IMPORTANT: This getter should be read-only.
    //
    // Writes are centralized in the backend so multi-window store instances
    // can't accidentally clobber each other.

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

      const disabled = typeof p.disabled === "boolean" ? p.disabled : false;

      const cleanup_prompt_sections = normalizeCleanupPromptSectionsOverride(
        p.cleanup_prompt_sections,
      );
      const stt_provider =
        typeof p.stt_provider === "string" ? p.stt_provider : null;
      const stt_model = typeof p.stt_model === "string" ? p.stt_model : null;
      const stt_language = normalizeSttLanguageOverride(p.stt_language);
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
      const quick_ask_dismiss_mode = normalizeQuickAskDismissModeOverride(
        p.quick_ask_dismiss_mode,
      );

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

      // Per-profile Active Window OCR mode overrides ("off" | "auto" | "manual" | null)
      const rewrite_active_window_ocr_mode: ActiveWindowOcrMode | null =
        p.rewrite_active_window_ocr_mode === "off" ||
        p.rewrite_active_window_ocr_mode === "auto" ||
        p.rewrite_active_window_ocr_mode === "manual"
          ? p.rewrite_active_window_ocr_mode
          : null;
      const quick_replace_active_window_ocr_mode: ActiveWindowOcrMode | null =
        p.quick_replace_active_window_ocr_mode === "off" ||
        p.quick_replace_active_window_ocr_mode === "auto" ||
        p.quick_replace_active_window_ocr_mode === "manual"
          ? p.quick_replace_active_window_ocr_mode
          : null;
      const quick_ask_active_window_ocr_mode: ActiveWindowOcrMode | null =
        p.quick_ask_active_window_ocr_mode === "off" ||
        p.quick_ask_active_window_ocr_mode === "auto" ||
        p.quick_ask_active_window_ocr_mode === "manual"
          ? p.quick_ask_active_window_ocr_mode
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

    const maxSavedRecordings = normalizeMaxSavedRecordings(
      await store.get("max_saved_recordings"),
    );
    const recordingsRetentionMode = normalizeRetentionMode(
      await store.get("recordings_retention_mode"),
      "amount",
    );
    const recordingsRetentionAmount = await (async () => {
      const raw = await store.get("recordings_retention_amount");
      if (raw == null) return maxSavedRecordings;
      return normalizeMaxSavedRecordings(raw);
    })();
    const recordingsRetentionUnit = normalizeTranscriptionRetentionUnit(
      await store.get("recordings_retention_unit"),
    );
    const recordingsRetentionValue = normalizeTranscriptionRetentionValue(
      await store.get("recordings_retention_value"),
      recordingsRetentionUnit,
    );
    const transcriptionRetentionMode = normalizeRetentionMode(
      (await store.get("transcription_retention_mode")) ?? "time",
      "time",
    );
    const transcriptionRetentionAmount = normalizeTranscriptionRetentionAmount(
      await store.get("transcription_retention_amount"),
    );

    const legacyHotkeys: LegacyHotkeySettings = {
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
    };

    const rawShortcutCards = await store.get("hotkey_shortcuts");
    const normalizedShortcutCards =
      normalizeHotkeyShortcutCards(rawShortcutCards);
    const hotkey_shortcuts =
      normalizedShortcutCards ?? buildShortcutCardsFromLegacy(legacyHotkeys);

    const settings: AppSettings = {
      settings_version: settingsVersion,
      policy_state: normalizePolicyState(await store.get("policy_state")),
      license_state: normalizeLicenseState(await store.get("license_state")),
      token_exchange_trigger_set: normalizeTokenExchangeTriggerSet(
        await store.get("token_exchange_trigger_set"),
      ),
      toggle_hotkey: getFirstHotkeyByType(hotkey_shortcuts, "toggle"),
      hold_hotkey: getFirstHotkeyByType(hotkey_shortcuts, "hold"),
      paste_last_hotkey: getFirstHotkeyByType(hotkey_shortcuts, "paste_last"),
      retry_hotkey: getFirstHotkeyByType(hotkey_shortcuts, "retry"),
      quick_ask_hold_hotkey: getFirstHotkeyByType(
        hotkey_shortcuts,
        "quick_ask_hold",
      ),
      quick_ask_toggle_hotkey: getFirstHotkeyByType(
        hotkey_shortcuts,
        "quick_ask_toggle",
      ),
      hotkey_shortcuts,

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
        if (!normalized) return DEFAULT_ACCENT_HEX;

        return normalized;
      })(),
      rewrite_llm_enabled:
        (await store.get<boolean>("rewrite_llm_enabled")) ?? false,
      quick_replace_enabled:
        (await store.get<boolean>("quick_replace_enabled")) ?? false,
      cleanup_prompt_sections: await (async () => {
        const raw = await store.get<unknown>("cleanup_prompt_sections");
        const normalized = normalizeCleanupPromptSections(raw);

        return normalized;
      })(),
      rewrite_program_prompt_profiles,
      stt_provider: (await store.get<string | null>("stt_provider")) ?? null,
      stt_model: (await store.get<string | null>("stt_model")) ?? null,
      stt_language: normalizeSttLanguage(
        await store.get("stt_language"),
        DEFAULT_STT_LANGUAGE,
      ),
      stt_transcription_prompt:
        (await store.get<string | null>("stt_transcription_prompt")) ?? null,
      stt_live_output: (await store.get<boolean>("stt_live_output")) ?? false,
      stt_simulated_streaming:
        (await store.get<boolean>("stt_simulated_streaming")) ?? false,
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
      quick_ask_dismiss_mode: normalizeQuickAskDismissMode(
        await store.get("quick_ask_dismiss_mode"),
      ),

      quick_ask_include_selected_text:
        (await store.get<boolean>("quick_ask_include_selected_text")) ?? false,
      windows_clipboard_fallback_for_context_capture:
        (await store.get<boolean>(
          "windows_clipboard_fallback_for_context_capture",
        )) ?? false,

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
      output_smart_paste_protection:
        (await store.get<boolean>("output_smart_paste_protection")) ?? false,

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

      max_saved_recordings: maxSavedRecordings,
      recordings_retention_mode: recordingsRetentionMode,
      recordings_retention_amount: recordingsRetentionAmount,
      recordings_retention_unit: recordingsRetentionUnit,
      recordings_retention_value: recordingsRetentionValue,

      request_logs_retention_mode: normalizeRequestLogsRetentionMode(
        await store.get("request_logs_retention_mode"),
      ),
      request_logs_retention_amount: normalizeRequestLogsRetentionAmount(
        await store.get("request_logs_retention_amount"),
      ),
      request_logs_retention_days: normalizeRequestLogsRetentionDays(
        await store.get("request_logs_retention_days"),
      ),
      request_logs_privacy_mode:
        (await store.get<boolean>("request_logs_privacy_mode")) ?? false,

      transcription_retention_mode: transcriptionRetentionMode,
      transcription_retention_amount: transcriptionRetentionAmount,
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

      // ============================================================================
      // OCR (Active Window Context) settings
      // ============================================================================

      ocr_base_url: (await store.get<string | null>("ocr_base_url")) ?? null,
      ocr_model: (await store.get<string | null>("ocr_model")) ?? null,
      ocr_auth_mode: normalizeOcrAuthMode(await store.get("ocr_auth_mode")),
      ocr_prompt: (await store.get<string | null>("ocr_prompt")) ?? "",
      ocr_max_tokens: (await store.get<number | null>("ocr_max_tokens")) ?? 512,
      ocr_temperature: (await store.get<number | null>("ocr_temperature")) ?? 0,
      ocr_top_p: (await store.get<number | null>("ocr_top_p")) ?? 1,
      ocr_request_timeout_ms:
        (await store.get<number | null>("ocr_request_timeout_ms")) ?? 2000,
      ocr_context_max_chars:
        (await store.get<number | null>("ocr_context_max_chars")) ?? 8000,

      rewrite_active_window_ocr_mode: normalizeActiveWindowOcrMode(
        await store.get("rewrite_active_window_ocr_mode"),
      ),
      quick_replace_active_window_ocr_mode: normalizeActiveWindowOcrMode(
        await store.get("quick_replace_active_window_ocr_mode"),
      ),
      quick_ask_active_window_ocr_mode: normalizeActiveWindowOcrMode(
        await store.get("quick_ask_active_window_ocr_mode"),
      ),
      ocr_auto_capture_timing: normalizeOcrAutoCaptureTiming(
        await store.get("ocr_auto_capture_timing"),
      ),
      ocr_hallucination_protection:
        (await store.get<boolean | null>("ocr_hallucination_protection")) ??
        true,
      ocr_hallucination_threshold:
        (await store.get<number | null>("ocr_hallucination_threshold")) ?? 2500,
      ocr_resize_max_dimension:
        (await store.get<number | null>("ocr_resize_max_dimension")) ?? 0,
      ocr_resize_filter: normalizeOcrResizeFilter(
        await store.get("ocr_resize_filter"),
      ),
    };

    // Mirror the accent so index.html can apply it synchronously at next launch.
    tryWriteLocalStorage(LOCAL_ACCENT_COLOR_KEY, settings.accent_color ?? null);

    return settings;
  },

  async reloadSettingsFromDisk(): Promise<void> {
    await reloadSettingsStoreFromDisk();
  },

  async updateAccentColor(color: string | null): Promise<void> {
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
      await applySettingsPatch({ deleteKeys: ["accent_color"] });
    } else {
      await applySettingsPatch({ patch: { accent_color: normalized } });
    }
  },

  async updateMainWindowCloseBehavior(
    behavior: MainWindowCloseBehavior,
  ): Promise<void> {
    const normalized = normalizeMainWindowCloseBehavior(behavior);
    await applySettingsPatch({
      patch: { main_window_close_behavior: normalized },
    });
  },

  async updateGithubBackupGistId(gistId: string | null): Promise<void> {
    const trimmed = (gistId ?? "").trim();
    if (!trimmed) {
      await applySettingsPatch({ deleteKeys: ["github_backup_gist_id"] });
    } else {
      await applySettingsPatch({ patch: { github_backup_gist_id: trimmed } });
    }
  },

  async updateTokenExchangeTriggerSet(
    triggerSet: TokenExchangeTriggerSet,
  ): Promise<void> {
    await applySettingsPatch({
      patch: {
        token_exchange_trigger_set:
          normalizeTokenExchangeTriggerSet(triggerSet),
      },
    });
  },

  async updateToggleHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { toggle_hotkey: hotkey } });
  },

  async updateHoldHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { hold_hotkey: hotkey } });
  },

  async updatePasteLastHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { paste_last_hotkey: hotkey } });
  },

  async updateRetryHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { retry_hotkey: hotkey } });
  },

  async updateQuickAskHoldHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { quick_ask_hold_hotkey: hotkey } });
  },

  async updateQuickAskToggleHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({ patch: { quick_ask_toggle_hotkey: hotkey } });
  },

  async createHotkeyShortcutCard(
    card: HotkeyShortcutCard,
  ): Promise<HotkeyShortcutCard[]> {
    return invoke("hotkey_shortcut_cards_create", { card });
  },

  async updateHotkeyShortcutCard(
    cardId: string,
    hotkey: HotkeyConfig | null,
  ): Promise<HotkeyShortcutCard[]> {
    return invoke("hotkey_shortcut_cards_update", { cardId, hotkey });
  },

  async deleteHotkeyShortcutCard(
    cardId: string,
  ): Promise<HotkeyShortcutCard[]> {
    return invoke("hotkey_shortcut_cards_delete", { cardId });
  },

  /**
   * Legacy alias (pre split): Quick Ask hotkey (hold-to-record).
   *
   * Writes both keys for backward compatibility.
   */
  async updateQuickAskHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    await applySettingsPatch({
      patch: { quick_ask_hotkey: hotkey, quick_ask_hold_hotkey: hotkey },
    });
  },

  async updateQuickAskProvider(provider: string | null): Promise<void> {
    await applySettingsPatch({ patch: { quick_ask_provider: provider } });
  },

  async updateQuickAskModel(model: string | null): Promise<void> {
    await applySettingsPatch({ patch: { quick_ask_model: model } });
  },

  async updateQuickAskSystemPrompt(prompt: string | null): Promise<void> {
    const normalized = typeof prompt === "string" ? prompt.trim() : "";
    await applySettingsPatch({
      patch: {
        quick_ask_system_prompt: normalized.length > 0 ? normalized : null,
      },
    });
  },

  async updateQuickAskDismissMode(mode: QuickAskDismissMode): Promise<void> {
    const normalized = normalizeQuickAskDismissMode(mode);
    await applySettingsPatch({
      patch: { quick_ask_dismiss_mode: normalized },
    });
  },

  async updateQuickAskIncludeSelectedText(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { quick_ask_include_selected_text: Boolean(enabled) },
    });
  },

  async updateWindowsClipboardFallbackForContextCapture(
    enabled: boolean,
  ): Promise<void> {
    await applySettingsPatch({
      patch: {
        windows_clipboard_fallback_for_context_capture: Boolean(enabled),
      },
    });
  },

  async updateQuickAskConversationHistoryEnabled(
    enabled: boolean,
  ): Promise<void> {
    await applySettingsPatch({
      patch: { quick_ask_conversation_history_enabled: Boolean(enabled) },
    });
  },

  async updateQuickAskConversationHistoryCount(count: number): Promise<void> {
    const normalized = normalizeQuickAskConversationHistoryCount(count);
    await applySettingsPatch({
      patch: { quick_ask_conversation_history_count: normalized },
    });
  },

  async updateQuickAskOpenAiReasoningEffort(
    effort: OpenAiReasoningEffort | null,
  ): Promise<void> {
    if (effort == null) {
      await applySettingsPatch({
        deleteKeys: ["quick_ask_openai_reasoning_effort"],
      });
      return;
    }
    await applySettingsPatch({
      patch: {
        quick_ask_openai_reasoning_effort:
          normalizeOpenAiReasoningEffort(effort),
      },
    });
  },

  async updateQuickAskAnthropicThinkingBudget(
    budget: number | null,
  ): Promise<void> {
    if (budget == null) {
      await applySettingsPatch({
        deleteKeys: ["quick_ask_anthropic_thinking_budget"],
      });
      return;
    }
    await applySettingsPatch({
      patch: {
        quick_ask_anthropic_thinking_budget:
          normalizeAnthropicThinkingBudget(budget),
      },
    });
  },

  async updateQuickAskGeminiThinkingBudget(
    budget: number | null,
  ): Promise<void> {
    if (budget == null) {
      await applySettingsPatch({
        deleteKeys: ["quick_ask_gemini_thinking_budget"],
      });
      return;
    }
    await applySettingsPatch({
      patch: {
        quick_ask_gemini_thinking_budget: normalizeGeminiThinkingBudget(budget),
      },
    });
  },

  async updateQuickAskGeminiThinkingLevel(
    level: "minimal" | "low" | "medium" | "high" | null,
  ): Promise<void> {
    if (level == null) {
      await applySettingsPatch({
        deleteKeys: ["quick_ask_gemini_thinking_level"],
      });
      return;
    }
    await applySettingsPatch({
      patch: {
        quick_ask_gemini_thinking_level: normalizeGeminiThinkingLevel(level),
      },
    });
  },

  async updateSelectedMic(micId: string | null): Promise<void> {
    await applySettingsPatch({ patch: { selected_mic_id: micId } });
  },

  async updateSoundEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { sound_enabled: enabled } });
  },

  async updateHotkeyDebugEnabled(enabled: boolean): Promise<void> {
    // Update backend runtime flag immediately so debug events can start flowing
    // without waiting for store writes / reloads.
    await invoke("set_hotkey_debug_enabled_runtime", { enabled: !!enabled });

    await applySettingsPatch({
      patch: { hotkey_debug_enabled: !!enabled },
    });
  },

  async updateAudioCue(cue: AudioCue): Promise<void> {
    await applySettingsPatch({ patch: { audio_cue: normalizeAudioCue(cue) } });
  },

  async updateRewriteLlmEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { rewrite_llm_enabled: enabled } });
  },

  async updateCleanupPromptSections(
    sections: CleanupPromptSections | null,
  ): Promise<void> {
    await applySettingsPatch({ patch: { cleanup_prompt_sections: sections } });
  },

  async updateRewriteProgramPromptProfiles(
    profiles: RewriteProgramPromptProfile[],
  ): Promise<void> {
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

    await applySettingsPatch({
      patch: { rewrite_program_prompt_profiles: sanitized },
    });
  },

  async updateSTTProvider(provider: string | null): Promise<void> {
    await applySettingsPatch({ patch: { stt_provider: provider } });
  },

  async updateCerebrasFreeTier(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { cerebras_free_tier: !!enabled } });
  },

  async updateGroqFreeTier(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { groq_free_tier: !!enabled } });
  },

  async updateCohereFreeTier(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { cohere_free_tier: !!enabled } });
  },

  async updateAssemblyAiFreeTier(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { assemblyai_free_tier: !!enabled } });
  },

  async updateSpeechmaticsFreeTier(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { speechmatics_free_tier: !!enabled } });
  },

  async updateSTTModel(model: string | null): Promise<void> {
    await applySettingsPatch({ patch: { stt_model: model } });
  },

  async updateSTTLiveOutput(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { stt_live_output: !!enabled } });
  },

  async updateSTTSimulatedStreaming(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { stt_simulated_streaming: !!enabled } });
  },

  async updateSTTLanguage(language: string): Promise<void> {
    const normalized = normalizeSttLanguage(language, DEFAULT_STT_LANGUAGE);
    await applySettingsPatch({ patch: { stt_language: normalized } });
  },

  async updateSTTTranscriptionPrompt(prompt: string | null): Promise<void> {
    await applySettingsPatch({ patch: { stt_transcription_prompt: prompt } });
  },

  async updateWhisperServerBaseUrl(baseUrl: string | null): Promise<void> {
    const normalized = baseUrl?.trim() ? baseUrl.trim() : null;
    await applySettingsPatch({
      patch: { whisper_server_base_url: normalized },
    });
  },

  async updateOllamaUrl(baseUrl: string | null): Promise<void> {
    const normalized = baseUrl?.trim() ? baseUrl.trim() : null;
    await applySettingsPatch({ patch: { ollama_url: normalized } });
  },

  async updateLocalWhisperModelId(modelId: string | null): Promise<void> {
    const normalized = modelId?.trim() ? modelId.trim().toLowerCase() : null;
    await applySettingsPatch({
      patch: { local_whisper_model_id: normalized },
    });
  },

  async updateLocalWhisperLoadMode(mode: LocalWhisperLoadMode): Promise<void> {
    await applySettingsPatch({
      patch: { local_whisper_load_mode: normalizeLocalWhisperLoadMode(mode) },
    });
  },

  async updateProxySettings(proxySettings: ProxySettings): Promise<void> {
    await applySettingsPatch({
      patch: { proxy_settings: normalizeProxySettings(proxySettings) },
    });
  },

  async updateLLMProvider(provider: string | null): Promise<void> {
    await applySettingsPatch({ patch: { llm_provider: provider } });
  },

  async updateLLMModel(model: string | null): Promise<void> {
    await applySettingsPatch({ patch: { llm_model: model } });
  },

  async updateOpenAiReasoningEffort(
    effort: OpenAiReasoningEffort | null,
  ): Promise<void> {
    if (effort == null) {
      await applySettingsPatch({ deleteKeys: ["openai_reasoning_effort"] });
      return;
    }
    await applySettingsPatch({
      patch: {
        openai_reasoning_effort: normalizeOpenAiReasoningEffort(effort),
      },
    });
  },

  async updateAnthropicThinkingBudget(budget: number | null): Promise<void> {
    if (budget == null) {
      await applySettingsPatch({ deleteKeys: ["anthropic_thinking_budget"] });
      return;
    }
    await applySettingsPatch({
      patch: {
        anthropic_thinking_budget: normalizeAnthropicThinkingBudget(budget),
      },
    });
  },

  async updateGeminiThinkingBudget(budget: number | null): Promise<void> {
    if (budget == null) {
      await applySettingsPatch({ deleteKeys: ["gemini_thinking_budget"] });
      return;
    }
    await applySettingsPatch({
      patch: { gemini_thinking_budget: normalizeGeminiThinkingBudget(budget) },
    });
  },

  async updateGeminiThinkingLevel(
    level: "minimal" | "low" | "medium" | "high" | null,
  ): Promise<void> {
    if (level == null) {
      await applySettingsPatch({ deleteKeys: ["gemini_thinking_level"] });
      return;
    }
    await applySettingsPatch({
      patch: { gemini_thinking_level: normalizeGeminiThinkingLevel(level) },
    });
  },

  async updatePlayingAudioHandling(
    handling: PlayingAudioHandling,
  ): Promise<void> {
    await applySettingsPatch({ patch: { playing_audio_handling: handling } });
  },

  async updateSTTTimeout(timeoutSeconds: number | null): Promise<void> {
    await applySettingsPatch({
      patch: { stt_timeout_seconds: timeoutSeconds },
    });
  },

  async updateOverlayMode(mode: OverlayMode): Promise<void> {
    await applySettingsPatch({ patch: { overlay_mode: mode } });
    // Apply the mode immediately
    await invoke("set_overlay_mode", { mode });
    // Best-effort: notify other windows immediately (the backend also emits this).
    await emitTyped("settings-changed", {});
  },

  async updateOverlayShowDetailedLoading(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { overlay_show_detailed_loading: !!enabled },
    });
  },

  async updateOverlayMonitorTarget(
    target: OverlayMonitorTarget,
  ): Promise<void> {
    const normalized = normalizeOverlayMonitorTarget(target);
    await applySettingsPatch({ patch: { overlay_monitor_target: normalized } });

    // Best-effort: immediately re-snap overlay windows to the selected monitor.
    // This uses the user's saved widget_position.
    try {
      const store = await getStore();
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
  },

  async updateWidgetPosition(position: WidgetPosition): Promise<void> {
    await applySettingsPatch({ patch: { widget_position: position } });
    // Apply the position immediately
    await invoke("set_widget_position", { position });
  },

  async updateOutputMode(mode: OutputMode): Promise<void> {
    await applySettingsPatch({ patch: { output_mode: mode } });
  },

  async updateOutputHitEnter(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { output_hit_enter: enabled } });
  },

  async updateOutputSmartPasteProtection(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { output_smart_paste_protection: Boolean(enabled) },
    });
  },

  async updateQuietAudioGateEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { quiet_audio_gate_enabled: enabled } });
  },

  async updateQuietAudioMinDurationSecs(seconds: number): Promise<void> {
    await applySettingsPatch({
      patch: { quiet_audio_min_duration_secs: seconds },
    });
  },

  async updateQuietAudioRmsDbfsThreshold(dbfs: number): Promise<void> {
    await applySettingsPatch({
      patch: { quiet_audio_rms_dbfs_threshold: dbfs },
    });
  },

  async updateQuietAudioPeakDbfsThreshold(dbfs: number): Promise<void> {
    await applySettingsPatch({
      patch: { quiet_audio_peak_dbfs_threshold: dbfs },
    });
  },

  async updateQuietAudioRequireSpeech(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { quiet_audio_require_speech: enabled },
    });
  },

  async updateHotMicEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { hot_mic_enabled: !!enabled } });
  },

  async updateHotMicPreRollMs(ms: number): Promise<void> {
    const normalized = Number.isFinite(ms) ? Math.max(0, Math.round(ms)) : 0;
    await applySettingsPatch({ patch: { hot_mic_pre_roll_ms: normalized } });
  },

  async updateMicAutoRecoverEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { mic_auto_recover_enabled: !!enabled },
    });
  },

  async updateNoiseGateThresholdDbfs(
    thresholdDbfs: number | null,
  ): Promise<void> {
    const normalized = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
    await applySettingsPatch({
      patch: {
        noise_gate_threshold_dbfs: normalized,
        // Best-effort legacy key for downgrade compatibility.
        noise_gate_strength: noiseGateThresholdDbfsToStrength(normalized),
      },
    });
  },

  async updateNoiseGateStrength(strength: number): Promise<void> {
    const normalizedStrength = normalizeNoiseGateStrength(strength);
    await applySettingsPatch({
      patch: {
        noise_gate_strength: normalizedStrength,
        // Keep the new key in sync for newer builds.
        noise_gate_threshold_dbfs:
          noiseGateStrengthToThresholdDbfs(normalizedStrength),
      },
    });
  },

  async updateAudioDownmixToMono(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { audio_downmix_to_mono: enabled } });
  },

  async updateAudioResampleTo16khz(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { audio_resample_to_16khz: enabled } });
  },

  async updateAudioHighpassEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { audio_highpass_enabled: enabled } });
  },

  async updateAudioAgcEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({ patch: { audio_agc_enabled: enabled } });
  },

  async updateAudioNoiseSuppressionEnabled(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { audio_noise_suppression_enabled: enabled },
    });
  },

  async updateMaxSavedRecordings(max: number): Promise<void> {
    await applySettingsPatch({
      patch: { max_saved_recordings: normalizeMaxSavedRecordings(max) },
    });
  },

  async updateRecordingsRetention(params: {
    mode: "amount" | "time";
    amount: number;
    unit: TranscriptionRetentionUnit;
    value: number;
  }): Promise<void> {
    const mode = normalizeRetentionMode(params.mode, "amount");
    const amount = normalizeMaxSavedRecordings(params.amount);
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const value = normalizeTranscriptionRetentionValue(params.value, unit);
    await applySettingsPatch({
      patch: {
        recordings_retention_mode: mode,
        recordings_retention_amount: amount,
        recordings_retention_unit: unit,
        recordings_retention_value: value,
      },
    });
  },

  async updateRequestLogsRetention(params: {
    mode: AppSettings["request_logs_retention_mode"];
    amount: number;
    days: number;
  }): Promise<void> {
    const mode = normalizeRequestLogsRetentionMode(params.mode);
    const amount = normalizeRequestLogsRetentionAmount(params.amount);
    const days = normalizeRequestLogsRetentionDays(params.days);
    await applySettingsPatch({
      patch: {
        request_logs_retention_mode: mode,
        request_logs_retention_amount: amount,
        request_logs_retention_days: days,
      },
    });
  },

  async updateRequestLogsPrivacyMode(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { request_logs_privacy_mode: Boolean(enabled) },
    });
  },

  async updateTranscriptionRetentionPolicy(params: {
    mode: "amount" | "time";
    amount: number;
    unit: TranscriptionRetentionUnit;
    value: number;
  }): Promise<void> {
    const mode = normalizeRetentionMode(params.mode, "time");
    const amount = normalizeTranscriptionRetentionAmount(params.amount);
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const normalizedValue = normalizeTranscriptionRetentionValue(
      params.value,
      unit,
    );
    const effectiveValue = mode === "time" ? normalizedValue : 0;
    await applySettingsPatch({
      patch: {
        transcription_retention_mode: mode,
        transcription_retention_amount: amount,
        transcription_retention_unit: unit,
        transcription_retention_value: effectiveValue,
        // Legacy key (kept for backward compatibility)
        ...(unit === "days"
          ? { transcription_retention_days: effectiveValue }
          : {}),
      },
    });
  },

  async updateTranscriptionRetentionDays(days: number): Promise<void> {
    const normalized = normalizeTranscriptionRetentionValue(days, "days");
    await applySettingsPatch({
      patch: {
        // Legacy key (kept for backward compatibility)
        transcription_retention_days: normalized,
        // New keys
        transcription_retention_unit: "days",
        transcription_retention_value: normalized,
      },
    });
  },

  async updateTranscriptionRetention(params: {
    unit: TranscriptionRetentionUnit;
    value: number;
  }): Promise<void> {
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const value = normalizeTranscriptionRetentionValue(params.value, unit);
    await applySettingsPatch({
      patch: {
        transcription_retention_unit: unit,
        transcription_retention_value: value,
        // Best-effort: keep the legacy days key in sync when unit is days.
        // (If unit is hours, we leave the legacy key untouched to avoid silently
        // changing semantics for older builds.)
        ...(unit === "days" ? { transcription_retention_days: value } : {}),
      },
    });
  },

  async updateTranscriptionRetentionDeleteRecordings(
    enabled: boolean,
  ): Promise<void> {
    await applySettingsPatch({
      patch: {
        transcription_retention_delete_recordings:
          normalizeTranscriptionRetentionDeleteRecordings(enabled),
      },
    });
  },

  async updateStatsRetention(params: {
    unit: TranscriptionRetentionUnit;
    value: number;
    max_bytes?: number;
  }): Promise<void> {
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const value = normalizeTranscriptionRetentionValue(params.value, unit);
    await applySettingsPatch({
      patch: {
        stats_retention_unit: unit,
        stats_retention_value: value,
        ...(typeof params.max_bytes === "number"
          ? {
              stats_retention_max_bytes: normalizeStatsRetentionMaxBytes(
                params.max_bytes,
              ),
            }
          : {}),
      },
    });
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
    await applySettingsPatch({
      patch: {
        [SETTINGS_GUIDE_STATE_KEY]: normalizeSettingsGuideState(state),
      },
    });

    try {
      if (typeof window !== "undefined" && window.localStorage) {
        window.localStorage.setItem("tv_settings_guide_state", state);
      }
    } catch {
      // ignore
    }

    // No explicit event emit here; the backend patch command emits settings-changed.
  },

  async resetHotkeysToDefaults(): Promise<void> {
    const hotkey_shortcuts: HotkeyShortcutCard[] = [];
    const pushCard = (type: HotkeyType, hotkey: HotkeyConfig | null) => {
      if (!hotkey) return;
      hotkey_shortcuts.push({
        id: createHotkeyShortcutId(),
        type,
        hotkey,
      });
    };

    pushCard("toggle", DEFAULT_TOGGLE_HOTKEY);
    pushCard("hold", DEFAULT_HOLD_HOTKEY);
    pushCard("paste_last", DEFAULT_PASTE_LAST_HOTKEY);
    pushCard("retry", DEFAULT_RETRY_HOTKEY);
    pushCard("quick_ask_hold", DEFAULT_QUICK_ASK_HOLD_HOTKEY);
    pushCard("quick_ask_toggle", DEFAULT_QUICK_ASK_TOGGLE_HOTKEY);

    await applySettingsPatch({
      patch: {
        toggle_hotkey: DEFAULT_TOGGLE_HOTKEY,
        hold_hotkey: DEFAULT_HOLD_HOTKEY,
        paste_last_hotkey: DEFAULT_PASTE_LAST_HOTKEY,
        retry_hotkey: DEFAULT_RETRY_HOTKEY,
        quick_ask_hold_hotkey: DEFAULT_QUICK_ASK_HOLD_HOTKEY,
        quick_ask_toggle_hotkey: DEFAULT_QUICK_ASK_TOGGLE_HOTKEY,
        // Legacy alias (pre split): keep in sync.
        quick_ask_hotkey: DEFAULT_QUICK_ASK_HOLD_HOTKEY,
        hotkey_shortcuts,
      },
    });
  },

  // ============================================================================
  // OCR (Active Window Context) settings
  // ============================================================================

  async updateOcrBaseUrl(baseUrl: string | null): Promise<void> {
    const normalized = baseUrl?.trim() ? baseUrl.trim() : null;
    await applySettingsPatch({ patch: { ocr_base_url: normalized } });
  },

  async updateOcrModel(model: string | null): Promise<void> {
    const normalized = model?.trim() ? model.trim() : null;
    await applySettingsPatch({ patch: { ocr_model: normalized } });
  },

  async updateOcrAuthMode(mode: OcrAuthMode): Promise<void> {
    await applySettingsPatch({ patch: { ocr_auth_mode: mode } });
  },

  async updateOcrPrompt(prompt: string): Promise<void> {
    const normalized = prompt?.trim() ? prompt.trim() : null;
    await applySettingsPatch({ patch: { ocr_prompt: normalized } });
  },

  async updateOcrMaxTokens(maxTokens: number): Promise<void> {
    const normalized = Math.max(1, Math.floor(maxTokens));
    await applySettingsPatch({ patch: { ocr_max_tokens: normalized } });
  },

  async updateOcrTemperature(temperature: number): Promise<void> {
    const normalized = Math.max(0, Math.min(2, temperature));
    await applySettingsPatch({ patch: { ocr_temperature: normalized } });
  },

  async updateOcrTopP(topP: number): Promise<void> {
    const normalized = Math.max(0, Math.min(1, topP));
    await applySettingsPatch({ patch: { ocr_top_p: normalized } });
  },

  async updateOcrRequestTimeoutMs(timeoutMs: number): Promise<void> {
    const normalized = Math.max(100, Math.floor(timeoutMs));
    await applySettingsPatch({ patch: { ocr_request_timeout_ms: normalized } });
  },

  async updateOcrContextMaxChars(maxChars: number): Promise<void> {
    const normalized = Math.max(0, Math.floor(maxChars));
    await applySettingsPatch({ patch: { ocr_context_max_chars: normalized } });
  },

  async updateOcrAutoCaptureTiming(
    timing: OcrAutoCaptureTiming,
  ): Promise<void> {
    const normalized = timing === "on_start" ? "on_start" : "on_stop";
    await applySettingsPatch({
      patch: { ocr_auto_capture_timing: normalized },
    });
  },

  async updateOcrHallucinationProtection(enabled: boolean): Promise<void> {
    await applySettingsPatch({
      patch: { ocr_hallucination_protection: enabled },
    });
  },

  async updateOcrHallucinationThreshold(value: number): Promise<void> {
    await applySettingsPatch({
      patch: { ocr_hallucination_threshold: value },
    });
  },

  async updateOcrResizeMaxDimension(value: number): Promise<void> {
    await applySettingsPatch({
      patch: { ocr_resize_max_dimension: value },
    });
  },

  async updateOcrResizeFilter(filter: OcrResizeFilter): Promise<void> {
    await applySettingsPatch({
      patch: { ocr_resize_filter: filter },
    });
  },
};
