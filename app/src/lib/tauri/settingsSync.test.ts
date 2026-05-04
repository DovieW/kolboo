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
	it("classifies pipeline-affecting, secondary-window, both, and no-runtime changes", () => {
		expect(
			classifySettingsRuntimeEffects({ patch: { stt_provider: "groq" } }),
		).toMatchObject({
			needsPipelineSync: true,
			needsSettingsChangedEvent: false,
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
		});
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
