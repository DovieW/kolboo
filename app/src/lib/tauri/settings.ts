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
import {
	createHotkeyShortcutId,
	type HotkeyConfig,
	type HotkeyShortcutCard,
	HotkeyShortcutCardsSchema,
	type HotkeyType,
	normalizeHotkeyConfig,
} from "../hotkeys";
import {
	buildTelemetryDisclosureResolutionPatch,
	POSTHOG_ANALYTICS_ENABLED_KEY,
	TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY,
	TELEMETRY_DISCLOSURE_VERSION_KEY,
} from "../settings/telemetryDisclosure";
import { DEFAULT_STT_LANGUAGE, normalizeSttLanguage } from "../sttLanguages";
import { emitTyped } from "./events";
import { DEFAULT_SETTINGS_VALUES } from "./settingsDefaults";
import {
	isRecord,
	noiseGateStrengthToThresholdDbfs,
	noiseGateThresholdDbfsToStrength,
	normalizeActiveWindowOcrMode,
	normalizeAnthropicThinkingBudget,
	normalizeAudioCue,
	normalizeBooleanSetting,
	normalizeCleanupPromptSections,
	normalizeGeminiThinkingBudget,
	normalizeGeminiThinkingLevel,
	normalizeLicenseState,
	normalizeLocalWhisperLoadMode,
	normalizeLocalWhisperModelId,
	normalizeMainWindowCloseBehavior,
	normalizeMaxSavedRecordings,
	normalizeNoiseGateStrength,
	normalizeNoiseGateThresholdDbfs,
	normalizeNonEmptyStringSetting,
	normalizeOcrAuthMode,
	normalizeOcrAutoCaptureTiming,
	normalizeOcrResizeFilter,
	normalizeOpenAiReasoningEffort,
	normalizeOutputMode,
	normalizeOverlayModeValue,
	normalizeOverlayMonitorTarget,
	normalizePlayingAudioHandling,
	normalizePolicyEnforcedFields,
	normalizePolicySource,
	normalizePolicyState,
	normalizePolicyTimestamp,
	normalizeProxySettings,
	normalizeQuickAskConversationHistoryCount,
	normalizeQuickAskDismissMode,
	normalizeRequestLogsRetentionAmount,
	normalizeRequestLogsRetentionDays,
	normalizeRequestLogsRetentionMode,
	normalizeRetentionMode,
	normalizeRewriteProfile,
	normalizeStatsRetentionMaxBytes,
	normalizeTokenExchangeTriggerSet,
	normalizeTranscriptionRetentionAmount,
	normalizeTranscriptionRetentionDeleteRecordings,
	normalizeTranscriptionRetentionUnit,
	normalizeTranscriptionRetentionValue,
} from "./settingsNormalizers";
import { applySettingsRuntimeSyncPolicy } from "./settingsSync";
import { settingValueView } from "./settingsViews";
import type {
	AppSettings,
	AudioCue,
	CleanupPromptSections,
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
	RewriteProgramPromptProfile,
	SettingsGuideState,
	TokenExchangeTriggerSet,
	TranscriptionRetentionUnit,
	WidgetPosition,
} from "./types";

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
	posthog_analytics_enabled: ["disable_product_analytics"],
	disable_product_analytics: [POSTHOG_ANALYTICS_ENABLED_KEY],
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
	const enforced_fields = normalizePolicyEnforcedFields(raw.enforced_fields);

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

	if (hasPatch || hasDeletes) {
		await invoke("settings_apply_patch", {
			patch: prepared.patch,
			deleteKeys: prepared.deleteKeys,
		});
	}

	await applySettingsRuntimeSyncPolicy({
		patch: prepared.patch,
		deleteKeys: prepared.deleteKeys,
		backendEventEmitted: hasPatch || hasDeletes,
		policyNormalized: prepared.policyNormalized,
		policyViolations: prepared.violations,
		invoke,
		emitSettingsChanged: (payload) => emitTyped("settings-changed", payload),
	});

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
		const readSettingValue = async <T>(
			key: string,
			defaultValue: T,
			normalize?: (value: unknown) => T | null,
		): Promise<T> => {
			return settingValueView({
				record: { [key]: await store.get(key) },
				key,
				defaultValue,
				normalize,
			}).value;
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

			hotkey_debug_enabled: await readSettingValue(
				"hotkey_debug_enabled",
				DEFAULT_SETTINGS_VALUES.hotkey_debug_enabled,
				normalizeBooleanSetting,
			),

			selected_mic_id:
				(await store.get<string | null>("selected_mic_id")) ?? null,
			sound_enabled: await readSettingValue(
				"sound_enabled",
				DEFAULT_SETTINGS_VALUES.sound_enabled,
				normalizeBooleanSetting,
			),
			audio_cue: normalizeAudioCue(await store.get("audio_cue")),
			accent_color: await (async () => {
				const raw = (await store.get<string | null>("accent_color")) ?? null;
				const normalized = normalizeHexColor(raw);

				// If unset/invalid, default to the app's default accent.
				// (Tangerine is an explicit option in the UI, not the implicit default.)
				if (!normalized) return DEFAULT_ACCENT_HEX;

				return normalized;
			})(),
			rewrite_llm_enabled: await readSettingValue(
				"rewrite_llm_enabled",
				DEFAULT_SETTINGS_VALUES.rewrite_llm_enabled,
				normalizeBooleanSetting,
			),
			quick_replace_enabled: await readSettingValue(
				"quick_replace_enabled",
				DEFAULT_SETTINGS_VALUES.quick_replace_enabled,
				normalizeBooleanSetting,
			),
			cleanup_prompt_sections: await (async () => {
				const raw = await store.get<unknown>("cleanup_prompt_sections");
				const normalized = normalizeCleanupPromptSections(raw);

				return normalized;
			})(),
			rewrite_program_prompt_profiles,
			stt_provider: await readSettingValue(
				"stt_provider",
				DEFAULT_SETTINGS_VALUES.stt_provider,
				normalizeNonEmptyStringSetting,
			),
			stt_model: (await store.get<string | null>("stt_model")) ?? null,
			stt_language: normalizeSttLanguage(
				await store.get("stt_language"),
				DEFAULT_STT_LANGUAGE,
			),
			stt_transcription_prompt:
				(await store.get<string | null>("stt_transcription_prompt")) ?? null,
			stt_live_output: await readSettingValue(
				"stt_live_output",
				DEFAULT_SETTINGS_VALUES.stt_live_output,
				normalizeBooleanSetting,
			),
			stt_simulated_streaming: await readSettingValue(
				"stt_simulated_streaming",
				DEFAULT_SETTINGS_VALUES.stt_simulated_streaming,
				normalizeBooleanSetting,
			),
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
				(await store.get("quick_ask_dismiss_mode")) ??
					DEFAULT_SETTINGS_VALUES.quick_ask_dismiss_mode,
			),

			quick_ask_include_selected_text: await readSettingValue(
				"quick_ask_include_selected_text",
				DEFAULT_SETTINGS_VALUES.quick_ask_include_selected_text,
				normalizeBooleanSetting,
			),
			windows_clipboard_fallback_for_context_capture: await readSettingValue(
				"windows_clipboard_fallback_for_context_capture",
				DEFAULT_SETTINGS_VALUES.windows_clipboard_fallback_for_context_capture,
				normalizeBooleanSetting,
			),

			quick_ask_conversation_history_enabled: await readSettingValue(
				"quick_ask_conversation_history_enabled",
				DEFAULT_SETTINGS_VALUES.quick_ask_conversation_history_enabled,
				normalizeBooleanSetting,
			),
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
				(await store.get<boolean>("cerebras_free_tier")) ??
				DEFAULT_SETTINGS_VALUES.cerebras_free_tier,
			groq_free_tier:
				(await store.get<boolean>("groq_free_tier")) ??
				DEFAULT_SETTINGS_VALUES.groq_free_tier,
			cohere_free_tier:
				(await store.get<boolean>("cohere_free_tier")) ??
				DEFAULT_SETTINGS_VALUES.cohere_free_tier,
			assemblyai_free_tier:
				(await store.get<boolean>("assemblyai_free_tier")) ??
				DEFAULT_SETTINGS_VALUES.assemblyai_free_tier,
			speechmatics_free_tier:
				(await store.get<boolean>("speechmatics_free_tier")) ??
				DEFAULT_SETTINGS_VALUES.speechmatics_free_tier,
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
			overlay_mode: await readSettingValue(
				"overlay_mode",
				DEFAULT_SETTINGS_VALUES.overlay_mode,
				normalizeOverlayModeValue,
			),
			overlay_show_detailed_loading: await readSettingValue(
				"overlay_show_detailed_loading",
				DEFAULT_SETTINGS_VALUES.overlay_show_detailed_loading,
				normalizeBooleanSetting,
			),
			overlay_monitor_target: normalizeOverlayMonitorTarget(
				(await store.get("overlay_monitor_target")) ??
					DEFAULT_SETTINGS_VALUES.overlay_monitor_target,
			),
			widget_position:
				(await store.get<WidgetPosition>("widget_position")) ??
				DEFAULT_SETTINGS_VALUES.widget_position,
			output_mode: normalizeOutputMode(await store.get("output_mode")),
			output_hit_enter: await readSettingValue(
				"output_hit_enter",
				DEFAULT_SETTINGS_VALUES.output_hit_enter,
				normalizeBooleanSetting,
			),
			output_clipboard_privacy_mode: await readSettingValue(
				"output_clipboard_privacy_mode",
				DEFAULT_SETTINGS_VALUES.output_clipboard_privacy_mode,
				normalizeBooleanSetting,
			),
			output_smart_paste_protection: await readSettingValue(
				"output_smart_paste_protection",
				DEFAULT_SETTINGS_VALUES.output_smart_paste_protection,
				normalizeBooleanSetting,
			),

			main_window_close_behavior: normalizeMainWindowCloseBehavior(
				await store.get("main_window_close_behavior"),
			),

			quiet_audio_gate_enabled:
				(await store.get<boolean>("quiet_audio_gate_enabled")) ??
				DEFAULT_SETTINGS_VALUES.quiet_audio_gate_enabled,
			quiet_audio_min_duration_secs:
				(await store.get<number>("quiet_audio_min_duration_secs")) ??
				DEFAULT_SETTINGS_VALUES.quiet_audio_min_duration_secs,
			quiet_audio_rms_dbfs_threshold:
				(await store.get<number>("quiet_audio_rms_dbfs_threshold")) ??
				DEFAULT_SETTINGS_VALUES.quiet_audio_rms_dbfs_threshold,
			quiet_audio_peak_dbfs_threshold:
				(await store.get<number>("quiet_audio_peak_dbfs_threshold")) ??
				DEFAULT_SETTINGS_VALUES.quiet_audio_peak_dbfs_threshold,
			quiet_audio_require_speech:
				(await store.get<boolean>("quiet_audio_require_speech")) ??
				DEFAULT_SETTINGS_VALUES.quiet_audio_require_speech,

			hot_mic_enabled:
				(await store.get<boolean>("hot_mic_enabled")) ??
				DEFAULT_SETTINGS_VALUES.hot_mic_enabled,
			hot_mic_pre_roll_ms:
				(await store.get<number>("hot_mic_pre_roll_ms")) ??
				DEFAULT_SETTINGS_VALUES.hot_mic_pre_roll_ms,
			mic_auto_recover_enabled:
				(await store.get<boolean>("mic_auto_recover_enabled")) ??
				DEFAULT_SETTINGS_VALUES.mic_auto_recover_enabled,

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
				(await store.get<boolean>("audio_downmix_to_mono")) ??
				DEFAULT_SETTINGS_VALUES.audio_downmix_to_mono,
			audio_resample_to_16khz:
				(await store.get<boolean>("audio_resample_to_16khz")) ??
				DEFAULT_SETTINGS_VALUES.audio_resample_to_16khz,
			audio_highpass_enabled:
				(await store.get<boolean>("audio_highpass_enabled")) ??
				DEFAULT_SETTINGS_VALUES.audio_highpass_enabled,
			audio_agc_enabled:
				(await store.get<boolean>("audio_agc_enabled")) ??
				DEFAULT_SETTINGS_VALUES.audio_agc_enabled,
			audio_noise_suppression_enabled:
				(await store.get<boolean>("audio_noise_suppression_enabled")) ??
				DEFAULT_SETTINGS_VALUES.audio_noise_suppression_enabled,

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
				(await store.get<boolean>("request_logs_privacy_mode")) ??
				DEFAULT_SETTINGS_VALUES.request_logs_privacy_mode,
			posthog_analytics_enabled: await readSettingValue(
				POSTHOG_ANALYTICS_ENABLED_KEY,
				DEFAULT_SETTINGS_VALUES.posthog_analytics_enabled,
				normalizeBooleanSetting,
			),
			telemetry_disclosure_acknowledged_at: await readSettingValue(
				TELEMETRY_DISCLOSURE_ACKNOWLEDGED_AT_KEY,
				DEFAULT_SETTINGS_VALUES.telemetry_disclosure_acknowledged_at,
				normalizeNonEmptyStringSetting,
			),
			telemetry_disclosure_version: await readSettingValue(
				TELEMETRY_DISCLOSURE_VERSION_KEY,
				DEFAULT_SETTINGS_VALUES.telemetry_disclosure_version,
				normalizeNonEmptyStringSetting,
			),

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
				(await store.get<string | null>("github_backup_gist_id")) ??
				DEFAULT_SETTINGS_VALUES.github_backup_gist_id,

			// ============================================================================
			// OCR (Active Window Context) settings
			// ============================================================================

			ocr_base_url:
				(await store.get<string | null>("ocr_base_url")) ??
				DEFAULT_SETTINGS_VALUES.ocr_base_url,
			ocr_model: await readSettingValue(
				"ocr_model",
				DEFAULT_SETTINGS_VALUES.ocr_model,
				normalizeNonEmptyStringSetting,
			),
			ocr_auth_mode: normalizeOcrAuthMode(await store.get("ocr_auth_mode")),
			ocr_prompt:
				(await store.get<string | null>("ocr_prompt")) ??
				DEFAULT_SETTINGS_VALUES.ocr_prompt,
			ocr_max_tokens:
				(await store.get<number | null>("ocr_max_tokens")) ??
				DEFAULT_SETTINGS_VALUES.ocr_max_tokens,
			ocr_temperature:
				(await store.get<number | null>("ocr_temperature")) ??
				DEFAULT_SETTINGS_VALUES.ocr_temperature,
			ocr_top_p:
				(await store.get<number | null>("ocr_top_p")) ??
				DEFAULT_SETTINGS_VALUES.ocr_top_p,
			ocr_request_timeout_ms:
				(await store.get<number | null>("ocr_request_timeout_ms")) ??
				DEFAULT_SETTINGS_VALUES.ocr_request_timeout_ms,
			ocr_context_max_chars:
				(await store.get<number | null>("ocr_context_max_chars")) ??
				DEFAULT_SETTINGS_VALUES.ocr_context_max_chars,

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
				DEFAULT_SETTINGS_VALUES.ocr_hallucination_protection,
			ocr_hallucination_threshold:
				(await store.get<number | null>("ocr_hallucination_threshold")) ??
				DEFAULT_SETTINGS_VALUES.ocr_hallucination_threshold,
			ocr_resize_max_dimension:
				(await store.get<number | null>("ocr_resize_max_dimension")) ??
				DEFAULT_SETTINGS_VALUES.ocr_resize_max_dimension,
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

	async resolveTelemetryDisclosure(params: {
		analyticsEnabled: boolean;
		acknowledgedAt?: string;
	}): Promise<void> {
		// Keep the first-run disclosure write on the shared backend patch path so
		// multi-window updates stay serialized and the stored version marker cannot
		// drift from the transport gate.
		await applySettingsPatch({
			patch: buildTelemetryDisclosureResolutionPatch({
				analyticsEnabled: params.analyticsEnabled,
				acknowledgedAt: params.acknowledgedAt,
			}),
		});
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
