import { Tooltip } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
	BarChart2,
	FileText,
	Key,
	MessageSquare,
	RotateCcw,
	Skull,
	Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { API_KEY_STORE_KEYS } from "../../lib/apiKeys";
import { formatErrorMessage } from "../../lib/formatError";
import {
	useDataStorageSummary,
	useRecordingsStats,
	useSettings,
	useUpdateMaxSavedRecordings,
	useUpdateTranscriptionRetentionDeleteRecordings,
} from "../../lib/queries";
import {
	buildDataStorageBreakdown,
	type RequestLogsRetentionMode,
	type RetentionMode,
	type RetentionUnit,
	readCloudSyncUiState,
	retentionDaysFromUnitValue,
	summarizeRecordingsStorage,
} from "../../lib/settings/dataLifecycle";
import {
	backupAPI,
	dataAPI,
	logsAPI,
	type RewriteProgramPromptProfile,
	recordingsAPI,
	type TranscriptionRetentionUnit,
	tauriAPI,
} from "../../lib/tauri";
import { trackProductEvent } from "../../lib/telemetry/posthog";
import {
	type DangerZoneAction,
	DataBackupSection,
	DataCloudSyncSection,
	DataDangerConfirmModal,
	type DataDangerDialogState,
	DataDangerZoneSection,
	DataGithubTokenModal,
	DataRetentionSection,
} from "./data";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

export function DataSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const { data: settings } = useSettings();

	const queryClient = useQueryClient();
	const invalidateSettingsQuery = () => {
		void queryClient.invalidateQueries({ queryKey: ["settings"] });
	};
	const invalidateStorageQueries = () => {
		void queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
		void queryClient.invalidateQueries({ queryKey: ["dataStorageSummary"] });
	};
	const invalidateImportedSettingsQueries = () => {
		invalidateSettingsQuery();
		invalidateStorageQueries();
	};
	const invalidateDangerZoneQueries = () => {
		invalidateImportedSettingsQueries();
		void queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
		void queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		void queryClient.invalidateQueries({ queryKey: ["history"] });
		void queryClient.invalidateQueries({ queryKey: ["apiKeysSavedCount"] });
	};
	const reRegisterShortcuts = async () => {
		await tauriAPI.unregisterShortcuts();
		await tauriAPI.registerShortcuts();
	};

	const updateRequestLogsRetention = useMutation({
		mutationFn: (params: {
			mode: RequestLogsRetentionMode;
			amount: number;
			days: number;
		}) => tauriAPI.updateRequestLogsRetention(params),
		onSuccess: () => {
			invalidateSettingsQuery();
			void queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		},
	});

	const updateStatsRetention = useMutation({
		mutationFn: (params: { unit: TranscriptionRetentionUnit; value: number }) =>
			tauriAPI.updateStatsRetention(params),
		onSuccess: () => {
			invalidateSettingsQuery();
		},
	});

	const updateMaxSavedRecordings = useUpdateMaxSavedRecordings();
	const updateTranscriptionRetentionDeleteRecordings =
		useUpdateTranscriptionRetentionDeleteRecordings();

	const recordingsStats = useRecordingsStats();
	const dataStorageSummary = useDataStorageSummary();

	const apiKeysSavedCount = useQuery({
		queryKey: ["apiKeysSavedCount"],
		queryFn: async () => {
			const results = await Promise.all(
				API_KEY_STORE_KEYS.map(async (key) => {
					try {
						return await tauriAPI.hasApiKey(key);
					} catch {
						return false;
					}
				}),
			);
			return results.filter(Boolean).length;
		},
		staleTime: 0,
		refetchOnWindowFocus: true,
		refetchInterval: 10000,
	});

	// ---------------------------------------------------------------------------
	// Settings backup (export/import) + GitHub Gist backup
	// ---------------------------------------------------------------------------

	const githubBackupHasToken = useQuery({
		queryKey: ["githubBackupHasToken"],
		queryFn: () => backupAPI.githubBackupHasToken(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});

	const [githubTokenModalOpen, setGithubTokenModalOpen] = useState(false);
	const [githubTokenDraft, setGithubTokenDraft] = useState("");

	const gistIdFromSettings = settings?.github_backup_gist_id ?? "";
	const [gistIdDraft, setGistIdDraft] = useState(gistIdFromSettings);
	useEffect(() => {
		setGistIdDraft(gistIdFromSettings);
	}, [gistIdFromSettings]);

	const saveGistId = useMutation({
		mutationFn: async (gistId: string | null) => {
			await tauriAPI.updateGithubBackupGistId(gistId);
		},
		onSuccess: () => {
			invalidateSettingsQuery();
		},
	});

	const exportSettingsBackup = useMutation({
		mutationFn: async (): Promise<boolean> => {
			const path = await save({
				defaultPath: "kolboo-settings-backup.json",
				filters: [{ name: "JSON", extensions: ["json"] }],
			});

			if (!path) return false;

			await backupAPI.exportSettingsBackupToFile({ path });
			return true;
		},
		onSuccess: (didExport) => {
			if (!didExport) return;
			notifications.show({
				title: "Exported",
				message: "Settings backup saved (secrets excluded).",
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Export failed",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const importSettingsBackup = useMutation({
		mutationFn: async (): Promise<boolean> => {
			const selected = await open({
				multiple: false,
				directory: false,
				filters: [{ name: "JSON", extensions: ["json"] }],
			});

			const path = typeof selected === "string" ? selected : null;
			if (!path) return false;

			await backupAPI.importSettingsBackupFromFile({ path });
			await reRegisterShortcuts();

			return true;
		},
		onSuccess: (didImport) => {
			if (!didImport) return;
			invalidateImportedSettingsQueries();
			notifications.show({
				title: "Imported",
				message: "Settings backup imported (secrets excluded).",
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Import failed",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const setGithubToken = useMutation({
		mutationFn: async (token: string) => {
			await backupAPI.githubBackupSetToken({ token });
		},
		onSuccess: () => {
			setGithubTokenDraft("");
			setGithubTokenModalOpen(false);
			void githubBackupHasToken.refetch();
			notifications.show({
				title: "GitHub token saved",
				message: "Stored securely in your OS credential manager.",
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Failed to save token",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const clearGithubToken = useMutation({
		mutationFn: async () => {
			await backupAPI.githubBackupClearToken();
		},
		onSuccess: () => {
			void githubBackupHasToken.refetch();
			notifications.show({
				title: "GitHub token removed",
				message: "The stored token was deleted from secure storage.",
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Failed to remove token",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const pushToGist = useMutation({
		mutationFn: async () => {
			const gistId = (gistIdDraft ?? "").trim();
			const nextId = await backupAPI.githubBackupPushToGist({
				gistId: gistId || null,
			});
			await tauriAPI.updateGithubBackupGistId(nextId);
			return nextId;
		},
		onSuccess: (id) => {
			invalidateSettingsQuery();
			notifications.show({
				title: "Backed up",
				message: `Settings pushed to GitHub Gist (${id}).`,
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Backup failed",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const pullFromGist = useMutation({
		mutationFn: async () => {
			const id = (gistIdDraft ?? "").trim();
			if (!id) throw new Error("Missing gist id");

			const json = await backupAPI.githubBackupPullFromGist({ gistId: id });
			await backupAPI.importSettingsBackupJson({ json });
			await reRegisterShortcuts();
		},
		onSuccess: () => {
			invalidateImportedSettingsQueries();
			notifications.show({
				title: "Restored",
				message: "Settings pulled from GitHub Gist and imported.",
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "Restore failed",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const cloudSyncState = useQuery({
		queryKey: ["cloudSyncUiState"],
		queryFn: () => readCloudSyncUiState(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});

	const refreshCloudSyncState = () => {
		void queryClient.invalidateQueries({ queryKey: ["cloudSyncUiState"] });
	};

	const runCloudSyncAction = useMutation({
		mutationFn: async (action: "push" | "pull") => {
			await invoke("settings_apply_patch", {
				patch: { __cloud_sync_action: action },
				deleteKeys: [],
			});
		},
		onSuccess: (_value, action) => {
			refreshCloudSyncState();
			invalidateSettingsQuery();
			void trackProductEvent("cloud_sync_action_succeeded", {
				action,
			});
			notifications.show({
				title: action === "push" ? "Cloud sync pushed" : "Cloud sync pulled",
				message:
					action === "push"
						? "Settings uploaded to cloud sync endpoint."
						: "Settings pulled from cloud sync endpoint.",
				color: "green",
			});
		},
		onError: (e, action) => {
			refreshCloudSyncState();
			void trackProductEvent("cloud_sync_action_failed", {
				action,
				error_kind: e instanceof Error ? e.name : typeof e,
			});
			notifications.show({
				title: "Cloud sync failed",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});

	const updateCloudSyncEnabled = useMutation({
		mutationFn: async (enabled: boolean) => {
			await invoke("settings_apply_patch", {
				patch: { cloud_sync_enabled: enabled },
				deleteKeys: [],
			});
		},
		onSuccess: (_value, enabled) => {
			refreshCloudSyncState();
			invalidateSettingsQuery();
			void trackProductEvent("cloud_sync_enabled_changed", {
				enabled,
			});
		},
	});

	const updateCloudSyncAutoPush = useMutation({
		mutationFn: async (enabled: boolean) => {
			await invoke("settings_apply_patch", {
				patch: { cloud_sync_auto_push: enabled },
				deleteKeys: [],
			});
		},
		onSuccess: (_value, enabled) => {
			refreshCloudSyncState();
			invalidateSettingsQuery();
			void trackProductEvent("cloud_sync_auto_push_changed", {
				enabled,
			});
		},
	});

	const updatePosthogAnalyticsEnabled = useMutation({
		mutationFn: async (enabled: boolean) => {
			await invoke("settings_apply_patch", {
				patch: { posthog_analytics_enabled: enabled },
				deleteKeys: [],
			});
		},
		onSuccess: (_value, enabled) => {
			refreshCloudSyncState();
			invalidateSettingsQuery();
			if (enabled) {
				void trackProductEvent("analytics_opted_in", {
					surface: "settings",
				});
			}
		},
	});

	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const profile: RewriteProgramPromptProfile | null =
		editingProfileId && editingProfileId !== "default"
			? (profiles.find((entry) => entry.id === editingProfileId) ?? null)
			: null;

	const isProfileScope = profile !== null;

	// ---------------------------------------------------------------------------
	// Logs retention
	// ---------------------------------------------------------------------------

	const logsRetentionModeFromSettings: RequestLogsRetentionMode =
		settings?.request_logs_retention_mode ?? "amount";
	const logsRetentionAmountFromSettings =
		settings?.request_logs_retention_amount ?? 10;
	const logsRetentionDaysFromSettings =
		settings?.request_logs_retention_days ?? 7;
	const logsRetentionUnitFromSettings: RetentionUnit = "days";
	const logsRetentionValueFromSettings = logsRetentionDaysFromSettings;

	const [logsRetentionDraft, setLogsRetentionDraft] = useState<{
		mode: RequestLogsRetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	} | null>(null);

	useEffect(() => {
		void logsRetentionModeFromSettings;
		void logsRetentionAmountFromSettings;
		void logsRetentionUnitFromSettings;
		void logsRetentionValueFromSettings;
		// Drop draft once settings refresh from disk so we stay source-of-truth.
		setLogsRetentionDraft(null);
	}, [
		logsRetentionModeFromSettings,
		logsRetentionAmountFromSettings,
		logsRetentionValueFromSettings,
	]);

	const logsRetentionMode =
		logsRetentionDraft?.mode ?? logsRetentionModeFromSettings;
	const logsRetentionAmount =
		logsRetentionDraft?.amount ?? logsRetentionAmountFromSettings;
	const logsRetentionUnit =
		logsRetentionDraft?.unit ?? logsRetentionUnitFromSettings;
	const logsRetentionValue =
		logsRetentionDraft?.value ?? logsRetentionValueFromSettings;

	const commitLogsRetention = (next: {
		mode: RequestLogsRetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	}) => {
		setLogsRetentionDraft(next);
		const days = retentionDaysFromUnitValue(next);
		updateRequestLogsRetention.mutate({
			mode: next.mode,
			amount: next.amount,
			days,
		});
	};

	// ---------------------------------------------------------------------------
	// Recordings retention (amount | time)
	// ---------------------------------------------------------------------------

	const recordingsRetentionModeFromSettings: RetentionMode =
		settings?.recordings_retention_mode ?? "amount";
	const recordingsRetentionAmountFromSettings =
		settings?.recordings_retention_amount ??
		settings?.max_saved_recordings ??
		50;
	const recordingsRetentionUnitFromSettings: RetentionUnit =
		settings?.recordings_retention_unit ?? "days";
	const recordingsRetentionValueFromSettings =
		settings?.recordings_retention_value ?? 0;

	const [recordingsRetentionDraft, setRecordingsRetentionDraft] = useState<{
		mode: RetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	} | null>(null);

	useEffect(() => {
		void recordingsRetentionModeFromSettings;
		void recordingsRetentionAmountFromSettings;
		void recordingsRetentionUnitFromSettings;
		void recordingsRetentionValueFromSettings;
		setRecordingsRetentionDraft(null);
	}, [
		recordingsRetentionModeFromSettings,
		recordingsRetentionAmountFromSettings,
		recordingsRetentionUnitFromSettings,
		recordingsRetentionValueFromSettings,
	]);

	const recordingsRetentionMode =
		recordingsRetentionDraft?.mode ?? recordingsRetentionModeFromSettings;
	const recordingsRetentionAmount =
		recordingsRetentionDraft?.amount ?? recordingsRetentionAmountFromSettings;
	const recordingsRetentionUnit =
		recordingsRetentionDraft?.unit ?? recordingsRetentionUnitFromSettings;
	const recordingsRetentionValue =
		recordingsRetentionDraft?.value ?? recordingsRetentionValueFromSettings;

	const updateRecordingsRetention = useMutation({
		mutationFn: (params: {
			mode: RetentionMode;
			amount: number;
			unit: RetentionUnit;
			value: number;
		}) => tauriAPI.updateRecordingsRetention(params),
		onSuccess: () => {
			invalidateSettingsQuery();
			void queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
		},
	});

	const commitRecordingsRetention = (next: {
		mode: RetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	}) => {
		setRecordingsRetentionDraft(next);
		updateRecordingsRetention.mutate(next);

		// Keep legacy key in sync for older builds / other call sites.
		if (next.mode === "amount") {
			updateMaxSavedRecordings.mutate(next.amount);
		}
	};

	const handleOpenRecordingsFolder = async () => {
		try {
			await recordingsAPI.openRecordingsFolder();
		} catch (e) {
			notifications.show({
				title: "Recordings",
				message: formatErrorMessage(e),
				color: "red",
			});
		}
	};

	const handleOpenAppLogsFolder = async () => {
		try {
			await logsAPI.openAppLogsFolder();
		} catch (e) {
			notifications.show({
				title: "App logs",
				message: formatErrorMessage(e),
				color: "red",
			});
		}
	};

	const closeGithubTokenModal = () => {
		if (setGithubToken.isPending) return;
		setGithubTokenModalOpen(false);
	};

	const recordingsSummary = summarizeRecordingsStorage(recordingsStats.data);

	// ---------------------------------------------------------------------------
	// Danger zone (destructive actions)
	// ---------------------------------------------------------------------------

	const [dangerDialog, setDangerDialog] =
		useState<DataDangerDialogState | null>(null);
	const [dangerRunning, setDangerRunning] = useState(false);
	const [dangerTypedDraft, setDangerTypedDraft] = useState("");

	const runDangerAction = async (action: () => Promise<void>) => {
		await action();

		// Ensure UI reflects the new reality after destructive actions.
		invalidateDangerZoneQueries();
	};

	const openDangerDialog = (args: DataDangerDialogState) => {
		setDangerTypedDraft("");
		setDangerDialog(args);
	};

	const closeDangerDialog = () => {
		if (dangerRunning) return;
		setDangerTypedDraft("");
		setDangerDialog(null);
	};

	// ---------------------------------------------------------------------------
	// Transcription retention (amount | time)
	// ---------------------------------------------------------------------------

	const transcriptionRetentionModeFromSettings: RetentionMode =
		settings?.transcription_retention_mode ?? "time";
	const transcriptionRetentionAmountFromSettings =
		settings?.transcription_retention_amount ?? 1000;

	const transcriptionRetentionUnitFromSettings: TranscriptionRetentionUnit =
		settings?.transcription_retention_unit ?? "days";
	const transcriptionRetentionValueFromSettings =
		settings?.transcription_retention_value ?? 0;
	const transcriptionRetentionDeleteRecordings =
		settings?.transcription_retention_delete_recordings ?? false;

	const [transcriptionRetentionDraft, setTranscriptionRetentionDraft] =
		useState<{
			mode: RetentionMode;
			amount: number;
			unit: TranscriptionRetentionUnit;
			value: number;
		} | null>(null);

	useEffect(() => {
		void transcriptionRetentionModeFromSettings;
		void transcriptionRetentionAmountFromSettings;
		void transcriptionRetentionUnitFromSettings;
		void transcriptionRetentionValueFromSettings;
		// Drop any draft once settings refresh from disk so we stay source-of-truth.
		setTranscriptionRetentionDraft(null);
	}, [
		transcriptionRetentionModeFromSettings,
		transcriptionRetentionAmountFromSettings,
		transcriptionRetentionUnitFromSettings,
		transcriptionRetentionValueFromSettings,
	]);

	const transcriptionRetentionMode =
		transcriptionRetentionDraft?.mode ?? transcriptionRetentionModeFromSettings;
	const transcriptionRetentionAmount =
		transcriptionRetentionDraft?.amount ??
		transcriptionRetentionAmountFromSettings;
	const transcriptionRetentionUnit =
		transcriptionRetentionDraft?.unit ?? transcriptionRetentionUnitFromSettings;
	const transcriptionRetentionValue =
		transcriptionRetentionDraft?.value ??
		transcriptionRetentionValueFromSettings;

	const updateTranscriptionRetentionPolicy = useMutation({
		mutationFn: (params: {
			mode: RetentionMode;
			amount: number;
			unit: TranscriptionRetentionUnit;
			value: number;
		}) => tauriAPI.updateTranscriptionRetentionPolicy(params),
		onSuccess: () => {
			invalidateSettingsQuery();
		},
	});

	const commitTranscriptionRetentionPolicy = (next: {
		mode: RetentionMode;
		amount: number;
		unit: TranscriptionRetentionUnit;
		value: number;
	}) => {
		setTranscriptionRetentionDraft(next);
		updateTranscriptionRetentionPolicy.mutate(next);
	};

	// ---------------------------------------------------------------------------
	// Stats retention (time)
	// ---------------------------------------------------------------------------

	const statsRetentionUnitFromSettings: TranscriptionRetentionUnit =
		settings?.stats_retention_unit ?? "days";
	const statsRetentionValueFromSettings = settings?.stats_retention_value ?? 30;

	const [statsRetentionDraft, setStatsRetentionDraft] = useState<{
		unit: TranscriptionRetentionUnit;
		value: number;
	} | null>(null);

	useEffect(() => {
		void statsRetentionUnitFromSettings;
		void statsRetentionValueFromSettings;
		setStatsRetentionDraft(null);
	}, [statsRetentionUnitFromSettings, statsRetentionValueFromSettings]);

	const statsRetentionUnit =
		statsRetentionDraft?.unit ?? statsRetentionUnitFromSettings;
	const statsRetentionValue =
		statsRetentionDraft?.value ?? statsRetentionValueFromSettings;

	const commitStatsRetention = (next: {
		unit: TranscriptionRetentionUnit;
		value: number;
	}) => {
		setStatsRetentionDraft(next);
		updateStatsRetention.mutate(next);
	};

	const dataStorageBreakdown = dataStorageSummary.data
		? buildDataStorageBreakdown({
				summary: dataStorageSummary.data,
				apiKeysSavedCount: apiKeysSavedCount.data,
				apiKeyStoreKeyCount: API_KEY_STORE_KEYS.length,
			})
		: [];

	const openExternalUrlWithFallback = async (args: {
		url: string;
		title: string;
		message: string;
	}) => {
		try {
			await openUrl(args.url);
		} catch {
			notifications.show({
				title: args.title,
				message: args.message,
				color: "red",
			});
		}
	};

	// Keep the destructive-action list visible in the adapter. The section just
	// lays the buttons out; the adapter still owns the meaning of each action.
	const dangerActions: DangerZoneAction[] = [
		{
			key: "delete-recordings",
			label: "Delete recordings",
			icon: <Trash2 size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Delete recordings",
					message:
						"This will permanently delete all saved .wav recordings from disk.",
					confirmLabel: "Delete recordings",
					action: async () => {
						await dataAPI.deleteAllRecordings();
					},
				});
			},
		},
		{
			key: "delete-transcriptions",
			label: "Delete transcriptions",
			icon: <MessageSquare size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Delete transcriptions (history)",
					message:
						"This will permanently delete all saved transcriptions from the History tab.",
					confirmLabel: "Delete transcriptions",
					action: async () => {
						await tauriAPI.clearHistory();
						await tauriAPI.emitHistoryChanged();
					},
				});
			},
		},
		{
			key: "delete-transcripts",
			label: "Delete transcripts",
			icon: <FileText size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Delete transcripts (keep recordings)",
					message:
						"This will delete all transcript text from history, but keep your saved .wav recordings.",
					confirmLabel: "Delete transcripts",
					action: async () => {
						await dataAPI.deleteAllTranscriptsKeepRecordings();
					},
				});
			},
		},
		{
			key: "clear-request-logs",
			label: "Clear request logs",
			icon: <FileText size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Clear request logs",
					message:
						"This will clear in-memory request logs shown in the Logs tab.",
					confirmLabel: "Clear logs",
					action: async () => {
						await logsAPI.clearRequestLogs();
					},
				});
			},
		},
		{
			key: "delete-stats",
			label: "Delete stats",
			icon: <BarChart2 size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Delete usage/cost stats",
					message:
						"This will permanently delete persisted usage/cost stats (JSONL shards).",
					confirmLabel: "Delete stats",
					action: async () => {
						await dataAPI.deleteAllStats();
					},
				});
			},
		},
		{
			key: "delete-api-keys",
			label: "Delete API keys",
			icon: <Key size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Delete API keys",
					message:
						"This will remove all stored API keys (OpenAI, Groq, Deepgram, Gemini, Anthropic).",
					confirmLabel: "Delete API keys",
					action: async () => {
						await dataAPI.deleteAllApiKeys();
					},
				});
			},
		},
		{
			key: "reset-settings",
			label: "Reset settings",
			icon: <RotateCcw size={14} />,
			color: "red",
			variant: "outline",
			onClick: () => {
				openDangerDialog({
					title: "Reset settings",
					message:
						"This will reset all settings back to defaults (including API keys).",
					confirmLabel: "Reset settings",
					action: async () => {
						await dataAPI.deleteAllSettings();
						await reRegisterShortcuts();
					},
				});
			},
		},
		{
			key: "delete-all-data",
			label: "Delete all data",
			icon: <Skull size={14} />,
			color: "red",
			variant: "filled",
			fullWidth: true,
			onClick: () => {
				openDangerDialog({
					title: "Delete all data",
					message:
						"This will delete ALL app data: history, recordings, request logs, persisted stats, and settings (including API keys).",
					typedConfirm: {
						requiredText: "DELETE",
						label: "Type DELETE to confirm",
						placeholder: "DELETE",
					},
					confirmLabel: "Delete everything",
					action: async () => {
						await dataAPI.deleteAllData();
						await logsAPI.clearRequestLogs();
						await reRegisterShortcuts();
					},
				});
			},
		},
	];

	const content = (
		<>
			<DataRetentionSection
				isProfileScope={isProfileScope}
				logsRetention={{
					mode: logsRetentionMode,
					amount: logsRetentionAmount,
					unit: logsRetentionUnit,
					value: logsRetentionValue,
					onCommit: commitLogsRetention,
				}}
				recordingsRetention={{
					mode: recordingsRetentionMode,
					amount: recordingsRetentionAmount,
					unit: recordingsRetentionUnit,
					value: recordingsRetentionValue,
					onCommit: commitRecordingsRetention,
				}}
				transcriptionRetention={{
					mode: transcriptionRetentionMode,
					amount: transcriptionRetentionAmount,
					unit: transcriptionRetentionUnit,
					value: transcriptionRetentionValue,
					deleteRecordings: transcriptionRetentionDeleteRecordings,
					onCommit: commitTranscriptionRetentionPolicy,
					onDeleteRecordingsChange: (checked) => {
						updateTranscriptionRetentionDeleteRecordings.mutate(checked);
					},
				}}
				statsRetention={{
					unit: statsRetentionUnit,
					value: statsRetentionValue,
					onCommit: commitStatsRetention,
				}}
				recordingsStatsLoading={recordingsStats.isLoading}
				recordingsSummary={recordingsSummary}
				onOpenAppLogsFolder={() => {
					void handleOpenAppLogsFolder();
				}}
				onOpenRecordingsFolder={() => {
					void handleOpenRecordingsFolder();
				}}
			/>

			<DataBackupSection
				exportSettingsBackupPending={exportSettingsBackup.isPending}
				importSettingsBackupPending={importSettingsBackup.isPending}
				onExportSettingsBackup={() => exportSettingsBackup.mutate()}
				onImportSettingsBackup={() => importSettingsBackup.mutate()}
				githubBackupHasTokenLoading={githubBackupHasToken.isLoading}
				githubBackupHasToken={githubBackupHasToken.data ?? false}
				onOpenGithubTokenModal={() => setGithubTokenModalOpen(true)}
				clearGithubTokenPending={clearGithubToken.isPending}
				onClearGithubToken={() => clearGithubToken.mutate()}
				gistIdDraft={gistIdDraft}
				onGistIdDraftChange={(value) => setGistIdDraft(value)}
				saveGistIdPending={saveGistId.isPending}
				onSaveGistId={() => {
					const trimmed = (gistIdDraft ?? "").trim();
					saveGistId.mutate(trimmed || null);
				}}
				pushToGistPending={pushToGist.isPending}
				onPushToGist={() => pushToGist.mutate()}
				pullFromGistPending={pullFromGist.isPending}
				onPullFromGist={() => pullFromGist.mutate()}
			/>

			<DataCloudSyncSection
				isProfileScope={isProfileScope}
				globalOnlyTooltip={GLOBAL_ONLY_TOOLTIP}
				cloudSyncState={cloudSyncState.data}
				cloudSyncStateLoading={cloudSyncState.isLoading}
				runCloudSyncActionPending={runCloudSyncAction.isPending}
				onPushCloudSync={() => runCloudSyncAction.mutate("push")}
				onPullCloudSync={() => runCloudSyncAction.mutate("pull")}
				updateCloudSyncEnabledPending={updateCloudSyncEnabled.isPending}
				onCloudSyncEnabledChange={(enabled) => {
					updateCloudSyncEnabled.mutate(enabled);
				}}
				updateCloudSyncAutoPushPending={updateCloudSyncAutoPush.isPending}
				onCloudSyncAutoPushChange={(enabled) => {
					updateCloudSyncAutoPush.mutate(enabled);
				}}
				updatePosthogAnalyticsEnabledPending={
					updatePosthogAnalyticsEnabled.isPending
				}
				onPosthogAnalyticsEnabledChange={(enabled) => {
					updatePosthogAnalyticsEnabled.mutate(enabled);
				}}
			/>

			<DataDangerZoneSection
				storageSummaryLoading={dataStorageSummary.isLoading}
				storageBreakdownItems={dataStorageBreakdown}
				actions={dangerActions}
			/>

			<DataGithubTokenModal
				opened={githubTokenModalOpen}
				saving={setGithubToken.isPending}
				value={githubTokenDraft}
				onChange={(value) => setGithubTokenDraft(value)}
				onClose={closeGithubTokenModal}
				onSave={() => {
					const token = githubTokenDraft.trim();
					setGithubToken.mutate(token);
				}}
				onOpenTokenCreationPage={() => {
					void openExternalUrlWithFallback({
						url: "https://github.com/settings/personal-access-tokens/new",
						title: "Couldn't open link",
						message:
							"Failed to open your browser. You can open the token page manually from GitHub settings.",
					});
				}}
				onOpenDocs={() => {
					void openExternalUrlWithFallback({
						url: "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
						title: "Couldn't open link",
						message:
							"Failed to open your browser. You can find GitHub token docs on docs.github.com.",
					});
				}}
			/>

			<DataDangerConfirmModal
				dialog={dangerDialog}
				running={dangerRunning}
				typedDraft={dangerTypedDraft}
				onTypedDraftChange={(value) => setDangerTypedDraft(value)}
				onClose={closeDangerDialog}
				onConfirm={async () => {
					const action = dangerDialog?.action;
					if (!action) return;

					try {
						setDangerRunning(true);
						await runDangerAction(action);
						notifications.show({
							title: "Done",
							message: "Completed.",
							color: "green",
						});
						setDangerTypedDraft("");
						setDangerDialog(null);
					} catch (e) {
						notifications.show({
							title: "Failed",
							message: formatErrorMessage(e),
							color: "red",
						});
					} finally {
						setDangerRunning(false);
					}
				}}
			/>
		</>
	);

	if (isProfileScope) {
		return (
			<Tooltip label={GLOBAL_ONLY_TOOLTIP} withArrow position="top-start">
				<div style={{ opacity: 0.5, cursor: "not-allowed" }}>
					<div style={{ pointerEvents: "none" }}>{content}</div>
				</div>
			</Tooltip>
		);
	}

	return content;
}
