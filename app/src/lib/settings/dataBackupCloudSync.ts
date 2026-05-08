import { useMutation, useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { backupAPI, tauriAPI } from "../tauri";
import { trackProductEvent as trackProductEventDefault } from "../telemetry/posthog";
import { type CloudSyncUiState, readCloudSyncUiState } from "./dataLifecycle";

type MaybePromise<T> = T | Promise<T>;

export type CloudSyncAction = "push" | "pull";

export interface DataBackupCloudSyncDependencies {
	exportSettingsBackupToFile: (params: { path: string }) => Promise<void>;
	importSettingsBackupFromFile: (params: { path: string }) => Promise<void>;
	importSettingsBackupJson: (params: { json: string }) => Promise<void>;
	githubBackupHasToken: () => Promise<boolean>;
	githubBackupSetToken: (params: { token: string }) => Promise<void>;
	githubBackupClearToken: () => Promise<void>;
	githubBackupPushToGist: (params: {
		gistId?: string | null;
	}) => Promise<string>;
	githubBackupPullFromGist: (params: { gistId: string }) => Promise<string>;
	updateGithubBackupGistId: (gistId: string | null) => Promise<void>;
	readCloudSyncUiState: () => Promise<CloudSyncUiState>;
	applySettingsPatch: (patch: Record<string, unknown>) => Promise<void>;
}

export interface DataBackupCloudSyncActionEffects {
	onSettingsChanged: () => MaybePromise<void>;
	onImportedSettingsApplied: () => MaybePromise<void>;
	onCloudSyncStateRefresh: () => MaybePromise<void>;
	onGithubTokenStateRefresh: () => MaybePromise<void>;
	reRegisterShortcuts: () => Promise<void>;
	trackProductEvent?: (
		event: string,
		properties: Record<string, unknown>,
	) => MaybePromise<void>;
}

type UseDataBackupCloudSyncOrchestrationArgs = {
	gistIdFromSettings: string;
	effects: Pick<
		DataBackupCloudSyncActionEffects,
		"onSettingsChanged" | "onImportedSettingsApplied" | "reRegisterShortcuts"
	> & {
		trackProductEvent?: DataBackupCloudSyncActionEffects["trackProductEvent"];
	};
	deps?: DataBackupCloudSyncDependencies;
};

export type SettingsBackupFileResult =
	| { kind: "cancelled" }
	| { kind: "exported" | "imported"; path: string };

export type GithubTokenMutationResult = { kind: "saved" | "cleared" };

export type GithubBackupGistResult =
	| { kind: "saved"; gistId: string | null }
	| { kind: "pushed" | "pulled"; gistId: string };

export type CloudSyncMutationResult =
	| { kind: "completed"; action: CloudSyncAction }
	| { kind: "cloud_sync_enabled_updated"; enabled: boolean }
	| { kind: "cloud_sync_auto_push_updated"; enabled: boolean }
	| { kind: "posthog_analytics_updated"; enabled: boolean };

// Keep concrete Tauri/backup adapters localized here so the feature hook can be
// tested with small mocked dependencies instead of real dialogs, GitHub, or
// cloud-sync endpoints.
const defaultDataBackupCloudSyncDependencies: DataBackupCloudSyncDependencies =
	{
		exportSettingsBackupToFile: (params) =>
			backupAPI.exportSettingsBackupToFile(params),
		importSettingsBackupFromFile: (params) =>
			backupAPI.importSettingsBackupFromFile(params),
		importSettingsBackupJson: (params) =>
			backupAPI.importSettingsBackupJson(params),
		githubBackupHasToken: () => backupAPI.githubBackupHasToken(),
		githubBackupSetToken: (params) => backupAPI.githubBackupSetToken(params),
		githubBackupClearToken: () => backupAPI.githubBackupClearToken(),
		githubBackupPushToGist: (params) =>
			backupAPI.githubBackupPushToGist(params),
		githubBackupPullFromGist: (params) =>
			backupAPI.githubBackupPullFromGist(params),
		updateGithubBackupGistId: (gistId) =>
			tauriAPI.updateGithubBackupGistId(gistId),
		readCloudSyncUiState: () => readCloudSyncUiState(),
		applySettingsPatch: async (patch) => {
			await invoke("settings_apply_patch", {
				patch,
				deleteKeys: [],
			});
		},
	};

function trimOrNull(value: string | null | undefined): string | null {
	const trimmed = (value ?? "").trim();
	return trimmed.length > 0 ? trimmed : null;
}

function errorKind(error: unknown): string {
	return error instanceof Error ? error.name : typeof error;
}

export function normalizeOptionalGistId(
	value: string | null | undefined,
): string | null {
	return trimOrNull(value);
}

export function requireGistId(value: string | null | undefined): string {
	const gistId = normalizeOptionalGistId(value);
	if (!gistId) {
		throw new Error("Missing gist id");
	}

	return gistId;
}

export async function exportSettingsBackupFromPath(
	path: string | null,
	deps: DataBackupCloudSyncDependencies,
): Promise<SettingsBackupFileResult> {
	if (!path) {
		return { kind: "cancelled" };
	}

	await deps.exportSettingsBackupToFile({ path });
	return { kind: "exported", path };
}

export async function importSettingsBackupFromPath(
	path: string | null,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<SettingsBackupFileResult> {
	if (!path) {
		return { kind: "cancelled" };
	}

	await deps.importSettingsBackupFromFile({ path });
	await effects.reRegisterShortcuts();
	await effects.onImportedSettingsApplied();

	return { kind: "imported", path };
}

export async function saveGithubBackupToken(
	token: string,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<GithubTokenMutationResult> {
	await deps.githubBackupSetToken({ token });
	await effects.onGithubTokenStateRefresh();

	return { kind: "saved" };
}

export async function clearGithubBackupToken(
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<GithubTokenMutationResult> {
	await deps.githubBackupClearToken();
	await effects.onGithubTokenStateRefresh();

	return { kind: "cleared" };
}

export async function saveGithubBackupGistId(
	gistIdDraft: string,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<GithubBackupGistResult> {
	const gistId = normalizeOptionalGistId(gistIdDraft);
	await deps.updateGithubBackupGistId(gistId);
	await effects.onSettingsChanged();

	return { kind: "saved", gistId };
}

export async function pushSettingsBackupToGist(
	gistIdDraft: string,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<GithubBackupGistResult> {
	const nextGistId = await deps.githubBackupPushToGist({
		gistId: normalizeOptionalGistId(gistIdDraft),
	});

	await deps.updateGithubBackupGistId(nextGistId);
	await effects.onSettingsChanged();

	return { kind: "pushed", gistId: nextGistId };
}

export async function pullSettingsBackupFromGist(
	gistIdDraft: string,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<GithubBackupGistResult> {
	const gistId = requireGistId(gistIdDraft);
	const json = await deps.githubBackupPullFromGist({ gistId });

	await deps.importSettingsBackupJson({ json });
	await effects.reRegisterShortcuts();
	await effects.onImportedSettingsApplied();

	return { kind: "pulled", gistId };
}

export async function runCloudSyncActionRequest(
	action: CloudSyncAction,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<CloudSyncMutationResult> {
	try {
		await deps.applySettingsPatch({ __cloud_sync_action: action });
		await effects.onCloudSyncStateRefresh();
		await effects.onSettingsChanged();
		await effects.trackProductEvent?.("cloud_sync_action_succeeded", {
			action,
		});

		return { kind: "completed", action };
	} catch (error) {
		await effects.onCloudSyncStateRefresh();
		await effects.trackProductEvent?.("cloud_sync_action_failed", {
			action,
			error_kind: errorKind(error),
		});
		throw error;
	}
}

export async function updateCloudSyncEnabledSetting(
	enabled: boolean,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<CloudSyncMutationResult> {
	await deps.applySettingsPatch({ cloud_sync_enabled: enabled });
	await effects.onCloudSyncStateRefresh();
	await effects.onSettingsChanged();
	await effects.trackProductEvent?.("cloud_sync_enabled_changed", {
		enabled,
	});

	return { kind: "cloud_sync_enabled_updated", enabled };
}

export async function updateCloudSyncAutoPushSetting(
	enabled: boolean,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<CloudSyncMutationResult> {
	await deps.applySettingsPatch({ cloud_sync_auto_push: enabled });
	await effects.onCloudSyncStateRefresh();
	await effects.onSettingsChanged();
	await effects.trackProductEvent?.("cloud_sync_auto_push_changed", {
		enabled,
	});

	return { kind: "cloud_sync_auto_push_updated", enabled };
}

export async function updatePosthogAnalyticsEnabledSetting(
	enabled: boolean,
	deps: DataBackupCloudSyncDependencies,
	effects: DataBackupCloudSyncActionEffects,
): Promise<CloudSyncMutationResult> {
	await deps.applySettingsPatch({ posthog_analytics_enabled: enabled });
	await effects.onCloudSyncStateRefresh();
	await effects.onSettingsChanged();

	if (enabled) {
		await effects.trackProductEvent?.("analytics_opted_in", {
			surface: "settings",
		});
	}

	return { kind: "posthog_analytics_updated", enabled };
}

export function useDataBackupCloudSyncOrchestration({
	gistIdFromSettings,
	effects,
	deps = defaultDataBackupCloudSyncDependencies,
}: UseDataBackupCloudSyncOrchestrationArgs) {
	// The DataSettings adapter still owns file dialogs, notifications, and the
	// destructive sections. This hook only owns backup/cloud-sync draft state,
	// query refreshes, and mutation plumbing.
	const [githubTokenModalOpen, setGithubTokenModalOpen] = useState(false);
	const [githubTokenDraft, setGithubTokenDraft] = useState("");
	const [gistIdDraft, setGistIdDraft] = useState(gistIdFromSettings);

	useEffect(() => {
		setGistIdDraft(gistIdFromSettings);
	}, [gistIdFromSettings]);

	const githubBackupHasToken = useQuery({
		queryKey: ["githubBackupHasToken"],
		queryFn: () => deps.githubBackupHasToken(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});

	const cloudSyncState = useQuery({
		queryKey: ["cloudSyncUiState"],
		queryFn: () => deps.readCloudSyncUiState(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});

	const actionEffects: DataBackupCloudSyncActionEffects = {
		onSettingsChanged: effects.onSettingsChanged,
		onImportedSettingsApplied: effects.onImportedSettingsApplied,
		// Hide react-query's result envelope from the orchestration seam so the
		// effect contract stays about "refresh happened" rather than query internals.
		onCloudSyncStateRefresh: async () => {
			await cloudSyncState.refetch();
		},
		onGithubTokenStateRefresh: async () => {
			await githubBackupHasToken.refetch();
		},
		reRegisterShortcuts: effects.reRegisterShortcuts,
		trackProductEvent: effects.trackProductEvent ?? trackProductEventDefault,
	};

	const exportSettingsBackup = useMutation({
		mutationFn: (path: string | null) =>
			exportSettingsBackupFromPath(path, deps),
	});

	const importSettingsBackup = useMutation({
		mutationFn: (path: string | null) =>
			importSettingsBackupFromPath(path, deps, actionEffects),
	});

	const setGithubToken = useMutation({
		mutationFn: (token: string) =>
			saveGithubBackupToken(token, deps, actionEffects),
		onSuccess: () => {
			setGithubTokenDraft("");
			setGithubTokenModalOpen(false);
		},
	});

	const clearGithubToken = useMutation({
		mutationFn: () => clearGithubBackupToken(deps, actionEffects),
	});

	const saveGistId = useMutation({
		mutationFn: (nextGistIdDraft: string) =>
			saveGithubBackupGistId(nextGistIdDraft, deps, actionEffects),
		onSuccess: (result) => {
			setGistIdDraft(result.gistId ?? "");
		},
	});

	const pushToGist = useMutation({
		mutationFn: (nextGistIdDraft: string) =>
			pushSettingsBackupToGist(nextGistIdDraft, deps, actionEffects),
		onSuccess: (result) => {
			setGistIdDraft(result.gistId ?? "");
		},
	});

	const pullFromGist = useMutation({
		mutationFn: (nextGistIdDraft: string) =>
			pullSettingsBackupFromGist(nextGistIdDraft, deps, actionEffects),
	});

	const runCloudSyncAction = useMutation({
		mutationFn: (action: CloudSyncAction) =>
			runCloudSyncActionRequest(action, deps, actionEffects),
	});

	const updateCloudSyncEnabled = useMutation({
		mutationFn: (enabled: boolean) =>
			updateCloudSyncEnabledSetting(enabled, deps, actionEffects),
	});

	const updateCloudSyncAutoPush = useMutation({
		mutationFn: (enabled: boolean) =>
			updateCloudSyncAutoPushSetting(enabled, deps, actionEffects),
	});

	const updatePosthogAnalyticsEnabled = useMutation({
		mutationFn: (enabled: boolean) =>
			updatePosthogAnalyticsEnabledSetting(enabled, deps, actionEffects),
	});

	return {
		githubTokenModalOpen,
		setGithubTokenModalOpen,
		githubTokenDraft,
		setGithubTokenDraft,
		gistIdDraft,
		setGistIdDraft,
		githubBackupHasToken,
		cloudSyncState,
		exportSettingsBackup,
		importSettingsBackup,
		setGithubToken,
		clearGithubToken,
		saveGistId,
		pushToGist,
		pullFromGist,
		runCloudSyncAction,
		updateCloudSyncEnabled,
		updateCloudSyncAutoPush,
		updatePosthogAnalyticsEnabled,
	};
}
