import { describe, expect, it } from "vitest";
import { normalizePasteShortcut } from "./appBehavior";
import {
	normalizeCleanupPromptSections,
	normalizeCleanupPromptSectionsOverride,
	normalizeRewriteProfile,
} from "./profiles";

describe("profile settings normalizer", () => {
	it.each(["system", "ctrl_v", "ctrl_shift_v", "shift_insert", "cmd_v"])(
		"preserves %s as an explicit paste override",
		(value) => {
			expect(normalizePasteShortcut(value)).toBe(value);
			expect(
				normalizeRewriteProfile({
					id: "terminal",
					name: "Terminal",
					output_paste_shortcut: value,
				})?.output_paste_shortcut,
			).toBe(value);
		},
	);
	it.each([null, undefined, "", "ctrl_a", 42])(
		"inherits when paste override is %s",
		(value) => {
			expect(normalizePasteShortcut(value)).toBeNull();
			expect(
				normalizeRewriteProfile({
					id: "terminal",
					name: "Terminal",
					output_paste_shortcut: value,
				})?.output_paste_shortcut,
			).toBeNull();
		},
	);
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
