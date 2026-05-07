import { normalizePresetRoutingHints } from "../../../lib/tauri/presetDefaults";
import type {
	AppSettings,
	RewritePreset,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri/types";
import {
	type PresetRuntimeFallbackViews,
	resolvePresetRuntimeFallbackViews,
} from "./effectivePromptSettings";

export type PresetMetadataEditorState = {
	name: string;
	routingHintsText: string;
};

export type PresetEditorState = PresetMetadataEditorState & {
	runtimeFallbackViews: PresetRuntimeFallbackViews | null;
};

type PresetRuntimeSettings = Partial<
	Pick<
		AppSettings,
		| "stt_provider"
		| "stt_model"
		| "stt_language"
		| "stt_timeout_seconds"
		| "llm_provider"
		| "llm_model"
	>
>;

export const EMPTY_PRESET_METADATA_EDITOR_STATE: PresetMetadataEditorState = {
	name: "",
	routingHintsText: "",
};

/**
 * Convert persisted routing hints into the textarea shape the editor owns.
 */
export function presetRoutingHintsToText(
	routingHints: RewritePreset["routing_hints"],
): string {
	return (routingHints ?? []).join("\n");
}

/**
 * Convert the preset editor's textarea back into normalized routing hints.
 */
export function presetRoutingHintsFromText(text: string): string[] | null {
	return normalizePresetRoutingHints(text.split(/\r?\n/));
}

/**
 * Resolve the local metadata state used by the preset editor.
 */
export function resolvePresetMetadataEditorState(
	preset: RewritePreset | null,
): PresetMetadataEditorState {
	if (!preset) {
		return EMPTY_PRESET_METADATA_EDITOR_STATE;
	}

	return {
		name: preset.name,
		routingHintsText: presetRoutingHintsToText(preset.routing_hints),
	};
}

/**
 * Resolve the full preset editor state, including effective runtime fallback provenance.
 */
export function resolvePresetEditorState({
	profile,
	preset,
	settings,
	defaultSttTimeout,
	defaultSttLanguage,
}: {
	profile: RewriteProgramPromptProfile | null;
	preset: RewritePreset | null;
	settings: PresetRuntimeSettings | undefined;
	defaultSttTimeout: number;
	defaultSttLanguage: string;
}): PresetEditorState {
	const metadata = resolvePresetMetadataEditorState(preset);

	return {
		...metadata,
		runtimeFallbackViews:
			profile && preset
				? resolvePresetRuntimeFallbackViews({
						profile,
						preset,
						settings,
						defaultSttTimeout,
						defaultSttLanguage,
					})
				: null,
	};
}
