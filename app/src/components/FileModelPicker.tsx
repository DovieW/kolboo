import { Button, Group, Select, Stack, Switch, Text } from "@mantine/core";
import { useEffect, useState } from "react";
import { useTranscriptionModels } from "../lib/queries/transcriptionModels";
import type { RecordingPreferences } from "../lib/tauri/types";

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
	const selected = value
		? `${value.provider}::${value.model}::${value.use_managed ? "managed" : "byok"}`
		: null;
	const models = [...options.values()].filter(
		(option) => (option.model === "gpt-4o-transcribe-diarize") === speakers,
	);
	useEffect(() => {
		onReadyChange(selected !== null && options.has(selected));
	}, [selected, options, onReadyChange]);
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
			<Select
				aria-label="Transcription model"
				size="lg"
				searchable
				clearable
				clearButtonProps={{ "aria-label": "Clear transcription model" }}
				placeholder={loading ? "Loading models…" : "Choose a model"}
				nothingFoundMessage="No matching models"
				data={models}
				value={selected}
				disabled={disabled}
				onChange={(key) => {
					const chosen = key ? options.get(key) : undefined;
					onChange(
						chosen
							? {
									provider: chosen.provider,
									model: chosen.model,
									use_managed: chosen.use_managed,
								}
							: null,
					);
				}}
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
			{selected && !loading && !options.has(selected) && (
				<Text size="xs" c="orange">
					This model is no longer available. Choose another.
				</Text>
			)}
		</Stack>
	);
}
