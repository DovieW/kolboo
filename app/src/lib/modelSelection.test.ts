import { expect, it, vi } from "vitest";
import {
	modelSelectionKey,
	recordingModelFromSelection,
	recordingModelSelection,
	submitModelChoice,
	MODEL_ACTIONS,
	modelSelectionSettingKey,
	scopedModelSelectionKey,
	normalizeModelSelection,
	normalizeModelSelections,
	resolveModelPreference,
	modelSelectionPolicyKey,
} from "./modelSelection";
import type { AppSettings, RewriteProgramPromptProfile, RewritePreset } from "./tauri/types";

it("validates atomic routes without rewriting raw IDs or accepting invalid source data", () => {
	expect(new Set(MODEL_ACTIONS.map(modelSelectionSettingKey)).size).toBe(7);
	expect(scopedModelSelectionKey("p::א", "q\"", "rewrite")).toBe('model_selection_scope:[null,"q\\\"","rewrite"]');
	expect(scopedModelSelectionKey("p", null, "quick_ask")).toBe('model_selection_scope:["p",null,"quick_ask"]');
	expect(modelSelectionPolicyKey("llm_provider")).toBeNull();
	expect(modelSelectionPolicyKey(scopedModelSelectionKey("p",null,"rewrite"))).toBe("model_selection_rewrite");
	expect(modelSelectionPolicyKey(scopedModelSelectionKey("default","q","quick_ask"))).toBe("model_selection_quick_ask");
	for (const raw of ['invalid','null','{}','[]','["p",null]','[4,null,"rewrite"]','["",null,"rewrite"]','["p",4,"rewrite"]','[null,"","rewrite"]','["p","q","rewrite"]','[null,null,"rewrite"]','["p",null,"bad"]','["default",null,"rewrite"]','["p", null, "rewrite"]']) expect(()=>modelSelectionPolicyKey(`model_selection_scope:${raw}`)).toThrow("Invalid model scope");
	const selection = {source:"byok",provider_id:" p ",model_id:" raw::א "};
	expect(normalizeModelSelection(selection)).toEqual(selection);
	for (const value of [null,undefined]) {
		expect(normalizeModelSelection(value)).toBeNull();
		expect(normalizeModelSelections(value)).toEqual({});
	}
	for (const value of [1,[],{}, {...selection,source:"wrong"}, {...selection,provider_id:0}, {...selection,provider_id:" "}, {...selection,provider_id:"א".repeat(51)}, {...selection,model_id:0}, {...selection,model_id:" "}, {...selection,model_id:"א".repeat(129)}, {...selection,model_id:"a\u0085"}, {...selection,extra:"secret"}]) {
		expect(() => normalizeModelSelection(value)).toThrow("Invalid saved model selection");
	}
	expect(normalizeModelSelections({rewrite:selection})).toEqual({rewrite:selection});
	for (const value of [1,[],{wrong:selection},{rewrite:null}]) expect(() => normalizeModelSelections(value)).toThrow();
});

it("resolves action/profile/preset choices atomically while preserving inherited source and legacy defaults", () => {
	const global = {source:"byok",provider_id:"custom_a",model_id:"raw"} as const;
	const scoped = {source:"managed",provider_id:"groq",model_id:"published"} as const;
	const settings = {llm_provider:"legacy", llm_model:"legacy-model", stt_provider:"legacy-stt", stt_model:"legacy-stt-model", model_selections:{rewrite:global}} as AppSettings;
	expect(resolveModelPreference("transcription",undefined)).toEqual({selection:null,provider:null,model:null});
	expect(resolveModelPreference("transcription", {stt_provider:8,stt_model:" "} as unknown as AppSettings)).toEqual({selection:null,provider:null,model:null});
	expect(resolveModelPreference("transcription",settings)).toEqual({selection:null,provider:"legacy-stt",model:"legacy-stt-model"});
	for (const action of ["rewrite","history_analysis","prompt_tools","quick_ask","quick_replace","intent_router"] as const) expect(resolveModelPreference(action,settings).selection).toEqual(global);
	const profile = {id:"p",llm_provider:"stale",llm_model:"stale-model",model_selections:{rewrite:scoped}} as RewriteProgramPromptProfile;
	const preset = {id:"q",llm_provider:"groq",llm_model:"preset-model"} as RewritePreset;
	for (const action of ["history_analysis","prompt_tools","rewrite"] as const) expect(resolveModelPreference(action,settings,profile).selection).toEqual(scoped);
	expect(resolveModelPreference("rewrite",settings,profile,preset).selection).toEqual({...scoped,model_id:"preset-model"});
	expect(resolveModelPreference("history_analysis",settings,profile,preset).selection).toEqual(scoped);
	preset.model_selections={rewrite:global};
	expect(resolveModelPreference("rewrite",settings,profile,preset).selection).toEqual(global);
	profile.llm_provider=" "; profile.llm_model=""; profile.model_selections={};
	expect(resolveModelPreference("rewrite",settings,profile).selection).toEqual(global);
	profile.router={llm_provider:"groq",llm_model:"router-model"} as RewriteProgramPromptProfile["router"];
	expect(resolveModelPreference("intent_router",settings,profile).selection).toEqual({...global,provider_id:"groq",model_id:"router-model"});
	settings.quick_ask_provider="legacy-ask";
	expect(resolveModelPreference("quick_ask",settings).selection).toBeNull();
	settings.quick_ask_model="legacy-ask-model";
	expect(resolveModelPreference("quick_ask",settings)).toEqual({selection:null,provider:"legacy-ask",model:"legacy-ask-model"});
	settings.model_selections={quick_ask:scoped};
	expect(resolveModelPreference("quick_ask",settings).selection).toEqual(scoped);
	settings.model_selections={};
	expect(resolveModelPreference("rewrite",settings,undefined,preset).selection).toEqual(global);
	expect(resolveModelPreference("intent_router",settings)).toEqual({selection:null,provider:"legacy",model:"legacy-model"});
});

it("keeps sources and raw IDs distinct, including IDs containing delimiters", () => {
	const model = { provider: "vendor", model: "raw::model", use_managed: true };
	const selection = recordingModelSelection(model);
	expect(selection).toEqual({
		source: "managed",
		provider_id: "vendor",
		model_id: "raw::model",
	});
	expect(recordingModelFromSelection(selection)).toEqual(model);
	expect(recordingModelSelection(null)).toBeNull();
	expect(recordingModelFromSelection(null)).toBeNull();
	expect(
		recordingModelSelection({ provider: "vendor", model: "raw" })?.source,
	).toBe("byok");
	expect(
		recordingModelFromSelection({
			source: "byok",
			provider_id: "vendor",
			model_id: "raw",
		})?.use_managed,
	).toBe(false);
	expect(
		modelSelectionKey({ source: "byok", provider_id: "a::b", model_id: "c" }),
	).not.toBe(
		modelSelectionKey({ source: "byok", provider_id: "a", model_id: "b::c" }),
	);
	expect(
		modelSelectionKey({ source: "byok", provider_id: "a", model_id: "c" }),
	).not.toBe(
		modelSelectionKey({ source: "managed", provider_id: "a", model_id: "c" }),
	);
});

it("commits only current, ready and enabled model choices", () => {
 const choices: import("./modelSelection").ModelChoice[] = [{selection:{source:"byok",provider_id:"vendor",model_id:"raw"},displayName:"Friendly",providerName:"Vendor",capabilities:["After recording"],ready:true}];
	const choice = choices[0];
	if (!choice) throw new Error("Expected fixture choice");
	const commit = vi.fn();
	const key = modelSelectionKey(choice.selection);
	submitModelChoice(choices, "stale", false, commit);
	submitModelChoice(choices, key, true, commit);
	submitModelChoice([{ ...choice, ready: false }], key, false, commit);
	expect(commit).not.toHaveBeenCalled();
	submitModelChoice(choices, key, false, commit);
	expect(commit).toHaveBeenCalledExactlyOnceWith({
		source: "byok",
		provider_id: "vendor",
		model_id: "raw",
	});
});
