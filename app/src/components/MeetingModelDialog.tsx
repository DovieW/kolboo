import {
	Alert,
	Button,
	Group,
	Loader,
	Modal,
	Stack,
	Text,
} from "@mantine/core";
import { useState } from "react";
import {
	modelSelectionKey,
	recordingModelChoices,
	recordingModelFromSelection,
	recordingModelSelection,
} from "../lib/modelSelection";
import { useTranscriptionModels } from "../lib/queries/transcriptionModels";
import type { RecordingPreferences } from "../lib/tauri/types";
import { ModelPicker } from "./ModelPicker";

export function MeetingModelDialog({
	preferences,
	onSave,
	onClose,
	saving,
	error,
}: {
	preferences: RecordingPreferences;
	onSave: (value: RecordingPreferences) => void;
	onClose: () => void;
	saving: boolean;
	error?: string | null;
}) {
	const { options, loading, failedSources } = useTranscriptionModels();
	const [selection, setSelection] = useState(
		recordingModelSelection(preferences.meeting_model),
	);
	const choices = recordingModelChoices(options.values());
	const chosen = selection
		? choices.find(
				(choice) =>
					modelSelectionKey(choice.selection) === modelSelectionKey(selection),
			)
		: undefined;
	return (
		<Modal
			opened
			onClose={onClose}
			title="Meeting transcription"
			centered
			closeOnEscape={!saving}
			closeOnClickOutside={!saving}
			withCloseButton={!saving}
		>
			<Stack>
				<Text size="sm" c="dimmed">
					Separate from dictation. Transcribes the full recording without
					rewriting.
				</Text>
				{loading && (
					<Group gap="xs" role="status">
						<Loader size="xs" />
						<Text size="sm" c="dimmed">
							Loading models…
						</Text>
					</Group>
				)}
				{failedSources.length > 0 && (
					<Alert color="orange" title="Some models couldn't load">
						<Text size="sm">
							Couldn't load {failedSources.map(({ label }) => label).join(", ")}
							.{options.size > 0 && " You can still choose an available model."}
						</Text>
						<Button
							variant="subtle"
							size="compact-xs"
							mt="xs"
							disabled={
								saving || failedSources.some(({ query }) => query.isFetching)
							}
							onClick={() => {
								void Promise.allSettled(
									failedSources.map(({ query }) => query.refetch()),
								);
							}}
						>
							Retry loading models
						</Button>
					</Alert>
				)}
				<ModelPicker
					label="Model"
					loading={loading}
					choices={choices}
					value={selection}
					onChange={setSelection}
					disabled={saving}
				/>
				<Text size="xs" c="dimmed">
					For speaker labels, select GPT-4o Transcribe · speaker labels. Managed
					models must be enabled by Kolboo; other cloud models require your key
					in Settings → Providers.
				</Text>
				{!options.size && !loading && failedSources.length === 0 && (
					<Text size="sm">
						No models are available. Configure a provider in Settings, or
						refresh your managed access.
					</Text>
				)}
				{selection && !chosen && !loading && failedSources.length === 0 && (
					<Alert color="orange">
						Your saved model is no longer available. Choose another model to
						change this setting; your saved choice has not been replaced.
					</Alert>
				)}
				{error && (
					<Alert color="red" title="Could not save model">
						{error}
					</Alert>
				)}
				<Group justify="flex-end">
					<Button variant="subtle" onClick={onClose} disabled={saving}>
						Cancel
					</Button>
					<Button
						disabled={!chosen}
						loading={saving}
						onClick={() => {
							if (chosen)
								onSave({
									...preferences,
									meeting_model: recordingModelFromSelection(chosen.selection),
								});
						}}
					>
						Save
					</Button>
				</Group>
			</Stack>
		</Modal>
	);
}
