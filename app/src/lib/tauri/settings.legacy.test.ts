import { describe, expect, it, vi } from "vitest";
import legacySettingsFixture from "./__fixtures__/legacy-settings.v0.json";

type StoreLike = {
	get<T = unknown>(key: string): Promise<T | undefined>;
	set<T = unknown>(key: string, value: T): Promise<void>;
	delete(key: string): Promise<void>;
	save(): Promise<void>;
};

class FakeStore implements StoreLike {
	public readonly data = new Map<string, unknown>();
	public saveCalls = 0;

	constructor(initial: Record<string, unknown>) {
		for (const [k, v] of Object.entries(initial)) {
			this.data.set(k, v);
		}
	}

	async get<T = unknown>(key: string): Promise<T | undefined> {
		return this.data.get(key) as T | undefined;
	}

	async set<T = unknown>(key: string, value: T): Promise<void> {
		this.data.set(key, value);
	}

	async delete(key: string): Promise<void> {
		this.data.delete(key);
	}

	async save(): Promise<void> {
		this.saveCalls += 1;
	}
}

let currentStore: FakeStore;

vi.mock("@tauri-apps/api/core", () => ({
	convertFileSrc: (x: string) => x,
	invoke: vi.fn(async () => {
		throw new Error("invoke() not implemented in unit tests");
	}),
}));

vi.mock("@tauri-apps/api/event", () => ({
	emit: vi.fn(async () => {
		// no-op
	}),
	listen: vi.fn(async () => {
		return () => {
			// no-op
		};
	}),
}));

vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: vi.fn(() => ({
		// minimal stub
	})),
}));

vi.mock("@tauri-apps/plugin-store", () => ({
	Store: {
		load: vi.fn(async () => currentStore),
	},
}));

describe("legacy settings fixtures", () => {
	it("normalizes a legacy settings.json shape", async () => {
		vi.resetModules();
		const fixture = JSON.parse(JSON.stringify(legacySettingsFixture)) as Record<
			string,
			unknown
		>;
		currentStore = new FakeStore(fixture);

		const { tauriAPI } = await import("../tauri");
		const settings = await tauriAPI.getSettings();

		expect(settings.settings_version).toBe(1);
		expect(currentStore.data.get("settings_version")).toBe(1);
		expect(settings.cleanup_prompt_sections).toEqual({
			system: { content: "Legacy system prompt" },
		});
		expect(settings.quick_ask_hold_hotkey).toBeNull();
		expect(settings.quick_ask_conversation_history_count).toBe(3);
		expect(settings.overlay_monitor_target).toBe("active_window");
		expect(settings.output_mode).toBe("paste");
		expect(settings.playing_audio_handling).toBe("none");
		expect(settings.overlay_show_detailed_loading).toBe(false);

		expect(settings.rewrite_program_prompt_profiles).toHaveLength(1);
		const profile = settings.rewrite_program_prompt_profiles[0];
		if (!profile) {
			throw new Error("Expected a normalized rewrite profile");
		}

		expect(profile.program_paths).toEqual(["C:\\Program Files\\Foo\\foo.exe"]);
		expect(profile.cleanup_prompt_sections).toBeNull();
		expect(profile.playing_audio_handling).toBe("mute");
		expect(profile.output_mode).toBe("paste");
		expect(profile.output_hit_enter).toBeNull();
		expect(profile.quick_replace_enabled).toBeNull();
		expect(profile.router?.enabled).toBe(true);
		expect(profile.router?.strategy).toBe("off");
		expect(profile.router?.similarity_threshold).toBeNull();

		const preset = profile.presets?.[0];
		if (!preset) {
			throw new Error("Expected a normalized preset");
		}

		expect(preset.rewrite_llm_enabled).toBe(true);
		expect(preset.output_mode).toBe("paste");
		expect(preset.routing_hints).toEqual(["Use this"]);
	});
});
