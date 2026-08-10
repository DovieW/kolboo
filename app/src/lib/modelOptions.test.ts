import { describe, expect, it } from "vitest";
import {
	isManagedModelSelection,
	managedChatModelOptions,
	managedTranscriptionModelOptions,
} from "./modelOptions";
import type { ManagedModel } from "./tauri";

const models: ManagedModel[] = [
	{
		id: "whisper-large-v3-turbo",
		display_name: "Whisper Large V3 Turbo",
		provider: "groq",
		capabilities: ["transcription"],
		default_for_provider: true,
	},
	{
		id: "gpt-5-mini",
		display_name: "GPT-5 mini",
		provider: "openai",
		capabilities: ["chat_completions", "responses"],
		default_for_provider: false,
	},
];

describe("managed model options", () => {
	it("keeps transcription and chat capabilities separate", () => {
		expect(managedTranscriptionModelOptions(models, "groq")).toEqual([
			{
				value: "whisper-large-v3-turbo",
				label: "Whisper Large V3 Turbo",
			},
		]);
		expect(managedChatModelOptions(models)).toEqual([
			{ value: "gpt-5-mini", label: "GPT-5 mini · openai" },
		]);
	});

	it("matches managed compatibility by provider, model, and capability", () => {
		expect(
			isManagedModelSelection(
				models,
				"transcription",
				"groq",
				"whisper-large-v3-turbo",
			),
		).toBe(true);
		expect(
			isManagedModelSelection(
				models,
				"transcription",
				"openai",
				"whisper-large-v3-turbo",
			),
		).toBe(false);
	});
});
