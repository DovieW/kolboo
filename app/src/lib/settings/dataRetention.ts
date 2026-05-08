import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import {
	type AppSettings,
	type TranscriptionRetentionUnit,
	tauriAPI,
} from "../tauri";
import {
	preserveRetentionDurationOnUnitChange,
	type RequestLogsRetentionMode,
	type RetentionMode,
	type RetentionUnit,
	retentionDaysFromUnitValue,
	shouldDisableTranscriptionDeleteRecordings,
} from "./dataLifecycle";

type MaybePromise<T> = T | Promise<T>;

export interface LogsRetentionState {
	mode: RequestLogsRetentionMode;
	amount: number;
	unit: RetentionUnit;
	value: number;
}

export interface RecordingsRetentionState {
	mode: RetentionMode;
	amount: number;
	unit: RetentionUnit;
	value: number;
}

export interface TranscriptionRetentionState {
	mode: RetentionMode;
	amount: number;
	unit: TranscriptionRetentionUnit;
	value: number;
}

export interface StatsRetentionState {
	unit: TranscriptionRetentionUnit;
	value: number;
}

export interface DataRetentionSources {
	logs: LogsRetentionState;
	recordings: RecordingsRetentionState;
	transcription: TranscriptionRetentionState;
	transcriptionDeleteRecordings: boolean;
	stats: StatsRetentionState;
}

export interface LogsRetentionControls extends LogsRetentionState {
	disabled: boolean;
	onModeChange: (mode: RequestLogsRetentionMode) => void;
	onAmountChange: (amount: number) => void;
	onValueChange: (value: number) => void;
	onUnitChange: (unit: RetentionUnit) => void;
}

export interface RecordingsRetentionControls extends RecordingsRetentionState {
	disabled: boolean;
	onModeChange: (mode: RetentionMode) => void;
	onAmountChange: (amount: number) => void;
	onValueChange: (value: number) => void;
	onUnitChange: (unit: RetentionUnit) => void;
}

export interface TranscriptionRetentionControls
	extends TranscriptionRetentionState {
	disabled: boolean;
	deleteRecordings: boolean;
	deleteRecordingsDisabled: boolean;
	onModeChange: (mode: RetentionMode) => void;
	onAmountChange: (amount: number) => void;
	onValueChange: (value: number) => void;
	onUnitChange: (unit: TranscriptionRetentionUnit) => void;
	onDeleteRecordingsChange: (enabled: boolean) => void;
}

export interface StatsRetentionControls extends StatsRetentionState {
	disabled: boolean;
	onValueChange: (value: number) => void;
	onUnitChange: (unit: TranscriptionRetentionUnit) => void;
}

export interface DataRetentionDependencies {
	updateRequestLogsRetention: (params: {
		mode: RequestLogsRetentionMode;
		amount: number;
		days: number;
	}) => Promise<void>;
	updateRecordingsRetention: (
		params: RecordingsRetentionState,
	) => Promise<void>;
	updateMaxSavedRecordings: (max: number) => Promise<void>;
	updateTranscriptionRetentionPolicy: (
		params: TranscriptionRetentionState,
	) => Promise<void>;
	updateTranscriptionRetentionDeleteRecordings: (
		enabled: boolean,
	) => Promise<void>;
	updateStatsRetention: (params: StatsRetentionState) => Promise<void>;
}

export interface DataRetentionEffects {
	onSettingsChanged: () => MaybePromise<void>;
	onRequestLogsChanged: () => MaybePromise<void>;
	onRecordingsChanged: () => MaybePromise<void>;
}

export interface DataRetentionOrchestration {
	logsRetention: LogsRetentionControls;
	recordingsRetention: RecordingsRetentionControls;
	transcriptionRetention: TranscriptionRetentionControls;
	statsRetention: StatsRetentionControls;
}

const defaultDataRetentionDependencies: DataRetentionDependencies = {
	updateRequestLogsRetention: (params) =>
		tauriAPI.updateRequestLogsRetention(params),
	updateRecordingsRetention: (params) =>
		tauriAPI.updateRecordingsRetention(params),
	updateMaxSavedRecordings: (max) => tauriAPI.updateMaxSavedRecordings(max),
	updateTranscriptionRetentionPolicy: (params) =>
		tauriAPI.updateTranscriptionRetentionPolicy(params),
	updateTranscriptionRetentionDeleteRecordings: (enabled) =>
		tauriAPI.updateTranscriptionRetentionDeleteRecordings(enabled),
	updateStatsRetention: (params) => tauriAPI.updateStatsRetention(params),
};

export function readDataRetentionSources(
	settings: Partial<AppSettings> | null | undefined,
): DataRetentionSources {
	return {
		logs: {
			mode: settings?.request_logs_retention_mode ?? "amount",
			amount: settings?.request_logs_retention_amount ?? 10,
			unit: "days",
			value: settings?.request_logs_retention_days ?? 7,
		},
		recordings: {
			mode: settings?.recordings_retention_mode ?? "amount",
			amount:
				settings?.recordings_retention_amount ??
				settings?.max_saved_recordings ??
				50,
			unit: settings?.recordings_retention_unit ?? "days",
			value: settings?.recordings_retention_value ?? 0,
		},
		transcription: {
			mode: settings?.transcription_retention_mode ?? "time",
			amount: settings?.transcription_retention_amount ?? 1000,
			unit: settings?.transcription_retention_unit ?? "days",
			value: settings?.transcription_retention_value ?? 0,
		},
		transcriptionDeleteRecordings:
			settings?.transcription_retention_delete_recordings ?? false,
		stats: {
			unit: settings?.stats_retention_unit ?? "days",
			value: settings?.stats_retention_value ?? 30,
		},
	};
}

export function shouldResetDataRetentionDrafts(
	current: DataRetentionSources,
	next: DataRetentionSources,
): boolean {
	return (
		current.logs.mode !== next.logs.mode ||
		current.logs.amount !== next.logs.amount ||
		current.logs.unit !== next.logs.unit ||
		current.logs.value !== next.logs.value ||
		current.recordings.mode !== next.recordings.mode ||
		current.recordings.amount !== next.recordings.amount ||
		current.recordings.unit !== next.recordings.unit ||
		current.recordings.value !== next.recordings.value ||
		current.transcription.mode !== next.transcription.mode ||
		current.transcription.amount !== next.transcription.amount ||
		current.transcription.unit !== next.transcription.unit ||
		current.transcription.value !== next.transcription.value ||
		current.transcriptionDeleteRecordings !==
			next.transcriptionDeleteRecordings ||
		current.stats.unit !== next.stats.unit ||
		current.stats.value !== next.stats.value
	);
}

function logsRetentionResetKey(source: LogsRetentionState): string {
	return `${source.mode}:${source.amount}:${source.unit}:${source.value}`;
}

function recordingsRetentionResetKey(source: RecordingsRetentionState): string {
	return `${source.mode}:${source.amount}:${source.unit}:${source.value}`;
}

function transcriptionRetentionResetKey(args: {
	source: TranscriptionRetentionState;
	deleteRecordings: boolean;
}): string {
	return `${args.source.mode}:${args.source.amount}:${args.source.unit}:${args.source.value}:${args.deleteRecordings}`;
}

function statsRetentionResetKey(source: StatsRetentionState): string {
	return `${source.unit}:${source.value}`;
}

export async function commitLogsRetention(
	next: LogsRetentionState,
	deps: DataRetentionDependencies,
	effects: DataRetentionEffects,
): Promise<void> {
	await deps.updateRequestLogsRetention({
		mode: next.mode,
		amount: next.amount,
		days: retentionDaysFromUnitValue(next),
	});
	await effects.onSettingsChanged();
	await effects.onRequestLogsChanged();
}

export async function commitRecordingsRetention(
	next: RecordingsRetentionState,
	deps: DataRetentionDependencies,
	effects: DataRetentionEffects,
): Promise<void> {
	await deps.updateRecordingsRetention(next);

	// Keep the legacy key in sync so older builds and untouched call sites still
	// see the expected amount-based retention value.
	if (next.mode === "amount") {
		await deps.updateMaxSavedRecordings(next.amount);
	}

	await effects.onSettingsChanged();
	await effects.onRecordingsChanged();
}

export async function commitTranscriptionRetention(
	next: TranscriptionRetentionState,
	deps: DataRetentionDependencies,
	effects: DataRetentionEffects,
): Promise<void> {
	await deps.updateTranscriptionRetentionPolicy(next);
	await effects.onSettingsChanged();
}

export async function commitTranscriptionDeleteRecordings(
	enabled: boolean,
	deps: DataRetentionDependencies,
	effects: DataRetentionEffects,
): Promise<void> {
	await deps.updateTranscriptionRetentionDeleteRecordings(enabled);
	await effects.onSettingsChanged();
}

export async function commitStatsRetention(
	next: StatsRetentionState,
	deps: DataRetentionDependencies,
	effects: DataRetentionEffects,
): Promise<void> {
	await deps.updateStatsRetention(next);
	await effects.onSettingsChanged();
}

export function buildLogsRetentionControls(args: {
	source: LogsRetentionState;
	draft: LogsRetentionState | null;
	disabled: boolean;
	onCommit: (next: LogsRetentionState) => void;
}): LogsRetentionControls {
	const current = args.draft ?? args.source;

	return {
		...current,
		disabled: args.disabled,
		onModeChange: (mode) => {
			args.onCommit({
				...current,
				mode,
			});
		},
		onAmountChange: (amount) => {
			args.onCommit({
				...current,
				mode: "amount",
				amount,
			});
		},
		onValueChange: (value) => {
			args.onCommit({
				...current,
				mode: "time",
				value,
			});
		},
		onUnitChange: (unit) => {
			args.onCommit({
				...current,
				mode: "time",
				unit,
				value: preserveRetentionDurationOnUnitChange({
					currentUnit: current.unit,
					nextUnit: unit,
					currentValue: current.value,
				}),
			});
		},
	};
}

export function buildRecordingsRetentionControls(args: {
	source: RecordingsRetentionState;
	draft: RecordingsRetentionState | null;
	disabled: boolean;
	onCommit: (next: RecordingsRetentionState) => void;
}): RecordingsRetentionControls {
	const current = args.draft ?? args.source;

	return {
		...current,
		disabled: args.disabled,
		onModeChange: (mode) => {
			args.onCommit({
				...current,
				mode,
			});
		},
		onAmountChange: (amount) => {
			args.onCommit({
				...current,
				mode: "amount",
				amount,
			});
		},
		onValueChange: (value) => {
			args.onCommit({
				...current,
				mode: "time",
				value,
			});
		},
		onUnitChange: (unit) => {
			args.onCommit({
				...current,
				mode: "time",
				unit,
				value: preserveRetentionDurationOnUnitChange({
					currentUnit: current.unit,
					nextUnit: unit,
					currentValue: current.value,
				}),
			});
		},
	};
}

export function buildTranscriptionRetentionControls(args: {
	source: TranscriptionRetentionState;
	draft: TranscriptionRetentionState | null;
	deleteRecordings: boolean;
	disabled: boolean;
	onCommit: (next: TranscriptionRetentionState) => void;
	onDeleteRecordingsChange: (enabled: boolean) => void;
}): TranscriptionRetentionControls {
	const current = args.draft ?? args.source;

	return {
		...current,
		disabled: args.disabled,
		deleteRecordings: args.deleteRecordings,
		deleteRecordingsDisabled: shouldDisableTranscriptionDeleteRecordings({
			isProfileScope: args.disabled,
			mode: current.mode,
			amount: current.amount,
			value: current.value,
		}),
		onModeChange: (mode) => {
			args.onCommit({
				...current,
				mode,
			});
		},
		onAmountChange: (amount) => {
			args.onCommit({
				...current,
				mode: "amount",
				amount,
			});
		},
		onValueChange: (value) => {
			args.onCommit({
				...current,
				mode: "time",
				value,
			});
		},
		onUnitChange: (unit) => {
			args.onCommit({
				...current,
				mode: "time",
				unit,
				value: preserveRetentionDurationOnUnitChange({
					currentUnit: current.unit,
					nextUnit: unit,
					currentValue: current.value,
				}),
			});
		},
		onDeleteRecordingsChange: args.onDeleteRecordingsChange,
	};
}

export function buildStatsRetentionControls(args: {
	source: StatsRetentionState;
	draft: StatsRetentionState | null;
	disabled: boolean;
	onCommit: (next: StatsRetentionState) => void;
}): StatsRetentionControls {
	const current = args.draft ?? args.source;

	return {
		...current,
		disabled: args.disabled,
		onValueChange: (value) => {
			args.onCommit({
				...current,
				value,
			});
		},
		onUnitChange: (unit) => {
			args.onCommit({
				unit,
				value: preserveRetentionDurationOnUnitChange({
					currentUnit: current.unit,
					nextUnit: unit,
					currentValue: current.value,
				}),
			});
		},
	};
}

export function useDataRetentionOrchestration(args: {
	settings: Partial<AppSettings> | null | undefined;
	isProfileScope: boolean;
	effects: DataRetentionEffects;
	deps?: DataRetentionDependencies;
}): DataRetentionOrchestration {
	const {
		settings,
		isProfileScope,
		effects,
		deps = defaultDataRetentionDependencies,
	} = args;
	const sources = readDataRetentionSources(settings);
	const [logsDraft, setLogsDraft] = useState<LogsRetentionState | null>(null);
	const [recordingsDraft, setRecordingsDraft] =
		useState<RecordingsRetentionState | null>(null);
	const [transcriptionDraft, setTranscriptionDraft] =
		useState<TranscriptionRetentionState | null>(null);
	const [statsDraft, setStatsDraft] = useState<StatsRetentionState | null>(
		null,
	);
	const logsResetKey = logsRetentionResetKey(sources.logs);
	const recordingsResetKey = recordingsRetentionResetKey(sources.recordings);
	const transcriptionResetKey = transcriptionRetentionResetKey({
		source: sources.transcription,
		deleteRecordings: sources.transcriptionDeleteRecordings,
	});
	const statsResetKey = statsRetentionResetKey(sources.stats);

	useEffect(() => {
		void logsResetKey;
		setLogsDraft(null);
	}, [logsResetKey]);

	useEffect(() => {
		void recordingsResetKey;
		setRecordingsDraft(null);
	}, [recordingsResetKey]);

	useEffect(() => {
		void transcriptionResetKey;
		setTranscriptionDraft(null);
	}, [transcriptionResetKey]);

	useEffect(() => {
		void statsResetKey;
		setStatsDraft(null);
	}, [statsResetKey]);

	const logsRetentionMutation = useMutation({
		mutationFn: (next: LogsRetentionState) =>
			commitLogsRetention(next, deps, effects),
	});
	const recordingsRetentionMutation = useMutation({
		mutationFn: (next: RecordingsRetentionState) =>
			commitRecordingsRetention(next, deps, effects),
	});
	const transcriptionRetentionMutation = useMutation({
		mutationFn: (next: TranscriptionRetentionState) =>
			commitTranscriptionRetention(next, deps, effects),
	});
	const transcriptionDeleteRecordingsMutation = useMutation({
		mutationFn: (enabled: boolean) =>
			commitTranscriptionDeleteRecordings(enabled, deps, effects),
	});
	const statsRetentionMutation = useMutation({
		mutationFn: (next: StatsRetentionState) =>
			commitStatsRetention(next, deps, effects),
	});

	return {
		logsRetention: buildLogsRetentionControls({
			source: sources.logs,
			draft: logsDraft,
			disabled: isProfileScope,
			onCommit: (next) => {
				setLogsDraft(next);
				logsRetentionMutation.mutate(next);
			},
		}),
		recordingsRetention: buildRecordingsRetentionControls({
			source: sources.recordings,
			draft: recordingsDraft,
			disabled: isProfileScope,
			onCommit: (next) => {
				setRecordingsDraft(next);
				recordingsRetentionMutation.mutate(next);
			},
		}),
		transcriptionRetention: buildTranscriptionRetentionControls({
			source: sources.transcription,
			draft: transcriptionDraft,
			deleteRecordings: sources.transcriptionDeleteRecordings,
			disabled: isProfileScope,
			onCommit: (next) => {
				setTranscriptionDraft(next);
				transcriptionRetentionMutation.mutate(next);
			},
			onDeleteRecordingsChange: (enabled) => {
				transcriptionDeleteRecordingsMutation.mutate(enabled);
			},
		}),
		statsRetention: buildStatsRetentionControls({
			source: sources.stats,
			draft: statsDraft,
			disabled: isProfileScope,
			onCommit: (next) => {
				setStatsDraft(next);
				statsRetentionMutation.mutate(next);
			},
		}),
	};
}
