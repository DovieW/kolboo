import { expect, it, vi } from "vitest";
import {
	modelSelectionKey,
	recordingModelChoices,
	recordingModelFromSelection,
	recordingModelSelection,
	submitModelChoice,
} from "./modelSelection";

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

it("adapts legacy choices and commits only ready, enabled, current choices atomically", () => {
	const choices = recordingModelChoices([
		{
			label: "Raw speech",
			provider: "vendor",
			model: "raw",
			use_managed: false,
		},
		{
			label: "Fallback",
			displayName: "Speaker model",
			providerName: "OpenAI",
			provider: "openai",
			model: "gpt-4o-transcribe-diarize",
			use_managed: true,
		},
	]);
	expect(choices[0]).toMatchObject({
		displayName: "Raw speech",
		providerName: "vendor",
		capabilities: ["After recording"],
	});
	expect(choices[1]).toMatchObject({
		displayName: "Speaker model",
		providerName: "OpenAI",
		capabilities: ["After recording", "Speaker labels"],
	});
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
