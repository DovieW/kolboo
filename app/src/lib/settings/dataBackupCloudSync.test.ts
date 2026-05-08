import { describe, expect, it, vi } from "vitest";
import {
	clearGithubBackupToken,
	type DataBackupCloudSyncActionEffects,
	type DataBackupCloudSyncDependencies,
	exportSettingsBackupFromPath,
	importSettingsBackupFromPath,
	normalizeOptionalGistId,
	pullSettingsBackupFromGist,
	pushSettingsBackupToGist,
	runCloudSyncActionRequest,
	updatePosthogAnalyticsEnabledSetting,
} from "./dataBackupCloudSync";
import type { CloudSyncUiState } from "./dataLifecycle";

function createDeps(
	overrides: Partial<DataBackupCloudSyncDependencies> = {},
): DataBackupCloudSyncDependencies {
	const cloudSyncState: CloudSyncUiState = {
		enabled: false,
		autoPush: true,
		lastPushedAt: null,
		lastPulledAt: null,
		lastError: null,
		remoteRevision: null,
		posthogAnalyticsEnabled: true,
	};

	return {
		exportSettingsBackupToFile: vi.fn(async () => undefined),
		importSettingsBackupFromFile: vi.fn(async () => undefined),
		importSettingsBackupJson: vi.fn(async () => undefined),
		githubBackupHasToken: vi.fn(async () => false),
		githubBackupSetToken: vi.fn(async () => undefined),
		githubBackupClearToken: vi.fn(async () => undefined),
		githubBackupPushToGist: vi.fn(async () => "gist-created"),
		githubBackupPullFromGist: vi.fn(async () => '{"settings":true}'),
		updateGithubBackupGistId: vi.fn(async () => undefined),
		readCloudSyncUiState: vi.fn(async () => cloudSyncState),
		applySettingsPatch: vi.fn(async () => undefined),
		...overrides,
	};
}

function createEffects(
	overrides: Partial<DataBackupCloudSyncActionEffects> = {},
): DataBackupCloudSyncActionEffects {
	return {
		onSettingsChanged: vi.fn(async () => undefined),
		onImportedSettingsApplied: vi.fn(async () => undefined),
		onCloudSyncStateRefresh: vi.fn(async () => undefined),
		onGithubTokenStateRefresh: vi.fn(async () => undefined),
		reRegisterShortcuts: vi.fn(async () => undefined),
		trackProductEvent: vi.fn(async () => undefined),
		...overrides,
	};
}

describe("Data backup/cloud-sync orchestration helpers", () => {
	it("normalizes optional gist ids defensively", () => {
		expect(normalizeOptionalGistId("  gist-123  ")).toBe("gist-123");
		expect(normalizeOptionalGistId("   ")).toBeNull();
		expect(normalizeOptionalGistId(null)).toBeNull();
	});

	it("treats a missing export path as a cancelled backup", async () => {
		const deps = createDeps();

		await expect(exportSettingsBackupFromPath(null, deps)).resolves.toEqual({
			kind: "cancelled",
		});
		expect(deps.exportSettingsBackupToFile).not.toHaveBeenCalled();
	});

	it("imports a settings backup, re-registers shortcuts, and invalidates imported settings", async () => {
		const deps = createDeps();
		const events: string[] = [];
		const effects = createEffects({
			reRegisterShortcuts: vi.fn(async () => {
				events.push("reRegisterShortcuts");
			}),
			onImportedSettingsApplied: vi.fn(async () => {
				events.push("onImportedSettingsApplied");
			}),
		});

		await expect(
			importSettingsBackupFromPath("C:/tmp/backup.json", deps, effects),
		).resolves.toEqual({
			kind: "imported",
			path: "C:/tmp/backup.json",
		});
		expect(deps.importSettingsBackupFromFile).toHaveBeenCalledWith({
			path: "C:/tmp/backup.json",
		});
		expect(events).toEqual([
			"reRegisterShortcuts",
			"onImportedSettingsApplied",
		]);
	});

	it("pushes settings to a gist, saves the returned gist id, and invalidates settings", async () => {
		const deps = createDeps({
			githubBackupPushToGist: vi.fn(async (params) => {
				expect(params).toEqual({ gistId: null });
				return "gist-created";
			}),
		});
		const effects = createEffects();

		await expect(
			pushSettingsBackupToGist("   ", deps, effects),
		).resolves.toEqual({ kind: "pushed", gistId: "gist-created" });
		expect(deps.updateGithubBackupGistId).toHaveBeenCalledWith("gist-created");
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
	});

	it("pulls settings from a gist, re-registers shortcuts, and invalidates imported settings", async () => {
		const deps = createDeps({
			githubBackupPullFromGist: vi.fn(async ({ gistId }) => {
				expect(gistId).toBe("gist-123");
				return '{"imported":true}';
			}),
		});
		const effects = createEffects();

		await expect(
			pullSettingsBackupFromGist(" gist-123 ", deps, effects),
		).resolves.toEqual({ kind: "pulled", gistId: "gist-123" });
		expect(deps.importSettingsBackupJson).toHaveBeenCalledWith({
			json: '{"imported":true}',
		});
		expect(effects.reRegisterShortcuts).toHaveBeenCalledTimes(1);
		expect(effects.onImportedSettingsApplied).toHaveBeenCalledTimes(1);
	});

	it("refreshes cloud-sync state and tracks success when a cloud sync action completes", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			runCloudSyncActionRequest("push", deps, effects),
		).resolves.toEqual({ kind: "completed", action: "push" });
		expect(deps.applySettingsPatch).toHaveBeenCalledWith({
			__cloud_sync_action: "push",
		});
		expect(effects.onCloudSyncStateRefresh).toHaveBeenCalledTimes(1);
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.trackProductEvent).toHaveBeenCalledWith(
			"cloud_sync_action_succeeded",
			{ action: "push" },
		);
	});

	it("still refreshes cloud-sync state and tracks failure when a cloud sync action errors", async () => {
		const deps = createDeps({
			applySettingsPatch: vi.fn(async () => {
				throw new TypeError("offline");
			}),
		});
		const effects = createEffects();

		await expect(
			runCloudSyncActionRequest("pull", deps, effects),
		).rejects.toThrow("offline");
		expect(effects.onCloudSyncStateRefresh).toHaveBeenCalledTimes(1);
		expect(effects.onSettingsChanged).not.toHaveBeenCalled();
		expect(effects.trackProductEvent).toHaveBeenCalledWith(
			"cloud_sync_action_failed",
			{ action: "pull", error_kind: "TypeError" },
		);
	});

	it("refreshes github token state after clearing secure storage", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(clearGithubBackupToken(deps, effects)).resolves.toEqual({
			kind: "cleared",
		});
		expect(effects.onGithubTokenStateRefresh).toHaveBeenCalledTimes(1);
	});

	it("refreshes cloud sync state and records analytics opt-in events", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			updatePosthogAnalyticsEnabledSetting(true, deps, effects),
		).resolves.toEqual({ kind: "posthog_analytics_updated", enabled: true });
		expect(deps.applySettingsPatch).toHaveBeenCalledWith({
			posthog_analytics_enabled: true,
		});
		expect(effects.onCloudSyncStateRefresh).toHaveBeenCalledTimes(1);
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.trackProductEvent).toHaveBeenCalledWith(
			"analytics_opted_in",
			{ surface: "settings" },
		);
	});
});
