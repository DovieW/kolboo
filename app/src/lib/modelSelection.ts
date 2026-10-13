import type { ModelAction, ModelSelection, ModelSelections, RecordingPreferences } from "./tauri/types";

export const MODEL_ACTIONS: readonly ModelAction[] = [
	"transcription", "rewrite", "quick_ask", "quick_replace", "intent_router", "history_analysis", "prompt_tools",
];

export function modelSelectionSettingKey(action: ModelAction): string {
	return `model_selection_${action}`;
}

export function scopedModelSelectionKey(profileId: string, presetId: string | null, action: ModelAction): string {
	return `model_selection_scope:${JSON.stringify([presetId ? null : profileId, presetId, action])}`;
}

/** Policy paths are shared by global choices and profile/preset overrides. */
export function modelSelectionPolicyKey(key: string): string | null {
	if (!key.startsWith("model_selection_scope:")) return null;
	let scope: unknown;
	try { scope = JSON.parse(key.slice("model_selection_scope:".length)); }
	catch { throw new Error("Invalid model scope"); }
	if (!Array.isArray(scope) || scope.length !== 3 || (scope[0] !== null && (typeof scope[0] !== "string" || !scope[0])) ||
		(scope[1] !== null && (typeof scope[1] !== "string" || !scope[1])) || !MODEL_ACTIONS.includes(scope[2]) ||
		(scope[0] !== null) === (scope[1] !== null) || scope[0] === "default" || scopedModelSelectionKey(scope[0] ?? "",scope[1],scope[2]) !== key) {
		throw new Error("Invalid model scope");
	}
	return modelSelectionSettingKey(scope[2]);
}

const LEGACY_ACTION_FIELDS: Record<ModelAction, readonly [string, string]> = {
	transcription: ["stt_provider", "stt_model"], rewrite: ["llm_provider", "llm_model"],
	quick_ask: ["quick_ask_provider", "quick_ask_model"], quick_replace: ["quick_replace_provider", "quick_replace_model"],
	history_analysis: ["llm_provider", "llm_model"], prompt_tools: ["llm_provider", "llm_model"],
	intent_router: ["llm_provider", "llm_model"],
};

/** A display/runtime view only: it never writes old IDs back to settings. */
export function resolveModelPreference(action: ModelAction, settings: import("./tauri/types").AppSettings | undefined, profile?: import("./tauri/types").RewriteProgramPromptProfile | null, preset?: import("./tauri/types").RewritePreset | null): {selection: ModelSelection | null; provider: string | null; model: string | null} {
	const fields = LEGACY_ACTION_FIELDS[action];
	const read = (record: object | null | undefined): [string | null, string | null] => {
		const values = action === "intent_router" ? (record as {router?: object} | null)?.router : record;
		const keys = fields;
		const raw = values as Record<string, unknown> | undefined;
		return keys.map(key => typeof raw?.[key] === "string" && (raw[key] as string).trim() ? raw[key] as string : null) as [string | null, string | null];
	};
	const [baseProvider, baseModel] = read(settings);
	const fallback = action !== "transcription" && action !== "rewrite";
	let provider = baseProvider ?? (fallback ? settings?.llm_provider ?? null : null);
	let model = baseModel ?? (fallback ? settings?.llm_model ?? null : null);
	const inheritRewrite = fallback && (action !== "quick_ask" || (!baseProvider && !baseModel));
	let selection = settings?.model_selections?.[action] ?? (inheritRewrite ? settings?.model_selections?.rewrite : null) ?? null;
	for (const layer of [profile, preset]) {
		const [legacyProvider, legacyModel] = layer === preset && action !== "transcription" && action !== "rewrite" ? [null, null] : read(layer);
		provider = legacyProvider ?? provider;
		model = legacyModel ?? model;
		const explicit = layer?.model_selections?.[action] ?? (layer === profile && (action === "history_analysis" || action === "prompt_tools") ? layer?.model_selections?.rewrite : null);
		if (explicit) selection = explicit;
		else if (selection) selection = {...selection, provider_id: legacyProvider ?? selection.provider_id, model_id: legacyModel ?? selection.model_id};
	}
	return selection ? {selection, provider:selection.provider_id, model:selection.model_id} : {selection:null, provider, model};
}

export function normalizeModelSelection(value: unknown): ModelSelection | null {
	if (value == null) return null;
	if (typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid saved model selection");
	const choice = value as Record<string, unknown>;
	if ((choice.source !== "managed" && choice.source !== "byok") ||
		typeof choice.provider_id !== "string" || !choice.provider_id.trim() || new TextEncoder().encode(choice.provider_id).length > 100 ||
		typeof choice.model_id !== "string" || !choice.model_id.trim() || new TextEncoder().encode(choice.model_id).length > 256 ||
		Array.from(choice.provider_id + choice.model_id).some(character => {
			const code = character.charCodeAt(0);
			return code < 32 || (code >= 127 && code < 160);
		}) ||
		Object.keys(choice).some(key => !["source", "provider_id", "model_id"].includes(key))) {
		throw new Error("Invalid saved model selection");
	}
	return {source: choice.source, provider_id: choice.provider_id, model_id: choice.model_id};
}

export function normalizeModelSelections(value: unknown): ModelSelections {
	if (value == null) return {};
	if (typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid saved model selections");
	const selections: ModelSelections = {};
	for (const [action, raw] of Object.entries(value)) {
		if (!MODEL_ACTIONS.includes(action as ModelAction)) throw new Error("Invalid model action");
		const selection = normalizeModelSelection(raw);
		if (!selection) throw new Error("Invalid saved model selection");
		selections[action as ModelAction] = selection;
	}
	return selections;
}

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
