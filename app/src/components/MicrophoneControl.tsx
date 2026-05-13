import {
	ActionIcon,
	Alert,
	Button,
	Group,
	Select,
	Text,
	Tooltip,
} from "@mantine/core";
import { AlertCircle, RotateCcw } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { useMicTestMeter } from "../hooks/useMicTestMeter";
import {
	buildMicSelectorModel,
	canRunMicTest,
	toMicListErrorMessage,
} from "../lib/audioDevices";
import {
	useAudioInputDevices,
	useSettings,
	useUpdateSelectedMic,
} from "../lib/queries";
import { MicLevelMeter } from "./MicLevelMeter";
import { SettingsRow } from "./settings/SettingsRow";

type MicrophoneControlProps = {
	variant?: "settings" | "home";
};

export function MicrophoneControl({
	variant = "settings",
}: MicrophoneControlProps) {
	const { data: settings, isLoading: settingsLoading } = useSettings();
	const audioDevicesQuery = useAudioInputDevices();
	const updateSelectedMic = useUpdateSelectedMic();
	const migratedLegacyMicRef = useRef<string | null>(null);

	const storedMicId = settings?.selected_mic_id ?? null;
	const model = useMemo(
		() =>
			buildMicSelectorModel({
				devices: audioDevicesQuery.data?.devices,
				defaultDeviceName: audioDevicesQuery.data?.defaultDeviceName,
				storedMicId,
			}),
		[
			audioDevicesQuery.data?.defaultDeviceName,
			audioDevicesQuery.data?.devices,
			storedMicId,
		],
	);

	// Keep upgrading legacy name-only selections to stable-ish encoded IDs.
	// Without this, duplicate-name microphones stay confusing every time the UI loads.
	useEffect(() => {
		if (!storedMicId) return;
		if (storedMicId === "default") return;
		if (storedMicId.startsWith("mic:v1:")) return;
		if (!model.legacySelectionTargetId) return;
		if (migratedLegacyMicRef.current === storedMicId) return;

		migratedLegacyMicRef.current = storedMicId;
		updateSelectedMic.mutate(model.legacySelectionTargetId);
	}, [model.legacySelectionTargetId, storedMicId, updateSelectedMic]);

	const listErrorMessage = audioDevicesQuery.isError
		? toMicListErrorMessage(audioDevicesQuery.error)
		: null;
	const hasLoadedDeviceSnapshot = audioDevicesQuery.data !== undefined;
	const shouldBlockForListError =
		Boolean(listErrorMessage) && !hasLoadedDeviceSnapshot;
	const showNoDevicesAlert =
		!settingsLoading &&
		!audioDevicesQuery.isLoading &&
		!audioDevicesQuery.isError &&
		!model.hasAnyDetectedInput;
	const showMissingAlert = model.missingSelected !== null;
	const isSelectDisabled =
		settingsLoading ||
		(audioDevicesQuery.isLoading && !hasLoadedDeviceSnapshot) ||
		updateSelectedMic.isPending ||
		shouldBlockForListError ||
		showNoDevicesAlert;
	const isMicTestBlocked =
		settingsLoading ||
		(audioDevicesQuery.isLoading && !hasLoadedDeviceSnapshot) ||
		shouldBlockForListError ||
		showNoDevicesAlert ||
		!canRunMicTest(model);
	const isMicTestDisabled = isMicTestBlocked || updateSelectedMic.isPending;

	const selectedMicIdForTest =
		model.selectedValue === "default" || model.missingSelected
			? null
			: model.selectedValue;

	const {
		isMicTesting,
		meterLevel,
		meterColor,
		clearMicTestError,
		stopMicTest,
		toggleMicTest,
	} = useMicTestMeter({
		selectedMicId: selectedMicIdForTest,
		disabled: isMicTestBlocked,
	});

	const handleChange = (value: string | null) => {
		clearMicTestError();
		const micId = value === "default" || value === "" ? null : value;
		updateSelectedMic.mutate(micId);
	};

	const handleUseSystemDefault = () => {
		clearMicTestError();
		updateSelectedMic.mutate(null);
	};

	const handleRefresh = async () => {
		clearMicTestError();
		await stopMicTest();
		await audioDevicesQuery.refetch();
	};

	const refreshIconButton = (label = "Refresh microphone list") => (
		<Tooltip label={label} withArrow>
			<ActionIcon
				aria-label={label}
				variant="light"
				color="gray"
				size={32}
				onClick={() => void handleRefresh()}
				disabled={settingsLoading || audioDevicesQuery.isLoading}
				className={
					audioDevicesQuery.isFetching
						? "microphone-refresh-icon microphone-refresh-icon--busy"
						: "microphone-refresh-icon"
				}
			>
				<RotateCcw size={15} />
			</ActionIcon>
		</Tooltip>
	);

	const settingsAlerts =
		variant === "settings"
			? [
					listErrorMessage ? (
						<Alert
							key="list-error"
							icon={<AlertCircle size={16} />}
							color="red"
							title="Couldn't list microphones"
						>
							<Group
								justify="space-between"
								align="center"
								wrap="wrap"
								gap="xs"
							>
								<Text size="sm">{listErrorMessage}</Text>
								<Button
									variant="light"
									size="compact-sm"
									color="red"
									onClick={() => void handleRefresh()}
									loading={audioDevicesQuery.isFetching}
								>
									Retry
								</Button>
							</Group>
						</Alert>
					) : null,
					showNoDevicesAlert ? (
						<Alert
							key="no-devices"
							icon={<AlertCircle size={16} />}
							color="yellow"
							title="No microphones detected"
						>
							<Group
								justify="space-between"
								align="center"
								wrap="wrap"
								gap="xs"
							>
								<Text size="sm">
									Kolboo can’t see any input microphones right now. Plug one in,
									check Windows sound settings or drivers, then refresh.
								</Text>
								{refreshIconButton()}
							</Group>
						</Alert>
					) : null,
					showMissingAlert ? (
						<Alert
							key="missing-device"
							icon={<AlertCircle size={16} />}
							color="yellow"
							title="Saved microphone unavailable"
						>
							<Group
								justify="space-between"
								align="center"
								wrap="wrap"
								gap="xs"
							>
								<Text size="sm">
									Your saved microphone isn’t available, so Kolboo will fall
									back to System Default unless you pick another mic.
								</Text>
								<Group gap="xs">
									<Button
										variant="light"
										size="compact-sm"
										color="yellow"
										onClick={handleUseSystemDefault}
									>
										Use System Default
									</Button>
									{refreshIconButton()}
								</Group>
							</Group>
						</Alert>
					) : null,
				].filter(Boolean)
			: [];

	const controls = (
		<div className="microphone-control-panel">
			<MicLevelMeter
				isActive={isMicTesting}
				level={meterLevel}
				color={meterColor}
				width={variant === "home" ? 132 : 150}
			/>

			<Button
				color="gray"
				variant="default"
				onClick={() => void toggleMicTest()}
				disabled={isMicTestDisabled}
				styles={{
					root: {
						backgroundColor: "var(--bg-elevated)",
						borderColor: "var(--border-default)",
						color: "var(--text-primary)",
						height: 36,
						// Keep Test/Stop at a stable width so the row doesn't shimmy.
						minWidth: 74,
					},
				}}
			>
				{isMicTesting ? "Stop" : "Test"}
			</Button>

			<Tooltip label="Refresh microphones" withArrow>
				<ActionIcon
					aria-label="Refresh microphone list"
					variant="default"
					color="gray"
					size={36}
					onClick={() => void handleRefresh()}
					disabled={settingsLoading || audioDevicesQuery.isLoading}
					className={
						audioDevicesQuery.isFetching
							? "microphone-refresh-icon microphone-refresh-icon--busy"
							: "microphone-refresh-icon"
					}
					styles={{
						root: {
							backgroundColor: "var(--bg-elevated)",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
						},
					}}
				>
					<RotateCcw size={16} />
				</ActionIcon>
			</Tooltip>

			<Select
				data={model.selectData}
				value={model.selectedValue}
				onChange={handleChange}
				allowDeselect={false}
				disabled={isSelectDisabled}
				className="device-selector microphone-control-select"
				withCheckIcon={false}
				styles={{
					input: {
						backgroundColor: "var(--bg-elevated)",
						borderColor: "var(--border-default)",
						color: "var(--text-primary)",
					},
				}}
			/>
		</div>
	);

	if (variant === "home") {
		return (
			<div className="microphone-home-card account-panel">
				<SettingsRow
					noDivider
					className="microphone-row microphone-row--home"
					label="Microphone"
					right={controls}
				/>
			</div>
		);
	}

	return (
		<>
			{settingsAlerts.length > 0 ? (
				<div className="microphone-control-alerts">{settingsAlerts}</div>
			) : null}
			{/*
				Keep the settings variant as a normal direct SettingsRow so it preserves
				the same first-row padding and divider behavior as the rest of Audio.
			*/}
			<SettingsRow
				className="microphone-row microphone-row--settings"
				label="Microphone"
				right={controls}
			/>
		</>
	);
}
