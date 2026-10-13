import { Button, Group, Stack, Switch, Text } from "@mantine/core";
import { useEffect, useState } from "react";
import {
	modelSelectionKey,
	recordingModelFromSelection,
	recordingModelSelection,
} from "../lib/modelSelection";
import { useModelChoices } from "../lib/queries/modelChoices";
import type { RecordingPreferences } from "../lib/tauri/types";
import { ModelPicker } from "./ModelPicker";
import { useModelNavigation } from "./ModelNavigation";

type Selection = RecordingPreferences["meeting_model"];
export function FileModelPicker({
	value,
	onChange,
	onReadyChange,
	disabled,
}: {
	value: Selection;
	onChange: (value: Selection) => void;
	onReadyChange: (ready: boolean) => void;
	disabled: boolean;
}) {
	const { choices, setupProviders, loading, failedSources, managedMessage, retry } = useModelChoices("file");
	const navigation = useModelNavigation();
	const [speakers, setSpeakers] = useState(
		value?.model === "gpt-4o-transcribe-diarize",
	);
	useEffect(() => {
		if (value) setSpeakers(value.model === "gpt-4o-transcribe-diarize");
	}, [value]);
	const selection = recordingModelSelection(value);
	const selected = selection ? modelSelectionKey(selection) : null;
	const available = choices.some(
		(choice) => choice.ready && modelSelectionKey(choice.selection) === selected,
	);
	const models = choices.filter(
		(choice) => choice.capabilities.includes("Speaker labels") === speakers,
	);
	useEffect(() => {
		onReadyChange(available);
	}, [available, onReadyChange]);
	return (
		<Stack gap="sm">
			<Group justify="space-between">
				<Text size="sm" fw={500}>
					Transcription model
				</Text>
				<Switch
					label="Speaker labels"
					checked={speakers}
					disabled={disabled}
					onChange={(event) => {
						setSpeakers(event.currentTarget.checked);
						onChange(null);
					}}
				/>
			</Group>
			<ModelPicker
				label="Transcription model"
				loading={loading}
				choices={models}
				setupProviders={setupProviders}
				value={recordingModelSelection(value)}
				disabled={disabled}
				onChange={(selection) =>
					onChange(recordingModelFromSelection(selection))
				}
				managedMessage={managedMessage}
				onRetry={retry}
				onSetup={navigation?.setup}
				onManagedAccess={navigation?.account}
			/>
			{failedSources.length > 0 ? (
				<Group gap="xs">
					<Text size="xs" c="dimmed">
						Some models couldn’t load.
					</Text>
					<Button
						variant="subtle"
						size="compact-xs"
						disabled={disabled}
						onClick={retry}
					>
						Retry
					</Button>
				</Group>
			) : !loading && models.length === 0 ? (
				<Text size="xs" c="dimmed">
					{speakers
						? "No speaker-label model is available. Add an OpenAI key in Providers or enable a managed model."
						: "Add a provider in Settings to get started."}
				</Text>
			) : null}
			{selected && !loading && !available && (
				<Text size="xs" c="orange">
					This model is no longer available. Choose another.
				</Text>
			)}
		</Stack>
	);
}
