import {
	Alert,
	Button,
	Group,
	Loader,
	Modal,
	Select,
	Stack,
	Text,
} from "@mantine/core";
import { useState } from "react";
import {
	modelSelectionKey,
	recordingModelFromSelection,
	recordingModelSelection,
	submitModelChoice,
} from "../lib/modelSelection";
import { useModelChoices } from "../lib/queries/modelChoices";
import {
	normalizeSttLanguage,
	STT_LANGUAGE_OPTIONS,
} from "../lib/sttLanguages";
import type { RecordingPreferences } from "../lib/tauri/types";
import { ModelPicker } from "./ModelPicker";
import { useModelNavigation } from "./ModelNavigation";

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
	const { choices, setupProviders, loading, failedSources, managedMessage, retry } = useModelChoices("meeting");
	const navigation = useModelNavigation();
	const [selection, setSelection] = useState(
		recordingModelSelection(preferences.meeting_model),
	);
	const [language, setLanguage] = useState(preferences.language_hint ?? "auto");
	const selectionKey = selection ? modelSelectionKey(selection) : "";
	const chosen = choices.find(choice => modelSelectionKey(choice.selection) === selectionKey);
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
							.{choices.some(choice => choice.ready) && " You can still choose an available model."}
						</Text>
						<Button
							variant="subtle"
							size="compact-xs"
							mt="xs"
							disabled={
								saving || failedSources.some(({ query }) => query.isFetching)
							}
							onClick={retry}
						>
							Retry loading models
						</Button>
					</Alert>
				)}
				<ModelPicker
					label="Model"
					loading={loading}
					choices={choices}
					setupProviders={setupProviders}
					value={selection}
					onChange={setSelection}
					disabled={saving}
					managedMessage={managedMessage}
					onRetry={retry}
					onSetup={navigation?.setup}
					onManagedAccess={navigation?.account}
				/>
				<Select
					label="Language hint"
					aria-label="Meeting language hint"
					description="Auto-detect unless you choose a hint. Saved with the recording for retries."
					data={STT_LANGUAGE_OPTIONS}
					value={language}
					allowDeselect={false}
					disabled={saving}
					onChange={(value) => setLanguage(normalizeSttLanguage(value))}
				/>
				<Text size="xs" c="dimmed">
					For speaker labels, select GPT-4o Transcribe · speaker labels. Managed
					models must be enabled by Kolboo; other cloud models require your key
					in Settings → Providers.
				</Text>
				{choices.length === 0 && !loading && failedSources.length === 0 && (
					<Text size="sm">
						No models are available. Configure a provider in Settings, or
						refresh your managed access.
					</Text>
				)}
				{selection && !chosen?.ready && !loading && failedSources.length === 0 && (
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
						disabled={!chosen?.ready}
						loading={saving}
						onClick={() => {
							submitModelChoice(choices, selectionKey, saving, model => onSave({
									...preferences,
									language_hint: language,
									meeting_model: recordingModelFromSelection(model),
								}));
						}}
					>
						Save
					</Button>
				</Group>
			</Stack>
		</Modal>
	);
}
