import type { ModelSelection, RecordingPreferences } from "./tauri/types";

export type ModelChoice = {
	selection: ModelSelection;
	displayName: string;
	providerName: string;
	capabilities: string[];
	ready: boolean;
	reason?: string;
	limitations?: string;
	pricing?: string;
};

export function recordingModelChoices(
	options: Iterable<{
		label: string;
		provider: string;
		model: string;
		use_managed: boolean;
		displayName?: string;
		providerName?: string;
	}>,
): ModelChoice[] {
	return [...options].map((option) => ({
		selection: {
			source: option.use_managed ? "managed" : "byok",
			provider_id: option.provider,
			model_id: option.model,
		},
		displayName: option.displayName ?? option.label,
		providerName: option.providerName ?? option.provider,
		capabilities: [
			"After recording",
			...(option.model === "gpt-4o-transcribe-diarize"
				? ["Speaker labels"]
				: []),
		],
		ready: true,
	}));
}

/** Collision-free identity, including the payment source. Never parse model IDs. */
export function modelSelectionKey(selection: ModelSelection): string {
	return JSON.stringify([
		selection.source,
		selection.provider_id,
		selection.model_id,
	]);
}

/** Ignore stale, incompatible and disabled choices without clearing a saved selection. */
export function submitModelChoice(
	choices: ModelChoice[],
	key: string,
	disabled: boolean,
	commit: (selection: ModelSelection) => void,
): void {
	const choice = choices.find(
		(choice) => modelSelectionKey(choice.selection) === key,
	);
	if (choice?.ready && !disabled) commit(choice.selection);
}

export function recordingModelSelection(
	model: RecordingPreferences["meeting_model"],
): ModelSelection | null {
	return model
		? {
				source: model.use_managed ? "managed" : "byok",
				provider_id: model.provider,
				model_id: model.model,
			}
		: null;
}

export function recordingModelFromSelection(
	selection: ModelSelection | null,
): RecordingPreferences["meeting_model"] {
	return selection
		? {
				provider: selection.provider_id,
				model: selection.model_id,
				use_managed: selection.source === "managed",
			}
		: null;
}
