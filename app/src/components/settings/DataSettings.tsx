import {
	ActionIcon,
	Button,
	Checkbox,
	Group,
	Modal,
	NumberInput,
	PasswordInput,
	SegmentedControl,
	Stack,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
	BarChart2,
	Download,
	FileText,
	FolderOpen,
	Github,
	Key,
	MessageSquare,
	RotateCcw,
	Skull,
	Trash2,
	Upload,
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
	backupAPI,
	configAPI,
	dataAPI,
	logsAPI,
	type RewriteProgramPromptProfile,
	recordingsAPI,
	type TranscriptionRetentionUnit,
	tauriAPI,
} from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

type RequestLogsRetentionMode = "amount" | "time";
type RetentionMode = "amount" | "time";
type RetentionUnit = "days" | "hours";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

export function DataSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const { data: settings } = useSettings();

	const queryClient = useQueryClient();

	const updateRequestLogsRetention = useMutation({
		mutationFn: (params: {
			mode: RequestLogsRetentionMode;
			amount: number;
			days: number;
		}) => tauriAPI.updateRequestLogsRetention(params),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
			queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		},
	});

	const updateStatsRetention = useMutation({
		mutationFn: (params: { unit: TranscriptionRetentionUnit; value: number }) =>
			tauriAPI.updateStatsRetention(params),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
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
			queryClient.invalidateQueries({ queryKey: ["settings"] });
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
			await tauriAPI.unregisterShortcuts();
			await tauriAPI.registerShortcuts();

			return true;
		},
		onSuccess: (didImport) => {
			if (!didImport) return;
			queryClient.invalidateQueries({ queryKey: ["settings"] });
			queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
			queryClient.invalidateQueries({ queryKey: ["dataStorageSummary"] });
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
			githubBackupHasToken.refetch();
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
			githubBackupHasToken.refetch();
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
			queryClient.invalidateQueries({ queryKey: ["settings"] });
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
			await tauriAPI.unregisterShortcuts();
			await tauriAPI.registerShortcuts();
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
			queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
			queryClient.invalidateQueries({ queryKey: ["dataStorageSummary"] });
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

	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const profile: RewriteProgramPromptProfile | null =
		editingProfileId && editingProfileId !== "default"
			? (profiles.find((p) => p.id === editingProfileId) ?? null)
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
		const days =
			next.unit === "hours" ? next.value / 24 : Math.max(0, next.value);
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
			queryClient.invalidateQueries({ queryKey: ["settings"] });
			queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
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

	const recordingsSummary = (() => {
		const stats = recordingsStats.data;
		if (!stats) return null;
		if (typeof stats.count !== "number" || !Number.isFinite(stats.count))
			return null;
		if (typeof stats.bytes !== "number" || !Number.isFinite(stats.bytes))
			return null;

		const gb = stats.bytes / 1024 ** 3;
		return {
			count: Math.max(0, Math.round(stats.count)),
			gb,
		};
	})();

	const formatBytes = (bytes: number) => {
		const b =
			typeof bytes === "number" && Number.isFinite(bytes)
				? Math.max(0, bytes)
				: 0;
		if (b < 1024) return `${Math.round(b)} B`;
		const kb = b / 1024;
		if (kb < 1024) return `${kb.toFixed(1)} KB`;
		const mb = kb / 1024;
		if (mb < 1024) return `${mb.toFixed(1)} MB`;
		const gb = mb / 1024;
		return `${gb.toFixed(2)} GB`;
	};

	// ---------------------------------------------------------------------------
	// Danger zone (destructive actions)
	// ---------------------------------------------------------------------------

	const [dangerDialog, setDangerDialog] = useState<null | {
		title: string;
		message: string;
		confirmLabel: string;
		typedConfirm?: {
			requiredText: string;
			label?: string;
			placeholder?: string;
		};
		action: () => Promise<void>;
	}>(null);

	const [dangerRunning, setDangerRunning] = useState(false);
	const [dangerTypedDraft, setDangerTypedDraft] = useState("");

	const runDangerAction = async (action: () => Promise<void>) => {
		await action();

		// Ensure UI reflects the new reality.
		queryClient.invalidateQueries({ queryKey: ["settings"] });
		queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
		queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
		queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		queryClient.invalidateQueries({ queryKey: ["history"] });
		queryClient.invalidateQueries({ queryKey: ["dataStorageSummary"] });
		queryClient.invalidateQueries({ queryKey: ["apiKeysSavedCount"] });
	};

	const openDangerDialog = (args: {
		title: string;
		message: string;
		confirmLabel: string;
		typedConfirm?: {
			requiredText: string;
			label?: string;
			placeholder?: string;
		};
		action: () => Promise<void>;
	}) => {
		setDangerTypedDraft("");
		setDangerDialog(args);
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
			queryClient.invalidateQueries({ queryKey: ["settings"] });
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

	const content = (
		<>
			<SettingsRow
				label="Logs retention"
				description="Keep request logs for debugging. Default: store last 10."
				right={
					<Group gap={10} align="center" wrap="wrap">
						{logsRetentionMode === "amount" ? (
							<NumberInput
								value={logsRetentionAmount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 10;
									commitLogsRetention({
										mode: "amount",
										amount: nextAmount,
										unit: logsRetentionUnit,
										value: logsRetentionValue,
									});
								}}
								min={1}
								max={1000}
								step={1}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										width: 140,
									},
								}}
							/>
						) : (
							<>
								<NumberInput
									value={logsRetentionValue}
									onChange={(value) => {
										const nextValue = typeof value === "number" ? value : 7;
										commitLogsRetention({
											mode: "time",
											amount: logsRetentionAmount,
											unit: logsRetentionUnit,
											value: nextValue,
										});
									}}
									min={0}
									max={logsRetentionUnit === "hours" ? 36500 * 24 : 36500}
									step={logsRetentionUnit === "hours" ? 0.5 : 1}
									decimalScale={logsRetentionUnit === "hours" ? 2 : 0}
									clampBehavior="strict"
									disabled={isProfileScope}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 140,
										},
									}}
								/>

								<SegmentedControl
									value={logsRetentionUnit}
									onChange={(next) => {
										const nextUnit = next === "hours" ? "hours" : "days";

										const current =
											typeof logsRetentionValue === "number"
												? logsRetentionValue
												: 0;

										// Preserve the underlying duration when switching units.
										const nextValue =
											current === 0
												? 0
												: logsRetentionUnit === "days" && nextUnit === "hours"
													? current * 24
													: logsRetentionUnit === "hours" && nextUnit === "days"
														? Math.round(current / 24)
														: current;

										commitLogsRetention({
											mode: "time",
											amount: logsRetentionAmount,
											unit: nextUnit,
											value: nextValue,
										});
									}}
									data={[
										{ label: "Days", value: "days" },
										{ label: "Hours", value: "hours" },
									]}
									disabled={isProfileScope}
									styles={{
										root: {
											backgroundColor: "var(--bg-elevated)",
											border: "1px solid var(--border-default)",
										},
										label: {
											color: "var(--text-primary)",
										},
									}}
								/>
							</>
						)}

						<SegmentedControl
							value={logsRetentionMode}
							onChange={(next) => {
								const mode =
									next === "time" ? ("time" as const) : ("amount" as const);
								commitLogsRetention({
									mode,
									amount: logsRetentionAmount,
									unit: logsRetentionUnit,
									value: logsRetentionValue,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={{
								root: {
									backgroundColor: "var(--bg-elevated)",
									border: "1px solid var(--border-default)",
								},
								label: {
									color: "var(--text-primary)",
								},
							}}
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="Max recordings to save"
				description={`Keep at most this many recordings on disk.${
					recordingsStats.isLoading
						? " (Calculating storage…)"
						: recordingsSummary === null
							? ""
							: ` (Currently saved ${recordingsSummary.count} recordings at ${recordingsSummary.gb.toFixed(2)} GB)`
				}`}
				right={
					<Group gap={8} align="center">
						<Tooltip label="Open recordings folder" withArrow position="top">
							<span>
								<ActionIcon
									variant="default"
									size={36}
									onClick={() => {
										handleOpenRecordingsFolder().catch(console.error);
									}}
									aria-label="Open recordings folder"
									styles={{
										root: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											height: 36,
											width: 36,
										},
									}}
								>
									<FolderOpen size={14} style={{ opacity: 0.75 }} />
								</ActionIcon>
							</span>
						</Tooltip>

						{recordingsRetentionMode === "amount" ? (
							<NumberInput
								value={recordingsRetentionAmount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 50;
									commitRecordingsRetention({
										mode: "amount",
										amount: nextAmount,
										unit: recordingsRetentionUnit,
										value: recordingsRetentionValue,
									});
								}}
								min={1}
								max={100000}
								step={10}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										width: 140,
									},
								}}
							/>
						) : (
							<>
								<NumberInput
									value={recordingsRetentionValue}
									onChange={(value) => {
										const nextValue = typeof value === "number" ? value : 0;
										commitRecordingsRetention({
											mode: "time",
											amount: recordingsRetentionAmount,
											unit: recordingsRetentionUnit,
											value: nextValue,
										});
									}}
									min={0}
									max={recordingsRetentionUnit === "hours" ? 36500 * 24 : 36500}
									step={recordingsRetentionUnit === "hours" ? 0.5 : 1}
									decimalScale={recordingsRetentionUnit === "hours" ? 2 : 0}
									clampBehavior="strict"
									disabled={isProfileScope}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 140,
										},
									}}
								/>

								<SegmentedControl
									value={recordingsRetentionUnit}
									onChange={(next) => {
										const nextUnit = next === "hours" ? "hours" : "days";

										const current =
											typeof recordingsRetentionValue === "number"
												? recordingsRetentionValue
												: 0;

										// Preserve the underlying duration when switching units.
										const nextValue =
											current === 0
												? 0
												: recordingsRetentionUnit === "days" &&
														nextUnit === "hours"
													? current * 24
													: recordingsRetentionUnit === "hours" &&
															nextUnit === "days"
														? Math.round(current / 24)
														: current;

										commitRecordingsRetention({
											mode: "time",
											amount: recordingsRetentionAmount,
											unit: nextUnit,
											value: nextValue,
										});
									}}
									data={[
										{ label: "Days", value: "days" },
										{ label: "Hours", value: "hours" },
									]}
									disabled={isProfileScope}
									styles={{
										root: {
											backgroundColor: "var(--bg-elevated)",
											border: "1px solid var(--border-default)",
										},
										label: {
											color: "var(--text-primary)",
										},
									}}
								/>
							</>
						)}

						<SegmentedControl
							value={recordingsRetentionMode}
							onChange={(next) => {
								const mode = next === "time" ? "time" : "amount";
								commitRecordingsRetention({
									mode,
									amount: recordingsRetentionAmount,
									unit: recordingsRetentionUnit,
									value: recordingsRetentionValue,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={{
								root: {
									backgroundColor: "var(--bg-elevated)",
									border: "1px solid var(--border-default)",
								},
								label: {
									color: "var(--text-primary)",
								},
							}}
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="Transcription retention"
				description="Delete transcriptions older than this (0 = forever)."
				right={
					<Group gap={10} align="center" wrap="wrap">
						{transcriptionRetentionMode === "amount" ? (
							<NumberInput
								value={transcriptionRetentionAmount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 1000;
									commitTranscriptionRetentionPolicy({
										mode: "amount",
										amount: nextAmount,
										unit: transcriptionRetentionUnit,
										value: transcriptionRetentionValue,
									});
								}}
								min={1}
								max={100000}
								step={10}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										width: 140,
									},
								}}
							/>
						) : (
							<>
								<NumberInput
									value={transcriptionRetentionValue}
									onChange={(value) => {
										const next = typeof value === "number" ? value : 0;
										commitTranscriptionRetentionPolicy({
											mode: "time",
											amount: transcriptionRetentionAmount,
											unit: transcriptionRetentionUnit,
											value: next,
										});
									}}
									min={0}
									max={
										transcriptionRetentionUnit === "hours" ? 36500 * 24 : 36500
									}
									step={transcriptionRetentionUnit === "hours" ? 0.5 : 1}
									decimalScale={transcriptionRetentionUnit === "hours" ? 2 : 0}
									clampBehavior="strict"
									disabled={isProfileScope}
									styles={{
										input: {
											backgroundColor: "var(--bg-elevated)",
											borderColor: "var(--border-default)",
											color: "var(--text-primary)",
											width: 140,
										},
									}}
								/>

								<SegmentedControl
									value={transcriptionRetentionUnit}
									onChange={(next) => {
										const nextUnit =
											next === "hours" ? ("hours" as const) : ("days" as const);

										const current =
											typeof transcriptionRetentionValue === "number"
												? transcriptionRetentionValue
												: 0;

										// Preserve the underlying duration when switching units.
										const nextValue =
											current === 0
												? 0
												: transcriptionRetentionUnit === "days" &&
														nextUnit === "hours"
													? current * 24
													: transcriptionRetentionUnit === "hours" &&
															nextUnit === "days"
														? Math.round(current / 24)
														: current;

										commitTranscriptionRetentionPolicy({
											mode: "time",
											amount: transcriptionRetentionAmount,
											unit: nextUnit,
											value: nextValue,
										});
									}}
									data={[
										{ label: "Days", value: "days" },
										{ label: "Hours", value: "hours" },
									]}
									disabled={isProfileScope}
									styles={{
										root: {
											backgroundColor: "var(--bg-elevated)",
											border: "1px solid var(--border-default)",
										},
										label: {
											color: "var(--text-primary)",
										},
									}}
								/>
							</>
						)}

						<SegmentedControl
							value={transcriptionRetentionMode}
							onChange={(next) => {
								const mode = next === "time" ? "time" : "amount";
								commitTranscriptionRetentionPolicy({
									mode,
									amount: transcriptionRetentionAmount,
									unit: transcriptionRetentionUnit,
									value: transcriptionRetentionValue,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={{
								root: {
									backgroundColor: "var(--bg-elevated)",
									border: "1px solid var(--border-default)",
								},
								label: {
									color: "var(--text-primary)",
								},
							}}
						/>

						<Checkbox
							checked={transcriptionRetentionDeleteRecordings}
							onChange={(event) =>
								updateTranscriptionRetentionDeleteRecordings.mutate(
									event.currentTarget.checked,
								)
							}
							disabled={
								isProfileScope ||
								(transcriptionRetentionMode === "time"
									? transcriptionRetentionValue === 0
									: transcriptionRetentionAmount <= 0)
							}
							label="Also delete recordings"
							color="gray"
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="Stats retention"
				description="Delete usage/cost stats older than this (0 = forever)."
				right={
					<Group gap={10} align="center" wrap="wrap">
						<NumberInput
							value={statsRetentionValue}
							onChange={(value) => {
								const next = typeof value === "number" ? value : 30;
								commitStatsRetention({
									unit: statsRetentionUnit,
									value: next,
								});
							}}
							min={0}
							max={statsRetentionUnit === "hours" ? 36500 * 24 : 36500}
							step={statsRetentionUnit === "hours" ? 0.5 : 1}
							decimalScale={statsRetentionUnit === "hours" ? 2 : 0}
							clampBehavior="strict"
							disabled={isProfileScope}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									width: 140,
								},
							}}
						/>

						<SegmentedControl
							value={statsRetentionUnit}
							onChange={(next) => {
								const nextUnit =
									next === "hours" ? ("hours" as const) : ("days" as const);

								const current =
									typeof statsRetentionValue === "number"
										? statsRetentionValue
										: 0;

								const nextValue =
									current === 0
										? 0
										: statsRetentionUnit === "days" && nextUnit === "hours"
											? current * 24
											: statsRetentionUnit === "hours" && nextUnit === "days"
												? Math.round(current / 24)
												: current;

								commitStatsRetention({ unit: nextUnit, value: nextValue });
							}}
							data={[
								{ label: "Days", value: "days" },
								{ label: "Hours", value: "hours" },
							]}
							disabled={isProfileScope}
							styles={{
								root: {
									backgroundColor: "var(--bg-elevated)",
									border: "1px solid var(--border-default)",
								},
								label: {
									color: "var(--text-primary)",
								},
							}}
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="Settings backup"
				description="Export/import settings as JSON. API keys and other secrets are not included."
				right={
					<Group gap="xs" wrap="nowrap" justify="flex-end">
						<Button
							variant="default"
							size="xs"
							leftSection={<Download size={14} />}
							loading={exportSettingsBackup.isPending}
							onClick={() => exportSettingsBackup.mutate()}
						>
							Export
						</Button>
						<Button
							variant="default"
							size="xs"
							leftSection={<Upload size={14} />}
							loading={importSettingsBackup.isPending}
							onClick={() => importSettingsBackup.mutate()}
						>
							Import
						</Button>
					</Group>
				}
			/>

			<SettingsRow
				label="GitHub Gist backup"
				description={
					<>
						Push/pull your settings to a private GitHub Gist. Requires a GitHub
						token with the <code>gist</code> scope (stored securely).
					</>
				}
				right={
					<Stack gap={8} style={{ width: "min(640px, 100%)" }}>
						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Text size="xs" c="dimmed">
								Token:{" "}
								{githubBackupHasToken.isLoading
									? "checking"
									: githubBackupHasToken.data
										? "configured"
										: "not configured"}
							</Text>

							<Button
								variant="default"
								size="xs"
								leftSection={<Github size={14} />}
								onClick={() => setGithubTokenModalOpen(true)}
							>
								Set token
							</Button>

							<Button
								variant="default"
								size="xs"
								color="red"
								loading={clearGithubToken.isPending}
								disabled={!githubBackupHasToken.data}
								onClick={() => clearGithubToken.mutate()}
							>
								Clear
							</Button>
						</Group>

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<TextInput
								value={gistIdDraft}
								onChange={(e) => setGistIdDraft(e.currentTarget.value)}
								placeholder="Gist id (optional for first push)"
								size="xs"
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										width: 280,
									},
								}}
							/>

							<Button
								variant="default"
								size="xs"
								loading={saveGistId.isPending}
								onClick={() => {
									const trimmed = (gistIdDraft ?? "").trim();
									saveGistId.mutate(trimmed || null);
								}}
							>
								Save
							</Button>
						</Group>

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Button
								variant="default"
								size="xs"
								leftSection={<Upload size={14} />}
								loading={pushToGist.isPending}
								disabled={!githubBackupHasToken.data}
								onClick={() => pushToGist.mutate()}
							>
								Push
							</Button>

							<Button
								variant="default"
								size="xs"
								leftSection={<Download size={14} />}
								loading={pullFromGist.isPending}
								disabled={!githubBackupHasToken.data}
								onClick={() => pullFromGist.mutate()}
							>
								Pull
							</Button>
						</Group>
					</Stack>
				}
			/>

			<div
				style={{
					marginTop: 16,
					border: "1px solid rgba(239, 68, 68, 0.20)",
					borderRadius: 12,
					padding: 12,
					background: "rgba(239, 68, 68, 0.05)",
				}}
			>
				<div>
					<p
						className="settings-label"
						style={{ color: "rgba(255, 150, 150, 0.95)" }}
					>
						Danger zone
					</p>
					<p className="settings-description">
						Destructive actions (cannot be undone)
					</p>

					<div
						style={{
							marginTop: 8,
							display: "grid",
							gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))",
							gap: 12,
							alignItems: "start",
						}}
					>
						<div>
							{dataStorageSummary.isLoading ? (
								<Text size="xs" c="dimmed">
									Calculating what’s stored…
								</Text>
							) : dataStorageSummary.data ? (
								<div
									style={{
										display: "grid",
										gridTemplateColumns: "auto 1fr",
										gap: "2px 12px",
										alignItems: "baseline",
									}}
								>
									<Text size="xs" c="dimmed">
										Recordings
									</Text>
									<Text size="xs" c="dimmed">
										{dataStorageSummary.data.recordings_count} (
										{formatBytes(dataStorageSummary.data.recordings_bytes)})
									</Text>

									<Text size="xs" c="dimmed">
										Transcriptions
									</Text>
									<Text size="xs" c="dimmed">
										{dataStorageSummary.data.history_count} (
										{formatBytes(dataStorageSummary.data.history_bytes)})
									</Text>

									<Text size="xs" c="dimmed">
										Request logs
									</Text>
									<Text size="xs" c="dimmed">
										{dataStorageSummary.data.request_logs_count}
									</Text>

									<Text size="xs" c="dimmed">
										Usage/cost stats
									</Text>
									<Text size="xs" c="dimmed">
										{dataStorageSummary.data.stats_files_count} files (
										{formatBytes(dataStorageSummary.data.stats_bytes)})
									</Text>

									<Text size="xs" c="dimmed">
										Settings
									</Text>
									<Text size="xs" c="dimmed">
										{formatBytes(dataStorageSummary.data.settings_bytes)}
									</Text>

									<Text size="xs" c="dimmed">
										API keys saved
									</Text>
									<Text size="xs" c="dimmed">
										{apiKeysSavedCount.data ??
											dataStorageSummary.data.api_keys_set_count ??
											0}{" "}
										/ {API_KEY_STORE_KEYS.length}
									</Text>
								</div>
							) : null}
						</div>

						<div
							style={{
								display: "grid",
								gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
								gap: 8,
								width: "min(560px, 100%)",
							}}
						>
							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<Trash2 size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Delete recordings",
										message:
											"This will permanently delete all saved .wav recordings from disk.",
										confirmLabel: "Delete recordings",
										action: async () => {
											await dataAPI.deleteAllRecordings();
										},
									});
								}}
							>
								Delete recordings
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<MessageSquare size={14} />}
								onClick={() => {
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
								}}
							>
								Delete transcriptions
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<FileText size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Delete transcripts (keep recordings)",
										message:
											"This will delete all transcript text from history, but keep your saved .wav recordings.",
										confirmLabel: "Delete transcripts",
										action: async () => {
											await dataAPI.deleteAllTranscriptsKeepRecordings();
										},
									});
								}}
							>
								Delete transcripts
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<FileText size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Clear request logs",
										message:
											"This will clear in-memory request logs shown in the Logs tab.",
										confirmLabel: "Clear logs",
										action: async () => {
											await logsAPI.clearRequestLogs();
										},
									});
								}}
							>
								Clear request logs
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<BarChart2 size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Delete usage/cost stats",
										message:
											"This will permanently delete persisted usage/cost stats (JSONL shards).",
										confirmLabel: "Delete stats",
										action: async () => {
											await dataAPI.deleteAllStats();
										},
									});
								}}
							>
								Delete stats
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<Key size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Delete API keys",
										message:
											"This will remove all stored API keys (OpenAI, Groq, Deepgram, Gemini, Anthropic).",
										confirmLabel: "Delete API keys",
										action: async () => {
											await dataAPI.deleteAllApiKeys();
											await configAPI.syncPipelineConfig();
										},
									});
								}}
							>
								Delete API keys
							</Button>

							<Button
								color="red"
								variant="outline"
								size="xs"
								leftSection={<RotateCcw size={14} />}
								onClick={() => {
									openDangerDialog({
										title: "Reset settings",
										message:
											"This will reset all settings back to defaults (including API keys).",
										confirmLabel: "Reset settings",
										action: async () => {
											await dataAPI.deleteAllSettings();
											await configAPI.syncPipelineConfig();
											await tauriAPI.unregisterShortcuts();
											await tauriAPI.registerShortcuts();
										},
									});
								}}
							>
								Reset settings
							</Button>

							<Button
								color="red"
								variant="filled"
								size="xs"
								leftSection={<Skull size={14} />}
								style={{ gridColumn: "1 / -1" }}
								onClick={() => {
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
											await configAPI.syncPipelineConfig();
											await tauriAPI.unregisterShortcuts();
											await tauriAPI.registerShortcuts();
										},
									});
								}}
							>
								Delete all data
							</Button>
						</div>
					</div>
				</div>
			</div>

			<Modal
				opened={githubTokenModalOpen}
				onClose={() => {
					if (setGithubToken.isPending) return;
					setGithubTokenModalOpen(false);
				}}
				title="GitHub token"
				centered
				size="sm"
			>
				<Text size="sm" mb="md">
					Create a GitHub personal access token with the <code>gist</code>{" "}
					scope. It will be stored securely in your OS credential manager.
				</Text>

				<Group gap="xs" mb="md" wrap="wrap">
					<Button
						variant="subtle"
						size="xs"
						onClick={async () => {
							try {
								await openUrl(
									"https://github.com/settings/personal-access-tokens/new",
								);
							} catch {
								notifications.show({
									title: "Couldn't open link",
									message:
										"Failed to open your browser. You can open the token page manually from GitHub settings.",
									color: "red",
								});
							}
						}}
					>
						Open token creation page
					</Button>

					<Button
						variant="subtle"
						size="xs"
						onClick={async () => {
							try {
								await openUrl(
									"https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens",
								);
							} catch {
								notifications.show({
									title: "Couldn't open link",
									message:
										"Failed to open your browser. You can find GitHub token docs on docs.github.com.",
									color: "red",
								});
							}
						}}
					>
						Docs
					</Button>
				</Group>

				<PasswordInput
					label="Token"
					value={githubTokenDraft}
					onChange={(e) => setGithubTokenDraft(e.currentTarget.value)}
				/>

				<Group justify="flex-end" gap="sm" mt="md">
					<Button
						variant="default"
						disabled={setGithubToken.isPending}
						onClick={() => setGithubTokenModalOpen(false)}
					>
						Cancel
					</Button>
					<Button
						loading={setGithubToken.isPending}
						onClick={() => {
							const token = githubTokenDraft.trim();
							setGithubToken.mutate(token);
						}}
					>
						Save token
					</Button>
				</Group>
			</Modal>

			<Modal
				opened={dangerDialog !== null}
				onClose={() => {
					if (dangerRunning) return;
					setDangerTypedDraft("");
					setDangerDialog(null);
				}}
				title={dangerDialog?.title ?? ""}
				centered
				size="sm"
			>
				<Text size="sm" mb="md">
					{dangerDialog?.message ?? ""}
				</Text>

				{dangerDialog?.typedConfirm ? (
					<TextInput
						label={
							dangerDialog.typedConfirm.label ??
							`Type ${dangerDialog.typedConfirm.requiredText} to confirm`
						}
						placeholder={
							dangerDialog.typedConfirm.placeholder ??
							dangerDialog.typedConfirm.requiredText
						}
						value={dangerTypedDraft}
						onChange={(e) => setDangerTypedDraft(e.currentTarget.value)}
						mb="md"
					/>
				) : null}

				<Text size="xs" c="dimmed" mb="md">
					Tip: if you only want to free up disk space, delete recordings — it's
					the least destructive option.
				</Text>

				<Group justify="flex-end" gap="sm">
					<Button
						variant="default"
						disabled={dangerRunning}
						onClick={() => {
							setDangerTypedDraft("");
							setDangerDialog(null);
						}}
					>
						Cancel
					</Button>
					<Button
						color="red"
						loading={dangerRunning}
						disabled={(() => {
							if (dangerRunning) return true;
							const tc = dangerDialog?.typedConfirm;
							if (!tc) return false;
							return dangerTypedDraft.trim() !== tc.requiredText;
						})()}
						onClick={async () => {
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
					>
						{dangerDialog?.confirmLabel ?? "Confirm"}
					</Button>
				</Group>
			</Modal>
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
