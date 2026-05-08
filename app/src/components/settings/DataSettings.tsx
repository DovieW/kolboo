import { Tooltip } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useQuery, useQueryClient } from "@tanstack/react-query";
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
import { useState } from "react";
import { API_KEY_STORE_KEYS } from "../../lib/apiKeys";
import { formatErrorMessage } from "../../lib/formatError";
import {
	useDataStorageSummary,
	useRecordingsStats,
	useSettings,
} from "../../lib/queries";
import {
	type CloudSyncAction,
	type GithubBackupGistResult,
	type SettingsBackupFileResult,
	useDataBackupCloudSyncOrchestration,
} from "../../lib/settings/dataBackupCloudSync";
import {
	buildDataStorageBreakdown,
	summarizeRecordingsStorage,
} from "../../lib/settings/dataLifecycle";
import { useDataRetentionOrchestration } from "../../lib/settings/dataRetention";
import {
	dataAPI,
	logsAPI,
	type RewriteProgramPromptProfile,
	recordingsAPI,
	tauriAPI,
} from "../../lib/tauri";
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

	const dataBackupCloudSync = useDataBackupCloudSyncOrchestration({
		gistIdFromSettings: settings?.github_backup_gist_id ?? "",
		effects: {
			onSettingsChanged: invalidateSettingsQuery,
			onImportedSettingsApplied: invalidateImportedSettingsQueries,
			reRegisterShortcuts,
		},
	});

	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const profile: RewriteProgramPromptProfile | null =
		editingProfileId && editingProfileId !== "default"
			? (profiles.find((entry) => entry.id === editingProfileId) ?? null)
			: null;

	const isProfileScope = profile !== null;

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
		if (dataBackupCloudSync.setGithubToken.isPending) return;
		dataBackupCloudSync.setGithubTokenModalOpen(false);
	};

	const handleExportSettingsBackup = async () => {
		const path = await save({
			defaultPath: "kolboo-settings-backup.json",
			filters: [{ name: "JSON", extensions: ["json"] }],
		});

		dataBackupCloudSync.exportSettingsBackup.mutate(path, {
			onSuccess: (result: SettingsBackupFileResult) => {
				if (result.kind !== "exported") return;
				notifications.show({
					title: "Exported",
					message: "Settings backup saved (secrets excluded).",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Export failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleImportSettingsBackup = async () => {
		const selected = await open({
			multiple: false,
			directory: false,
			filters: [{ name: "JSON", extensions: ["json"] }],
		});
		const path = typeof selected === "string" ? selected : null;

		dataBackupCloudSync.importSettingsBackup.mutate(path, {
			onSuccess: (result: SettingsBackupFileResult) => {
				if (result.kind !== "imported") return;
				notifications.show({
					title: "Imported",
					message: "Settings backup imported (secrets excluded).",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Import failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleSaveGithubToken = () => {
		const token = dataBackupCloudSync.githubTokenDraft.trim();

		dataBackupCloudSync.setGithubToken.mutate(token, {
			onSuccess: () => {
				notifications.show({
					title: "GitHub token saved",
					message: "Stored securely in your OS credential manager.",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Failed to save token",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleClearGithubToken = () => {
		dataBackupCloudSync.clearGithubToken.mutate(undefined, {
			onSuccess: () => {
				notifications.show({
					title: "GitHub token removed",
					message: "The stored token was deleted from secure storage.",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Failed to remove token",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleSaveGistId = () => {
		dataBackupCloudSync.saveGistId.mutate(dataBackupCloudSync.gistIdDraft);
	};

	const handlePushToGist = () => {
		dataBackupCloudSync.pushToGist.mutate(dataBackupCloudSync.gistIdDraft, {
			onSuccess: (result: GithubBackupGistResult) => {
				if (result.kind !== "pushed") return;
				notifications.show({
					title: "Backed up",
					message: `Settings pushed to GitHub Gist (${result.gistId}).`,
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Backup failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handlePullFromGist = () => {
		dataBackupCloudSync.pullFromGist.mutate(dataBackupCloudSync.gistIdDraft, {
			onSuccess: (result: GithubBackupGistResult) => {
				if (result.kind !== "pulled") return;
				notifications.show({
					title: "Restored",
					message: "Settings pulled from GitHub Gist and imported.",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Restore failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleCloudSyncAction = (action: CloudSyncAction) => {
		dataBackupCloudSync.runCloudSyncAction.mutate(action, {
			onSuccess: () => {
				notifications.show({
					title: action === "push" ? "Cloud sync pushed" : "Cloud sync pulled",
					message:
						action === "push"
							? "Settings uploaded to cloud sync endpoint."
							: "Settings pulled from cloud sync endpoint.",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Cloud sync failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
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

	const dataRetention = useDataRetentionOrchestration({
		settings,
		isProfileScope,
		effects: {
			onSettingsChanged: invalidateSettingsQuery,
			onRequestLogsChanged: () =>
				queryClient.invalidateQueries({ queryKey: ["requestLogs"] }),
			onRecordingsChanged: () =>
				queryClient.invalidateQueries({ queryKey: ["recordingsStats"] }),
		},
	});

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
				logsRetention={dataRetention.logsRetention}
				recordingsRetention={dataRetention.recordingsRetention}
				transcriptionRetention={dataRetention.transcriptionRetention}
				statsRetention={dataRetention.statsRetention}
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
				exportSettingsBackupPending={
					dataBackupCloudSync.exportSettingsBackup.isPending
				}
				importSettingsBackupPending={
					dataBackupCloudSync.importSettingsBackup.isPending
				}
				onExportSettingsBackup={() => {
					void handleExportSettingsBackup();
				}}
				onImportSettingsBackup={() => {
					void handleImportSettingsBackup();
				}}
				githubBackupHasTokenLoading={
					dataBackupCloudSync.githubBackupHasToken.isLoading
				}
				githubBackupHasToken={
					dataBackupCloudSync.githubBackupHasToken.data ?? false
				}
				onOpenGithubTokenModal={() =>
					dataBackupCloudSync.setGithubTokenModalOpen(true)
				}
				clearGithubTokenPending={dataBackupCloudSync.clearGithubToken.isPending}
				onClearGithubToken={handleClearGithubToken}
				gistIdDraft={dataBackupCloudSync.gistIdDraft}
				onGistIdDraftChange={(value) =>
					dataBackupCloudSync.setGistIdDraft(value)
				}
				saveGistIdPending={dataBackupCloudSync.saveGistId.isPending}
				onSaveGistId={handleSaveGistId}
				pushToGistPending={dataBackupCloudSync.pushToGist.isPending}
				onPushToGist={handlePushToGist}
				pullFromGistPending={dataBackupCloudSync.pullFromGist.isPending}
				onPullFromGist={handlePullFromGist}
			/>

			<DataCloudSyncSection
				isProfileScope={isProfileScope}
				globalOnlyTooltip={GLOBAL_ONLY_TOOLTIP}
				cloudSyncState={dataBackupCloudSync.cloudSyncState.data}
				cloudSyncStateLoading={dataBackupCloudSync.cloudSyncState.isLoading}
				runCloudSyncActionPending={
					dataBackupCloudSync.runCloudSyncAction.isPending
				}
				onPushCloudSync={() => handleCloudSyncAction("push")}
				onPullCloudSync={() => handleCloudSyncAction("pull")}
				updateCloudSyncEnabledPending={
					dataBackupCloudSync.updateCloudSyncEnabled.isPending
				}
				onCloudSyncEnabledChange={(enabled) => {
					dataBackupCloudSync.updateCloudSyncEnabled.mutate(enabled);
				}}
				updateCloudSyncAutoPushPending={
					dataBackupCloudSync.updateCloudSyncAutoPush.isPending
				}
				onCloudSyncAutoPushChange={(enabled) => {
					dataBackupCloudSync.updateCloudSyncAutoPush.mutate(enabled);
				}}
				updatePosthogAnalyticsEnabledPending={
					dataBackupCloudSync.updatePosthogAnalyticsEnabled.isPending
				}
				onPosthogAnalyticsEnabledChange={(enabled) => {
					dataBackupCloudSync.updatePosthogAnalyticsEnabled.mutate(enabled);
				}}
			/>

			<DataDangerZoneSection
				storageSummaryLoading={dataStorageSummary.isLoading}
				storageBreakdownItems={dataStorageBreakdown}
				actions={dangerActions}
			/>

			<DataGithubTokenModal
				opened={dataBackupCloudSync.githubTokenModalOpen}
				saving={dataBackupCloudSync.setGithubToken.isPending}
				value={dataBackupCloudSync.githubTokenDraft}
				onChange={(value) => dataBackupCloudSync.setGithubTokenDraft(value)}
				onClose={closeGithubTokenModal}
				onSave={handleSaveGithubToken}
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
