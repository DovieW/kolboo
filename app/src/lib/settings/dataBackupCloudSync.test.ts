import { describe, expect, it, vi } from "vitest";
import type { LicenseState } from "../tauri/types";
import {
	clearGithubBackupToken,
	type DataBackupCloudSyncActionEffects,
	type DataBackupCloudSyncDependencies,
	exportSettingsBackupFromPath,
	getCloudSyncAccessState,
	importSettingsBackupFromPath,
	normalizeOptionalGistId,
	pullSettingsBackupFromGist,
	pushSettingsBackupToGist,
	runCloudSyncActionRequest,
	updateCloudSyncAutoPushSetting,
	updateCloudSyncEnabledSetting,
	updatePosthogAnalyticsEnabledSetting,
} from "./dataBackupCloudSync";
import type { CloudSyncUiState } from "./dataLifecycle";

const signedOutCommunityState: LicenseState = {
	tier: "community",
	status: "signed_out",
	user_id: null,
	email: null,
	org: null,
	expires_at: null,
	cached_at: "2026-05-21T00:00:00.000Z",
	last_validated_at: null,
	usage: {
		stt_seconds_used: 0,
		llm_tokens_used: 0,
		requests_today: 0,
	},
	limits: {
		stt_seconds_monthly: 0,
		llm_tokens_monthly: 0,
		requests_per_day: 0,
	},
	portal_available: false,
};

const signedInCommunityState: LicenseState = {
	...signedOutCommunityState,
	status: "active",
	user_id: "user-123",
	email: "person@example.com",
};

const personalState: LicenseState = {
	...signedInCommunityState,
	tier: "personal",
};

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
		telemetryDisclosureAcknowledgedAt: null,
		telemetryDisclosureVersion: null,
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
	it("describes which account states can use cloud sync", () => {
		expect(getCloudSyncAccessState(undefined)).toMatchObject({
			status: "loading",
			canUseCloudSync: false,
		});
		expect(getCloudSyncAccessState(signedOutCommunityState)).toMatchObject({
			status: "sign_in_required",
			canUseCloudSync: false,
		});
		expect(getCloudSyncAccessState(signedInCommunityState)).toMatchObject({
			status: "upgrade_required",
			canUseCloudSync: false,
		});
		expect(getCloudSyncAccessState(personalState)).toMatchObject({
			status: "included",
			canUseCloudSync: true,
		});
	});

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
			runCloudSyncActionRequest("push", deps, effects, personalState),
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
			runCloudSyncActionRequest("pull", deps, effects, personalState),
		).rejects.toThrow("offline");
		expect(effects.onCloudSyncStateRefresh).toHaveBeenCalledTimes(1);
		expect(effects.onSettingsChanged).not.toHaveBeenCalled();
		expect(effects.trackProductEvent).toHaveBeenCalledWith(
			"cloud_sync_action_failed",
			{ action: "pull", error_kind: "TypeError" },
		);
	});

	it("blocks cloud sync actions for Community/BYOK accounts", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			runCloudSyncActionRequest("push", deps, effects, signedInCommunityState),
		).resolves.toMatchObject({
			kind: "blocked_by_plan",
			accessStatus: "upgrade_required",
		});
		expect(deps.applySettingsPatch).not.toHaveBeenCalled();
		expect(effects.onCloudSyncStateRefresh).not.toHaveBeenCalled();
		expect(effects.onSettingsChanged).not.toHaveBeenCalled();
	});

	it("blocks turning cloud sync on before a paid account is available", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			updateCloudSyncEnabledSetting(
				true,
				deps,
				effects,
				signedOutCommunityState,
			),
		).resolves.toMatchObject({
			kind: "blocked_by_plan",
			accessStatus: "sign_in_required",
		});
		expect(deps.applySettingsPatch).not.toHaveBeenCalled();
	});

	it("still lets ineligible accounts turn existing sync flags back off", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			updateCloudSyncEnabledSetting(
				false,
				deps,
				effects,
				signedInCommunityState,
			),
		).resolves.toEqual({ kind: "cloud_sync_enabled_updated", enabled: false });
		await expect(
			updateCloudSyncAutoPushSetting(
				false,
				deps,
				effects,
				signedInCommunityState,
			),
		).resolves.toEqual({
			kind: "cloud_sync_auto_push_updated",
			enabled: false,
		});
		expect(deps.applySettingsPatch).toHaveBeenNthCalledWith(1, {
			cloud_sync_enabled: false,
		});
		expect(deps.applySettingsPatch).toHaveBeenNthCalledWith(2, {
			cloud_sync_auto_push: false,
		});
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

	it("refreshes cloud sync state without emitting a trailing opt-out event", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await expect(
			updatePosthogAnalyticsEnabledSetting(false, deps, effects),
		).resolves.toEqual({ kind: "posthog_analytics_updated", enabled: false });
		expect(deps.applySettingsPatch).toHaveBeenCalledWith({
			posthog_analytics_enabled: false,
		});
		expect(effects.onCloudSyncStateRefresh).toHaveBeenCalledTimes(1);
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.trackProductEvent).not.toHaveBeenCalled();
	});
});
