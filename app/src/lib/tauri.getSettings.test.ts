import { describe, expect, it, vi } from "vitest";

type StoreLike = {
	get<T = unknown>(key: string): Promise<T | undefined>;
	set<T = unknown>(key: string, value: T): Promise<void>;
	delete(key: string): Promise<void>;
	save(): Promise<void>;
};

class FakeStore implements StoreLike {
	public readonly data = new Map<string, unknown>();
	public readonly setCalls: Array<{ key: string; value: unknown }> = [];
	public readonly deleteCalls: string[] = [];
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
		this.setCalls.push({ key, value });
	}

	async delete(key: string): Promise<void> {
		this.data.delete(key);
		this.deleteCalls.push(key);
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

describe("tauriAPI.getSettings() normalization", () => {
	it("migrates legacy cleanup_prompt_sections.main -> cleanup_prompt_sections.system and writes back", async () => {
		vi.resetModules();
		currentStore = new FakeStore({
			cleanup_prompt_sections: {
				main: "Hello legacy prompt",
			},
		});

		const { tauriAPI } = await import("./tauri");
		const settings = await tauriAPI.getSettings();

		expect(settings.cleanup_prompt_sections).toEqual({
			system: { content: "Hello legacy prompt" },
		});

		// Should have cleaned up the legacy shape.
		expect(
			currentStore.setCalls.some(
				(c) =>
					c.key === "cleanup_prompt_sections" &&
					JSON.stringify(c.value) ===
						JSON.stringify({ system: { content: "Hello legacy prompt" } })
			)
		).toBe(true);
		expect(currentStore.saveCalls).toBeGreaterThan(0);
	});

	it("defaults invalid accent_color to DEFAULT_ACCENT_HEX and writes back", async () => {
		vi.resetModules();
		currentStore = new FakeStore({
			accent_color: "not-a-color",
		});

		const { tauriAPI } = await import("./tauri");
		const settings = await tauriAPI.getSettings();

		// Don’t hardcode the hex; import the canonical constant.
		const { DEFAULT_ACCENT_HEX } = await import("./accentColor");
		expect(settings.accent_color).toBe(DEFAULT_ACCENT_HEX);
		expect(
			currentStore.setCalls.some(
				(c) => c.key === "accent_color" && c.value === DEFAULT_ACCENT_HEX
			)
		).toBe(true);
		expect(currentStore.saveCalls).toBeGreaterThan(0);
	});

	it("migrates legacy profile program_path -> program_paths[]", async () => {
		vi.resetModules();
		currentStore = new FakeStore({
			rewrite_program_prompt_profiles: [
				{
					id: "profile-1",
					name: "Profile One",
					program_path: "C:\\Program Files\\Foo\\foo.exe",
				},
			],
		});

		const { tauriAPI } = await import("./tauri");
		const settings = await tauriAPI.getSettings();

		expect(settings.rewrite_program_prompt_profiles).toHaveLength(1);
		expect(settings.rewrite_program_prompt_profiles[0]?.program_paths).toEqual([
			"C:\\Program Files\\Foo\\foo.exe",
		]);
	});
});
