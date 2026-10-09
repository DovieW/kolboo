import { Button, Group, Stack, Switch, Text } from "@mantine/core";
import { useEffect, useState } from "react";
import {
	modelSelectionKey,
	recordingModelChoices,
	recordingModelFromSelection,
	recordingModelSelection,
} from "../lib/modelSelection";
import { useTranscriptionModels } from "../lib/queries/transcriptionModels";
import type { RecordingPreferences } from "../lib/tauri/types";
import { ModelPicker } from "./ModelPicker";

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
	const { options, loading, failedSources } = useTranscriptionModels();
	const [speakers, setSpeakers] = useState(
		value?.model === "gpt-4o-transcribe-diarize",
	);
	useEffect(() => {
		if (value) setSpeakers(value.model === "gpt-4o-transcribe-diarize");
	}, [value]);
	const selection = recordingModelSelection(value);
	const selected = selection ? modelSelectionKey(selection) : null;
	const allChoices = recordingModelChoices(options.values());
	const available = allChoices.some(
		(choice) => modelSelectionKey(choice.selection) === selected,
	);
	const models = [...options.values()].filter(
		(option) => (option.model === "gpt-4o-transcribe-diarize") === speakers,
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
				choices={recordingModelChoices(models)}
				value={recordingModelSelection(value)}
				disabled={disabled}
				onChange={(selection) =>
					onChange(recordingModelFromSelection(selection))
				}
				managedMessage="Managed choices depend on your signed-in account and current access. Your models remain available independently."
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
						onClick={() =>
							void Promise.allSettled(
								failedSources.map((source) => source.query.refetch()),
							)
						}
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
