import { Text } from "@mantine/core";
import {
	modelSelectionSettingKey,
	scopedModelSelectionKey,
	resolveModelPreference,
} from "../lib/modelSelection";
import {
	useModelChoices,
	type ModelPurpose,
} from "../lib/queries/modelChoices";
import { useSettings, useUpdateModelSelection } from "../lib/queries/settings";
import {
	getPolicyPathEnforcement,
	type ModelAction,
	type ModelSelection,
} from "../lib/tauri";
import { ModelPicker } from "./ModelPicker";
import { useModelNavigation } from "./ModelNavigation";

/** Shared persistence and readiness behavior for all settings/action surfaces. */
export function ActionModelPicker({
	action,
	label = "Model",
	profileId,
	presetId,
	disabled = false,
	onSelection,
}: {
	action: ModelAction;
	label?: string;
	profileId?: string;
	presetId?: string | null;
	disabled?: boolean;
	onSelection?: (selection: ModelSelection | null) => void;
}) {
	const settings = useSettings();
	const mutation = useUpdateModelSelection();
	const navigation = useModelNavigation();
	const purpose: ModelPurpose =
		action === "transcription" ? "dictation" : "llm";
	const catalog = useModelChoices(purpose);
	const profile =
		profileId &&
		(profileId !== "default" ||
			presetId ||
			action === "quick_replace" ||
			action === "intent_router")
			? settings.data?.rewrite_program_prompt_profiles.find(
					(profile) => profile.id === profileId,
				)
			: null;
	const preset = presetId
		? profile?.presets?.find((preset) => preset.id === presetId)
		: null;
	const preference = resolveModelPreference(
		action,
		settings.data,
		profile,
		preset,
	);
	const scope =
		profileId && (profileId !== "default" || presetId)
			? { profileId, presetId }
			: undefined;
	const path = scope
		? scopedModelSelectionKey(scope.profileId, scope.presetId ?? null, action)
		: modelSelectionSettingKey(action);
	const enforcement = getPolicyPathEnforcement(
		settings.data?.policy_state,
		path,
	);
	const value = preference.selection;
	const legacy = !value && preference.provider && preference.model;
	const legacyModel = legacy
		? catalog.choices.find(
				(choice) =>
					choice.selection.model_id === preference.model &&
					choice.selection.provider_id === preference.provider,
			)
		: undefined;
	const save = (selection: ModelSelection | null) =>
		mutation.mutate(
			{ action, selection, scope },
			{ onSuccess: () => onSelection?.(selection) },
		);
	return (
		<div className="action-model-picker">
			<ModelPicker
				key={path}
				label={label}
				value={value}
				choices={catalog.choices}
				setupProviders={catalog.setupProviders}
				onChange={save}
				legacyLabel={
					legacy
						? `${legacyModel?.displayName ?? preference.model} · ${legacyModel?.providerName ?? preference.provider} · legacy routing`
						: undefined
				}
				disabled={disabled || mutation.isPending || enforcement.enforced}
				loading={catalog.loading}
				managedMessage={catalog.managedMessage}
				onRetry={catalog.retry}
				onSetup={navigation?.setup}
				onManagedAccess={navigation?.account}
				onInherit={() => save(null)}
				clearable={false}
			/>
			{!value && preference.provider && !preference.model && (
				<Text size="xs" c="dimmed">
					Using {preference.provider}'s default model. Choosing a result saves
					its source and exact ID.
				</Text>
			)}
			{legacy && (
				<Text size="xs" c="dimmed">
					This saved model uses your existing routing preferences. Choose a
					result to pin its provider, model and payment source together.
				</Text>
			)}
			{catalog.error && (
				<Text size="xs" c="orange">
					Provider availability could not be loaded. The saved choice is
					unchanged.
				</Text>
			)}
			{mutation.isError && (
				<Text role="alert" size="xs" c="red">
					The model choice could not be applied. Saved settings may have changed;
					retry or check enforced settings.
				</Text>
			)}
			{enforcement.enforced && (
				<Text size="xs" c="dimmed">
					This choice is controlled by organization policy.
				</Text>
			)}
		</div>
	);
}
