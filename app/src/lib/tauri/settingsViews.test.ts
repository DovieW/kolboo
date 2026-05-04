import { describe, expect, it } from "vitest";
import {
	DEFAULT_SETTINGS_VALUES,
	defaultSettingValue,
} from "./settingsDefaults";
import {
	defaultSettingsView,
	findPresetById,
	findProfileById,
	inheritedSettingView,
	isExplicitNull,
	isMissing,
	presetSettingView,
	settingValueView,
} from "./settingsViews";
import type { RewriteProgramPromptProfile } from "./types";

const normalizeBoolean = (value: unknown) =>
	typeof value === "boolean" ? value : null;

const normalizeOverlayMode = (value: unknown) =>
	value === "always" || value === "never" || value === "recording_only"
		? value
		: null;

describe("settings view helpers", () => {
	it("returns canonical defaults with source metadata", () => {
		expect(defaultSettingsView("overlay_mode")).toEqual({
			value: DEFAULT_SETTINGS_VALUES.overlay_mode,
			source: "default",
			explicitNull: false,
		});
		expect(defaultSettingValue("output_mode")).toBe(
			DEFAULT_SETTINGS_VALUES.output_mode,
		);
	});

	it("distinguishes missing, malformed, explicit-null, and stored values", () => {
		const raw = {
			sound_enabled: null,
			overlay_mode: "sideways",
			output_hit_enter: true,
		};

		expect(isExplicitNull(raw, "sound_enabled")).toBe(true);
		expect(isMissing(raw, "missing_key")).toBe(true);
		expect(
			settingValueView({
				record: raw,
				key: "sound_enabled",
				defaultValue: true,
				normalize: normalizeBoolean,
			}),
		).toEqual({ value: true, source: "default", explicitNull: true });
		expect(
			settingValueView({
				record: raw,
				key: "overlay_mode",
				defaultValue: "recording_only" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({
			value: "recording_only",
			source: "default",
			explicitNull: false,
		});
		expect(
			settingValueView({
				record: raw,
				key: "output_hit_enter",
				defaultValue: false,
				normalize: normalizeBoolean,
			}),
		).toEqual({ value: true, source: "stored", explicitNull: false });
		expect(
			settingValueView({
				record: { provider: "groq" },
				key: "provider",
				defaultValue: "openai",
				source: "policy",
			}),
		).toEqual({ value: "groq", source: "policy", explicitNull: false });
		expect(isMissing(null, "sound_enabled")).toBe(true);
		expect(isMissing({ sound_enabled: undefined }, "sound_enabled")).toBe(true);
		expect(
			settingValueView({
				record: "not-an-object",
				key: "sound_enabled",
				defaultValue: true,
				normalize: normalizeBoolean,
			}),
		).toEqual({ value: true, source: "default", explicitNull: false });
	});

	it("resolves profile inheritance from profile to global to default", () => {
		const explicitProfile: RewriteProgramPromptProfile = {
			id: "app",
			name: "App",
			program_paths: [],
			cleanup_prompt_sections: null,
			rewrite_llm_enabled: true,
			overlay_mode: "always",
		};
		const inheritingProfile: RewriteProgramPromptProfile = {
			...explicitProfile,
			id: "inherit",
			overlay_mode: null,
		};

		expect(
			inheritedSettingView({
				globalValue: "recording_only" as const,
				profile: explicitProfile,
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({ value: "always", source: "profile", explicitNull: false });
		expect(
			inheritedSettingView({
				globalValue: "recording_only" as const,
				profile: inheritingProfile,
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({
			value: "recording_only",
			source: "global",
			explicitNull: true,
		});
		expect(
			inheritedSettingView({
				globalValue: null,
				profile: null,
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({ value: "never", source: "default", explicitNull: false });
		expect(
			inheritedSettingView({
				globalValue: "recording_only" as const,
				profile: { ...explicitProfile, overlay_mode: "sideways" as never },
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({
			value: "recording_only",
			source: "global",
			explicitNull: false,
		});
	});

	it("resolves preset inheritance before profile and global values", () => {
		const profile: RewriteProgramPromptProfile = {
			id: "editor",
			name: "Editor",
			program_paths: ["editor.exe"],
			cleanup_prompt_sections: null,
			overlay_mode: "never",
			presets: [
				{
					id: "formal",
					name: "Formal",
					cleanup_prompt_sections: null,
					rewrite_llm_enabled: true,
					overlay_mode: "always",
				},
				{
					id: "inherit",
					name: "Inherit",
					cleanup_prompt_sections: null,
					rewrite_llm_enabled: true,
					overlay_mode: null,
				},
			],
		};

		expect(findProfileById([profile], "editor")?.name).toBe("Editor");
		expect(findProfileById([profile], null)).toBeNull();
		expect(findProfileById([profile], "missing")).toBeNull();
		expect(findPresetById(profile, "formal")?.name).toBe("Formal");
		expect(findPresetById(null, "formal")).toBeNull();
		expect(findPresetById(profile, null)).toBeNull();
		expect(findPresetById(profile, "missing")).toBeNull();
		expect(
			presetSettingView({
				globalValue: "recording_only" as const,
				profile,
				preset: findPresetById(profile, "formal"),
				key: "overlay_mode",
				defaultValue: "recording_only" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({ value: "always", source: "preset", explicitNull: false });
		expect(
			presetSettingView({
				globalValue: "recording_only" as const,
				profile,
				preset: findPresetById(profile, "inherit"),
				key: "overlay_mode",
				defaultValue: "recording_only" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({ value: "never", source: "profile", explicitNull: false });
		expect(
			presetSettingView({
				globalValue: "recording_only" as const,
				profile: null,
				preset: {
					id: "invalid-overlay",
					name: "Invalid overlay",
					cleanup_prompt_sections: null,
					rewrite_llm_enabled: true,
					overlay_mode: "sideways" as never,
				},
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({
			value: "recording_only",
			source: "global",
			explicitNull: false,
		});
		expect(
			presetSettingView({
				globalValue: "recording_only" as const,
				profile: null,
				preset: {
					id: "no-overlay",
					name: "No overlay",
					cleanup_prompt_sections: null,
					rewrite_llm_enabled: true,
				},
				key: "overlay_mode",
				defaultValue: "never" as const,
				normalize: normalizeOverlayMode,
			}),
		).toEqual({
			value: "recording_only",
			source: "global",
			explicitNull: false,
		});
	});
});
