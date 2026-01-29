import fs from "node:fs";
import { describe, expect, it, vi } from "vitest";

import { tauriAPI } from "../tauri";

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

function extractRustSeededSettingsKeys(rustSource: string): string[] {
	// We intentionally keep this parsing dumb + stable.
	// The backend default seeding uses set_default("key", ...) and some direct store.set("key", ...)
	// in ensure_default_settings for migrations.
	const keys = new Set<string>();

	const functionBody = extractRustFunctionBody(
		rustSource,
		"ensure_default_settings",
	);
	const targetSource = functionBody ?? rustSource;

	const patterns = [/set_default\(\s*"([^"]+)"/g, /store\.set\(\s*"([^"]+)"/g];
	for (const re of patterns) {
		for (const match of targetSource.matchAll(re)) {
			const k = match[1];
			if (typeof k === "string" && k.trim().length > 0) {
				keys.add(k);
			}
		}
	}

	return [...keys].sort();
}

function extractRustFunctionBody(
	source: string,
	fnName: string,
): string | null {
	const idx = source.indexOf(`fn ${fnName}`);
	if (idx < 0) return null;
	const braceStart = source.indexOf("{", idx);
	if (braceStart < 0) return null;

	let depth = 0;
	for (let i = braceStart; i < source.length; i += 1) {
		const ch = source[i];
		if (ch === "{") depth += 1;
		if (ch === "}") depth -= 1;
		if (depth === 0) {
			return source.slice(braceStart, i + 1);
		}
	}

	return null;
}

describe("settings contract: Rust defaults vs TS getSettings", () => {
	it("keeps TS getSettings() in sync with backend-seeded settings keys", async () => {
		vi.resetModules();
		currentStore = new FakeStore({});

		const settings = await tauriAPI.getSettings();
		const tsKeys = new Set(Object.keys(settings));

		const rustPath = new URL(
			"../../../src-tauri/src/settings/defaults.rs",
			import.meta.url,
		);
		const rustSource = fs.readFileSync(rustPath, "utf8");
		const rustSeededKeys = extractRustSeededSettingsKeys(rustSource);
		const rustSeededKeySet = new Set(rustSeededKeys);

		// Keys that are intentionally backend-only (or legacy) and not part of the UI settings model.
		// If you add a new backend-seeded setting and the UI should see it, DO NOT add it here.
		const allowMissingFromUi = new Set<string>([
			// Backend pipeline config (stored in settings but not exposed in AppSettings today)
			"vad_settings",
			// Legacy key: replaced by quick_ask_hold_hotkey, but still used as a fallback for older installs
			"quick_ask_hotkey",
			// Legacy key: UI migrated to unit+value but backend still seeds this for compatibility
			"transcription_retention_days",
		]);

		const missingInTs = rustSeededKeys.filter(
			(k) => !tsKeys.has(k) && !allowMissingFromUi.has(k),
		);

		// Keys that are intentionally UI-only (or stored but not seeded by the backend).
		// If you add a new UI setting that should have a backend default, DO NOT add it here.
		const allowMissingFromRust = new Set<string>([
			// UI-only runtime state / device selection
			"selected_mic_id",
			// UI-only presentation settings
			"accent_color",
			"audio_cue",
			// UI prompt editing state (backend falls back to its own defaults)
			"cleanup_prompt_sections",
			// Provider/model selections are optional and can be unset
			"stt_model",
			"llm_provider",
			"llm_model",
			"quick_ask_provider",
			"quick_ask_model",
			// Provider-specific thinking knobs (optional)
			"openai_reasoning_effort",
			"anthropic_thinking_budget",
			"gemini_thinking_budget",
			"gemini_thinking_level",
			"quick_ask_openai_reasoning_effort",
			"quick_ask_anthropic_thinking_budget",
			"quick_ask_gemini_thinking_budget",
			"quick_ask_gemini_thinking_level",
			// Provider-specific base URL
			"aquavoice_base_url",
			// Windows-specific setting (not seeded in Rust defaults yet)
			"windows_clipboard_fallback_for_context_capture",
		]);

		const missingInRust = [...tsKeys].filter(
			(k) => !rustSeededKeySet.has(k) && !allowMissingFromRust.has(k),
		);

		expect(
			missingInTs,
			`Rust seeds settings keys not present in tauriAPI.getSettings(): ${missingInTs.join(
				", ",
			)}`,
		).toEqual([]);

		expect(
			missingInRust,
			`tauriAPI.getSettings() returns keys not seeded in Rust defaults: ${missingInRust.join(
				", ",
			)}`,
		).toEqual([]);
	});
});
