import type {
	PolicyConstraintViolation,
	SettingsChangedPayload,
} from "./types";

export type RuntimeSyncReason =
	| "pipeline-setting"
	| "secondary-window-setting"
	| "api-keys"
	| "policy-normalized"
	| "policy-constraints";

export type RuntimeSyncEffects = {
  needsPipelineSync: boolean;
  needsSettingsChangedEvent: boolean;
  reasons: RuntimeSyncReason[];
  eventPayload: SettingsChangedPayload;
  queryInvalidations: SettingsQueryInvalidation[];
};

export type SettingsQueryInvalidationReason = "settings" | "policy" | "license";

export type SettingsQueryInvalidation = {
  queryKey: readonly unknown[];
  reason: SettingsQueryInvalidationReason;
};

export type RuntimeSyncPolicyResult = RuntimeSyncEffects & {
	syncPerformed: boolean;
	eventEmitted: boolean;
};

export type RuntimeSyncPolicyParams = {
	patch?: Record<string, unknown>;
	deleteKeys?: string[];
	backendEventEmitted?: boolean;
	apiKeysChanged?: boolean;
	policyNormalized?: boolean;
	policyViolations?: PolicyConstraintViolation[];
	invoke: (command: string) => Promise<unknown>;
	emitSettingsChanged: (payload: SettingsChangedPayload) => Promise<unknown>;
};

const PIPELINE_SETTING_KEYS = new Set([
	"selected_mic_id",
	"rewrite_llm_enabled",
	"quick_replace_enabled",
	"cleanup_prompt_sections",
	"rewrite_program_prompt_profiles",
	"stt_provider",
	"stt_model",
	"stt_language",
	"stt_transcription_prompt",
	"stt_live_output",
	"stt_simulated_streaming",
	"stt_timeout_seconds",
	"whisper_server_base_url",
	"ollama_url",
	"local_whisper_model_id",
	"local_whisper_load_mode",
	"proxy_settings",
	"llm_provider",
	"llm_model",
	"quick_ask_provider",
	"quick_ask_model",
	"quick_ask_system_prompt",
	"quick_ask_dismiss_mode",
	"quick_ask_include_selected_text",
	"quick_ask_conversation_history_enabled",
	"quick_ask_conversation_history_count",
	"quick_ask_openai_reasoning_effort",
	"quick_ask_anthropic_thinking_budget",
	"quick_ask_gemini_thinking_budget",
	"quick_ask_gemini_thinking_level",
	"openai_reasoning_effort",
	"anthropic_thinking_budget",
	"gemini_thinking_budget",
	"gemini_thinking_level",
	"playing_audio_handling",
	"output_mode",
	"output_hit_enter",
	"quiet_audio_gate_enabled",
	"quiet_audio_min_duration_secs",
	"quiet_audio_rms_dbfs_threshold",
	"quiet_audio_peak_dbfs_threshold",
	"quiet_audio_require_speech",
	"hot_mic_enabled",
	"hot_mic_pre_roll_ms",
	"mic_auto_recover_enabled",
	"noise_gate_threshold_dbfs",
	"noise_gate_strength",
	"audio_downmix_to_mono",
	"audio_resample_to_16khz",
	"audio_highpass_enabled",
	"audio_agc_enabled",
	"audio_noise_suppression_enabled",
	"policy_state",
	"license_state",
	"token_exchange_trigger_set",
	"ocr_base_url",
	"ocr_model",
	"ocr_auth_mode",
	"ocr_prompt",
	"ocr_max_tokens",
	"ocr_temperature",
	"ocr_top_p",
	"ocr_request_timeout_ms",
	"ocr_context_max_chars",
	"rewrite_active_window_ocr_mode",
	"quick_replace_active_window_ocr_mode",
	"quick_ask_active_window_ocr_mode",
	"ocr_auto_capture_timing",
	"ocr_hallucination_protection",
	"ocr_hallucination_threshold",
	"ocr_resize_max_dimension",
	"ocr_resize_filter",
]);

const SECONDARY_WINDOW_SETTING_KEYS = new Set([
	"accent_color",
	"overlay_mode",
	"overlay_show_detailed_loading",
	"overlay_monitor_target",
	"widget_position",
	"main_window_close_behavior",
	"settings_guide_state",
	"rewrite_program_prompt_profiles",
]);

function changedKeys(params: {
	patch?: Record<string, unknown>;
	deleteKeys?: string[];
}): Set<string> {
	return new Set([
		...Object.keys(params.patch ?? {}),
		...(params.deleteKeys ?? []),
	]);
}

function buildSettingsChangedPayload(params: {
	keys: Set<string>;
	apiKeysChanged?: boolean;
	policyNormalized?: boolean;
	policyViolations?: PolicyConstraintViolation[];
}): SettingsChangedPayload {
	const payload: SettingsChangedPayload = {};

	for (const key of params.keys) {
		if (SECONDARY_WINDOW_SETTING_KEYS.has(key)) {
			payload[key] = true;
		}
	}

	if (params.apiKeysChanged) {
		payload.api_keys_changed = true;
	}

	if (params.policyNormalized) {
		payload.policy_normalized = true;
	}

	const policyViolations = params.policyViolations ?? [];
	if (policyViolations.length > 0) {
		payload.policy_constraints_applied = true;
		payload.policy_violations = policyViolations;
	}

	return payload;
}

function addQueryInvalidation(
  invalidations: Map<string, SettingsQueryInvalidation>,
  queryKey: readonly unknown[],
  reason: SettingsQueryInvalidationReason,
) {
  // Query invalidation is part of the settings-mutation Interface, so keep
  // dedupe here rather than making each mutation caller remember which query
  // keys overlap. The string key is only for local set membership.
  invalidations.set(JSON.stringify(queryKey), { queryKey, reason });
}

function buildQueryInvalidations(params: {
  keys: Set<string>;
  apiKeysChanged?: boolean;
  policyNormalized?: boolean;
  policyViolations?: PolicyConstraintViolation[];
}): SettingsQueryInvalidation[] {
  const invalidations = new Map<string, SettingsQueryInvalidation>();
  const hasChanges = params.keys.size > 0;
  const hasPolicyViolations = (params.policyViolations?.length ?? 0) > 0;
  const hasPolicyStateChange =
    params.keys.has("policy_state") ||
    params.keys.has("token_exchange_trigger_set") ||
    Boolean(params.policyNormalized) ||
    hasPolicyViolations;
  const hasLicenseStateChange = params.keys.has("license_state");

  if (hasChanges || params.apiKeysChanged) {
    addQueryInvalidation(invalidations, ["settings"], "settings");
  }

  if (hasPolicyStateChange) {
    addQueryInvalidation(invalidations, ["policyState"], "policy");
    addQueryInvalidation(invalidations, ["settings"], "settings");
  }

  if (hasLicenseStateChange) {
    addQueryInvalidation(invalidations, ["licenseState"], "license");
    addQueryInvalidation(invalidations, ["licenseAuthContext"], "license");
    addQueryInvalidation(invalidations, ["settings"], "settings");
  }

  return [...invalidations.values()];
}

export function classifySettingsRuntimeEffects(params: {
	patch?: Record<string, unknown>;
	deleteKeys?: string[];
	apiKeysChanged?: boolean;
	policyNormalized?: boolean;
	policyViolations?: PolicyConstraintViolation[];
}): RuntimeSyncEffects {
	const keys = changedKeys(params);
	const reasons = new Set<RuntimeSyncReason>();
	const hasPipelineSetting = [...keys].some((key) =>
		PIPELINE_SETTING_KEYS.has(key),
	);
	const hasSecondaryWindowSetting = [...keys].some((key) =>
		SECONDARY_WINDOW_SETTING_KEYS.has(key),
	);
	const hasPolicyViolations = (params.policyViolations?.length ?? 0) > 0;

	if (hasPipelineSetting) reasons.add("pipeline-setting");
	if (hasSecondaryWindowSetting) reasons.add("secondary-window-setting");
	if (params.apiKeysChanged) reasons.add("api-keys");
	if (params.policyNormalized) reasons.add("policy-normalized");
	if (hasPolicyViolations) reasons.add("policy-constraints");

	const needsPipelineSync =
		hasPipelineSetting ||
		Boolean(params.apiKeysChanged) ||
		Boolean(params.policyNormalized) ||
		hasPolicyViolations;
	const needsSettingsChangedEvent =
		hasSecondaryWindowSetting ||
		Boolean(params.apiKeysChanged) ||
		Boolean(params.policyNormalized) ||
		hasPolicyViolations;

	return {
    needsPipelineSync,
    needsSettingsChangedEvent,
    reasons: [...reasons],
    eventPayload: buildSettingsChangedPayload({
      keys,
      apiKeysChanged: params.apiKeysChanged,
      policyNormalized: params.policyNormalized,
      policyViolations: params.policyViolations,
    }),
    queryInvalidations: buildQueryInvalidations({
      keys,
      apiKeysChanged: params.apiKeysChanged,
      policyNormalized: params.policyNormalized,
      policyViolations: params.policyViolations,
    }),
  };
}

export async function applySettingsRuntimeSyncPolicy(
	params: RuntimeSyncPolicyParams,
): Promise<RuntimeSyncPolicyResult> {
	const effects = classifySettingsRuntimeEffects(params);

	let syncPerformed = false;
	if (effects.needsPipelineSync) {
		await params.invoke("sync_pipeline_config");
		syncPerformed = true;
	}

	let eventEmitted = false;
	if (effects.needsSettingsChangedEvent && !params.backendEventEmitted) {
		await params.emitSettingsChanged(effects.eventPayload);
		eventEmitted = true;
	}

	return {
		...effects,
		syncPerformed,
		eventEmitted,
	};
}
