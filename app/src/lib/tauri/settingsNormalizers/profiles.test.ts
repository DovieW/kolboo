import { describe, expect, it } from "vitest";
import {
	normalizeCleanupPromptSections,
	normalizeCleanupPromptSectionsOverride,
	normalizeRewriteProfile,
} from "./profiles";

describe("profile settings normalizer", () => {
	it("migrates legacy cleanup prompt sections into the new system shape", () => {
		expect(
			normalizeCleanupPromptSections({
				main: "Legacy system prompt",
				advanced: "ignored",
			}),
		).toEqual({
			system: { content: "Legacy system prompt" },
		});

		expect(normalizeCleanupPromptSectionsOverride({ foo: "bar" })).toBeNull();
	});

	it("normalizes legacy profile blobs without losing inherit/null semantics", () => {
		expect(
			normalizeRewriteProfile({
				id: "profile-1",
				name: "Profile One",
				program_path: "C:\\Program Files\\Foo\\foo.exe",
				quick_ask_system_prompt: "   ",
				auto_mute_audio: true,
				output_mode: "keystrokes",
				router: {
					enabled: true,
					strategy: "banana",
				},
			}),
		).toMatchObject({
			id: "profile-1",
			program_paths: ["C:\\Program Files\\Foo\\foo.exe"],
			disabled: false,
			quick_ask_system_prompt: null,
			playing_audio_handling: "mute",
			output_mode: "paste",
			router: {
				enabled: true,
				strategy: "off",
			},
		});
	});
});
