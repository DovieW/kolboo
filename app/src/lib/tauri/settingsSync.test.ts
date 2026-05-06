import { describe, expect, it, vi } from "vitest";
import {
	applySettingsRuntimeSyncPolicy,
	classifySettingsRuntimeEffects,
} from "./settingsSync";

function createAdapters() {
	return {
		invoke: vi.fn(async () => undefined),
		emitSettingsChanged: vi.fn(async () => undefined),
	};
}

describe("settings runtime sync policy", () => {
	it.each([
		"selected_mic_id",
		"rewrite_llm_enabled",
		"quick_replace_enabled",
		"cleanup_prompt_sections",
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
	])("classifies runtime-affecting mutation key %s", (key) => {
		// This table mirrors the settings mutation paths whose values are folded
		// into PipelineConfig or policy/license runtime state. It is intentionally
		// verbose so key drift breaks here instead of silently skipping sync.
		expect(
			classifySettingsRuntimeEffects({ patch: { [key]: true } }),
		).toMatchObject({
			needsPipelineSync: true,
		});
	});

	it.each([
		"accent_color",
		"overlay_mode",
		"overlay_show_detailed_loading",
		"overlay_monitor_target",
		"widget_position",
		"main_window_close_behavior",
		"settings_guide_state",
	])("classifies secondary-window mutation key %s", (key) => {
		expect(
			classifySettingsRuntimeEffects({ patch: { [key]: true } }),
		).toMatchObject({
			needsSettingsChangedEvent: true,
		});
	});

	it.each([
		"github_backup_gist_id",
		"max_saved_recordings",
		"request_logs_retention_mode",
		"request_logs_privacy_mode",
		"stats_retention_unit",
		"stats_retention_value",
		"groq_free_tier",
		"cerebras_free_tier",
		"cohere_free_tier",
		"assemblyai_free_tier",
		"speechmatics_free_tier",
	])("classifies non-runtime mutation key %s", (key) => {
		expect(
			classifySettingsRuntimeEffects({ patch: { [key]: true } }),
		).toMatchObject({
			needsPipelineSync: false,
			needsSettingsChangedEvent: false,
		});
	});

	it("classifies pipeline-affecting, secondary-window, both, and no-runtime changes", () => {
		expect(
      classifySettingsRuntimeEffects({ patch: { stt_provider: "groq" } }),
    ).toMatchObject({
      needsPipelineSync: true,
      needsSettingsChangedEvent: false,
      queryInvalidations: [{ queryKey: ["settings"], reason: "settings" }],
    });
		expect(
			classifySettingsRuntimeEffects({ patch: { overlay_mode: "always" } }),
		).toMatchObject({
			needsPipelineSync: false,
			needsSettingsChangedEvent: true,
		});
		expect(
			classifySettingsRuntimeEffects({
				patch: { rewrite_program_prompt_profiles: [] },
			}),
		).toMatchObject({
			needsPipelineSync: true,
			needsSettingsChangedEvent: true,
		});
		expect(
      classifySettingsRuntimeEffects({
        patch: { github_backup_gist_id: "gist" },
      }),
    ).toMatchObject({
      needsPipelineSync: false,
      needsSettingsChangedEvent: false,
      queryInvalidations: [{ queryKey: ["settings"], reason: "settings" }],
    });
	});

	it("centralizes query invalidation intent for policy and license changes", () => {
    expect(
      classifySettingsRuntimeEffects({ policyNormalized: true })
        .queryInvalidations,
    ).toEqual([
      { queryKey: ["policyState"], reason: "policy" },
      { queryKey: ["settings"], reason: "settings" },
    ]);

    expect(
      classifySettingsRuntimeEffects({ patch: { license_state: {} } })
        .queryInvalidations,
    ).toEqual([
      { queryKey: ["settings"], reason: "settings" },
      { queryKey: ["licenseState"], reason: "license" },
      { queryKey: ["licenseAuthContext"], reason: "license" },
    ]);
  });

	it("deduplicates pipeline sync and backend settings-change events for one patch batch", async () => {
		const adapters = createAdapters();

		const result = await applySettingsRuntimeSyncPolicy({
			patch: {
				stt_provider: "groq",
				stt_model: "whisper-large-v3",
				overlay_mode: "recording_only",
			},
			backendEventEmitted: true,
			invoke: adapters.invoke,
			emitSettingsChanged: adapters.emitSettingsChanged,
		});

		expect(result).toMatchObject({
			syncPerformed: true,
			eventEmitted: false,
		});
		expect(adapters.invoke).toHaveBeenCalledTimes(1);
		expect(adapters.invoke).toHaveBeenCalledWith("sync_pipeline_config");
		expect(adapters.emitSettingsChanged).not.toHaveBeenCalled();
	});

	it("emits one settings-change event for secondary-window changes when no backend event exists", async () => {
		const adapters = createAdapters();

		const result = await applySettingsRuntimeSyncPolicy({
			patch: { widget_position: "bottom-center" },
			backendEventEmitted: false,
			invoke: adapters.invoke,
			emitSettingsChanged: adapters.emitSettingsChanged,
		});

		expect(result).toMatchObject({
			syncPerformed: false,
			eventEmitted: true,
		});
		expect(adapters.invoke).not.toHaveBeenCalled();
		expect(adapters.emitSettingsChanged).toHaveBeenCalledTimes(1);
		expect(adapters.emitSettingsChanged).toHaveBeenCalledWith({
			widget_position: true,
		});
	});

	it("treats delete keys as changed settings", async () => {
		const adapters = createAdapters();

		const result = await applySettingsRuntimeSyncPolicy({
			deleteKeys: ["stt_model"],
			backendEventEmitted: true,
			invoke: adapters.invoke,
			emitSettingsChanged: adapters.emitSettingsChanged,
		});

		expect(result.syncPerformed).toBe(true);
		expect(adapters.invoke).toHaveBeenCalledTimes(1);
		expect(adapters.emitSettingsChanged).not.toHaveBeenCalled();
	});

	it("preserves policy/license metadata when policy handling has no backend patch event", async () => {
		const adapters = createAdapters();
		const policyViolations = [{ path: "llm_provider", reason: "managed" }];

		const result = await applySettingsRuntimeSyncPolicy({
			policyNormalized: true,
			policyViolations,
			backendEventEmitted: false,
			invoke: adapters.invoke,
			emitSettingsChanged: adapters.emitSettingsChanged,
		});

		expect(result).toMatchObject({
			syncPerformed: true,
			eventEmitted: true,
		});
		expect(adapters.invoke).toHaveBeenCalledWith("sync_pipeline_config");
		expect(adapters.emitSettingsChanged).toHaveBeenCalledWith({
			policy_normalized: true,
			policy_constraints_applied: true,
			policy_violations: policyViolations,
		});
	});

	it("syncs runtime config and emits one event for API key changes", async () => {
		const adapters = createAdapters();

		const result = await applySettingsRuntimeSyncPolicy({
			apiKeysChanged: true,
			backendEventEmitted: false,
			invoke: adapters.invoke,
			emitSettingsChanged: adapters.emitSettingsChanged,
		});

		expect(result).toMatchObject({
			syncPerformed: true,
			eventEmitted: true,
		});
		expect(adapters.invoke).toHaveBeenCalledTimes(1);
		expect(adapters.invoke).toHaveBeenCalledWith("sync_pipeline_config");
		expect(adapters.emitSettingsChanged).toHaveBeenCalledTimes(1);
		expect(adapters.emitSettingsChanged).toHaveBeenCalledWith({
			api_keys_changed: true,
		});
	});
});
