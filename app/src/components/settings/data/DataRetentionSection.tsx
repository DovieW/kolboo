import {
	ActionIcon,
	Checkbox,
	Group,
	NumberInput,
	SegmentedControl,
	Tooltip,
} from "@mantine/core";
import { FolderOpen } from "lucide-react";
import {
	describeRecordingsRetention,
	getRetentionTimeInputConfig,
	preserveRetentionDurationOnUnitChange,
	type RecordingsStorageSummary,
	type RequestLogsRetentionMode,
	type RetentionMode,
	type RetentionTimeUnit,
	type RetentionUnit,
	shouldDisableTranscriptionDeleteRecordings,
} from "../../../lib/settings/dataLifecycle";
import type { TranscriptionRetentionUnit } from "../../../lib/tauri";
import { SettingsRow } from "../SettingsRow";

type LogsRetentionControls = {
	mode: RequestLogsRetentionMode;
	amount: number;
	unit: RetentionUnit;
	value: number;
	onCommit: (next: {
		mode: RequestLogsRetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	}) => void;
};

type RecordingsRetentionControls = {
	mode: RetentionMode;
	amount: number;
	unit: RetentionUnit;
	value: number;
	onCommit: (next: {
		mode: RetentionMode;
		amount: number;
		unit: RetentionUnit;
		value: number;
	}) => void;
};

type TranscriptionRetentionControls = {
	mode: RetentionMode;
	amount: number;
	unit: TranscriptionRetentionUnit;
	value: number;
	deleteRecordings: boolean;
	onCommit: (next: {
		mode: RetentionMode;
		amount: number;
		unit: TranscriptionRetentionUnit;
		value: number;
	}) => void;
	onDeleteRecordingsChange: (checked: boolean) => void;
};

type StatsRetentionControls = {
	unit: TranscriptionRetentionUnit;
	value: number;
	onCommit: (next: { unit: TranscriptionRetentionUnit; value: number }) => void;
};

type DataRetentionSectionProps = {
	isProfileScope: boolean;
	logsRetention: LogsRetentionControls;
	recordingsRetention: RecordingsRetentionControls;
	transcriptionRetention: TranscriptionRetentionControls;
	statsRetention: StatsRetentionControls;
	recordingsStatsLoading: boolean;
	recordingsSummary: RecordingsStorageSummary | null;
	onOpenAppLogsFolder: () => void;
	onOpenRecordingsFolder: () => void;
};

const retentionInputStyles = {
	input: {
		backgroundColor: "var(--bg-elevated)",
		borderColor: "var(--border-default)",
		color: "var(--text-primary)",
		width: 140,
	},
} as const;

const segmentedControlStyles = {
	root: {
		backgroundColor: "var(--bg-elevated)",
		border: "1px solid var(--border-default)",
	},
	label: {
		color: "var(--text-primary)",
	},
} as const;

function OpenFolderButton({
	label,
	ariaLabel,
	onClick,
}: {
	label: string;
	ariaLabel: string;
	onClick: () => void;
}) {
	return (
		<Tooltip label={label} withArrow position="top">
			<span>
				<ActionIcon
					variant="default"
					size={36}
					onClick={onClick}
					aria-label={ariaLabel}
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
	);
}

function RetentionTimeControls({
	unit,
	value,
	disabled,
	onValueChange,
	onUnitChange,
}: {
	unit: RetentionTimeUnit;
	value: number;
	disabled: boolean;
	onValueChange: (value: number) => void;
	onUnitChange: (unit: RetentionTimeUnit) => void;
}) {
	const inputConfig = getRetentionTimeInputConfig(unit);

	return (
		<>
			<NumberInput
				value={value}
				onChange={(nextValue) => {
					onValueChange(typeof nextValue === "number" ? nextValue : 0);
				}}
				min={inputConfig.min}
				max={inputConfig.max}
				step={inputConfig.step}
				decimalScale={inputConfig.decimalScale}
				clampBehavior="strict"
				disabled={disabled}
				styles={retentionInputStyles}
			/>

			<SegmentedControl
				value={unit}
				onChange={(next) => {
					const nextUnit: RetentionTimeUnit =
						next === "hours" ? "hours" : "days";
					onUnitChange(nextUnit);
				}}
				data={[
					{ label: "Days", value: "days" },
					{ label: "Hours", value: "hours" },
				]}
				disabled={disabled}
				styles={segmentedControlStyles}
			/>
		</>
	);
}

export function DataRetentionSection({
	isProfileScope,
	logsRetention,
	recordingsRetention,
	transcriptionRetention,
	statsRetention,
	recordingsStatsLoading,
	recordingsSummary,
	onOpenAppLogsFolder,
	onOpenRecordingsFolder,
}: DataRetentionSectionProps) {
	// Keep this section presentational and mutation-agnostic: the parent owns
	// query/mutation wiring, while this file owns the Mantine layout for the
	// data-retention controls.
	return (
		<>
			<SettingsRow
				label="Logs retention"
				description="Keep request logs for debugging. Default: store last 10."
				right={
					<Group gap={10} align="center" wrap="wrap">
						{logsRetention.mode === "amount" ? (
							<NumberInput
								value={logsRetention.amount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 10;
									logsRetention.onCommit({
										mode: "amount",
										amount: nextAmount,
										unit: logsRetention.unit,
										value: logsRetention.value,
									});
								}}
								min={1}
								max={1000}
								step={1}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={retentionInputStyles}
							/>
						) : (
							<RetentionTimeControls
								unit={logsRetention.unit}
								value={logsRetention.value}
								disabled={isProfileScope}
								onValueChange={(nextValue) => {
									logsRetention.onCommit({
										mode: "time",
										amount: logsRetention.amount,
										unit: logsRetention.unit,
										value: nextValue,
									});
								}}
								onUnitChange={(nextUnit) => {
									logsRetention.onCommit({
										mode: "time",
										amount: logsRetention.amount,
										unit: nextUnit,
										value: preserveRetentionDurationOnUnitChange({
											currentUnit: logsRetention.unit,
											nextUnit,
											currentValue: logsRetention.value,
										}),
									});
								}}
							/>
						)}

						<SegmentedControl
							value={logsRetention.mode}
							onChange={(next) => {
								logsRetention.onCommit({
									mode: next === "time" ? "time" : "amount",
									amount: logsRetention.amount,
									unit: logsRetention.unit,
									value: logsRetention.value,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={segmentedControlStyles}
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="App logs"
				description="Daily-rotated trace logs for troubleshooting (7 day retention)."
				right={
					<OpenFolderButton
						label="Open app logs folder"
						ariaLabel="Open app logs folder"
						onClick={onOpenAppLogsFolder}
					/>
				}
			/>

			<SettingsRow
				label="Max recordings to save"
				description={describeRecordingsRetention({
					isLoading: recordingsStatsLoading,
					summary: recordingsSummary,
				})}
				right={
					<Group gap={8} align="center">
						<OpenFolderButton
							label="Open recordings folder"
							ariaLabel="Open recordings folder"
							onClick={onOpenRecordingsFolder}
						/>

						{recordingsRetention.mode === "amount" ? (
							<NumberInput
								value={recordingsRetention.amount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 50;
									recordingsRetention.onCommit({
										mode: "amount",
										amount: nextAmount,
										unit: recordingsRetention.unit,
										value: recordingsRetention.value,
									});
								}}
								min={1}
								max={100000}
								step={10}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={retentionInputStyles}
							/>
						) : (
							<RetentionTimeControls
								unit={recordingsRetention.unit}
								value={recordingsRetention.value}
								disabled={isProfileScope}
								onValueChange={(nextValue) => {
									recordingsRetention.onCommit({
										mode: "time",
										amount: recordingsRetention.amount,
										unit: recordingsRetention.unit,
										value: nextValue,
									});
								}}
								onUnitChange={(nextUnit) => {
									recordingsRetention.onCommit({
										mode: "time",
										amount: recordingsRetention.amount,
										unit: nextUnit,
										value: preserveRetentionDurationOnUnitChange({
											currentUnit: recordingsRetention.unit,
											nextUnit,
											currentValue: recordingsRetention.value,
										}),
									});
								}}
							/>
						)}

						<SegmentedControl
							value={recordingsRetention.mode}
							onChange={(next) => {
								recordingsRetention.onCommit({
									mode: next === "time" ? "time" : "amount",
									amount: recordingsRetention.amount,
									unit: recordingsRetention.unit,
									value: recordingsRetention.value,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={segmentedControlStyles}
						/>
					</Group>
				}
			/>

			<SettingsRow
				label="Transcription retention"
				description="Delete transcriptions older than this (0 = forever)."
				right={
					<Group gap={10} align="center" wrap="wrap">
						{transcriptionRetention.mode === "amount" ? (
							<NumberInput
								value={transcriptionRetention.amount}
								onChange={(value) => {
									const nextAmount = typeof value === "number" ? value : 1000;
									transcriptionRetention.onCommit({
										mode: "amount",
										amount: nextAmount,
										unit: transcriptionRetention.unit,
										value: transcriptionRetention.value,
									});
								}}
								min={1}
								max={100000}
								step={10}
								clampBehavior="strict"
								disabled={isProfileScope}
								styles={retentionInputStyles}
							/>
						) : (
							<RetentionTimeControls
								unit={transcriptionRetention.unit}
								value={transcriptionRetention.value}
								disabled={isProfileScope}
								onValueChange={(nextValue) => {
									transcriptionRetention.onCommit({
										mode: "time",
										amount: transcriptionRetention.amount,
										unit: transcriptionRetention.unit,
										value: nextValue,
									});
								}}
								onUnitChange={(nextUnit) => {
									transcriptionRetention.onCommit({
										mode: "time",
										amount: transcriptionRetention.amount,
										unit: nextUnit,
										value: preserveRetentionDurationOnUnitChange({
											currentUnit: transcriptionRetention.unit,
											nextUnit,
											currentValue: transcriptionRetention.value,
										}),
									});
								}}
							/>
						)}

						<SegmentedControl
							value={transcriptionRetention.mode}
							onChange={(next) => {
								transcriptionRetention.onCommit({
									mode: next === "time" ? "time" : "amount",
									amount: transcriptionRetention.amount,
									unit: transcriptionRetention.unit,
									value: transcriptionRetention.value,
								});
							}}
							data={[
								{ label: "Amount", value: "amount" },
								{ label: "Time", value: "time" },
							]}
							disabled={isProfileScope}
							styles={segmentedControlStyles}
						/>

						<Checkbox
							checked={transcriptionRetention.deleteRecordings}
							onChange={(event) => {
								transcriptionRetention.onDeleteRecordingsChange(
									event.currentTarget.checked,
								);
							}}
							disabled={shouldDisableTranscriptionDeleteRecordings({
								isProfileScope,
								mode: transcriptionRetention.mode,
								amount: transcriptionRetention.amount,
								value: transcriptionRetention.value,
							})}
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
						<RetentionTimeControls
							unit={statsRetention.unit}
							value={statsRetention.value}
							disabled={isProfileScope}
							onValueChange={(nextValue) => {
								statsRetention.onCommit({
									unit: statsRetention.unit,
									value: nextValue,
								});
							}}
							onUnitChange={(nextUnit) => {
								statsRetention.onCommit({
									unit: nextUnit,
									value: preserveRetentionDurationOnUnitChange({
										currentUnit: statsRetention.unit,
										nextUnit,
										currentValue: statsRetention.value,
									}),
								});
							}}
						/>
					</Group>
				}
			/>
		</>
	);
}
