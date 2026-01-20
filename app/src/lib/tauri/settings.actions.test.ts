import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

type StoreLike = {
	get<T = unknown>(key: string): Promise<T | undefined>;
	set<T = unknown>(key: string, value: T): Promise<void>;
	delete(key: string): Promise<void>;
	save(): Promise<void>;
};

class FakeStore implements StoreLike {
	data = new Map<string, unknown>();
	setCalls: Array<{ key: string; value: unknown }> = [];
	deleteCalls: string[] = [];
	saveCalls = 0;

	constructor(seed: Record<string, unknown> = {}) {
		for (const [key, value] of Object.entries(seed)) {
			this.data.set(key, value);
		}
	}

	async get<T = unknown>(key: string): Promise<T | undefined> {
		return this.data.get(key) as T | undefined;
	}

	async set<T = unknown>(key: string, value: T): Promise<void> {
		this.setCalls.push({ key, value });
		this.data.set(key, value);
	}

	async delete(key: string): Promise<void> {
		this.deleteCalls.push(key);
		this.data.delete(key);
	}

	async save(): Promise<void> {
		this.saveCalls += 1;
	}
}

let currentStore: FakeStore;

const invokeMock = vi.fn();
const storeLoadMock = vi.fn(async () => currentStore);

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-store", () => ({
	Store: {
		load: storeLoadMock,
	},
}));

describe("tauri settings side effects", () => {
	beforeAll(() => {
		(globalThis as unknown as { window?: { localStorage?: Storage } }).window =
			{
				localStorage: {
					getItem: vi.fn(),
					setItem: vi.fn(),
					removeItem: vi.fn(),
					clear: vi.fn(),
					key: vi.fn(),
					length: 0,
				},
			};
	});

	beforeEach(() => {
		currentStore = new FakeStore();
		invokeMock.mockReset();
		storeLoadMock.mockClear();
	});

	it("updateHotkeyDebugEnabled syncs runtime and patches settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateHotkeyDebugEnabled(true);

		expect(invokeMock).toHaveBeenNthCalledWith(
			1,
			"set_hotkey_debug_enabled_runtime",
			{ enabled: true },
		);
		expect(invokeMock).toHaveBeenNthCalledWith(2, "settings_apply_patch", {
			patch: { hotkey_debug_enabled: true },
			deleteKeys: [],
		});
	});

	it("updateOverlayMode persists and tells the backend", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateOverlayMode("always");

		expect(invokeMock).toHaveBeenNthCalledWith(1, "settings_apply_patch", {
			patch: { overlay_mode: "always" },
			deleteKeys: [],
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, "set_overlay_mode", {
			mode: "always",
		});
	});

	it("updateAccentColor patches settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateAccentColor("#123456");

		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: { accent_color: "#123456" },
			deleteKeys: [],
		});
	});

	it("updateOverlayMonitorTarget normalizes and repositions widget", async () => {
		vi.resetModules();
		currentStore = new FakeStore({ widget_position: "top-right" });
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateOverlayMonitorTarget("active_window");

		expect(invokeMock).toHaveBeenNthCalledWith(1, "settings_apply_patch", {
			patch: { overlay_monitor_target: "active_window" },
			deleteKeys: [],
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, "set_widget_position", {
			position: "top-right",
		});
	});

	it("updateTranscriptionRetention keeps legacy days in sync", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateTranscriptionRetention({
			unit: "days",
			value: 3.6,
		});
		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: {
				transcription_retention_unit: "days",
				transcription_retention_value: 4,
				transcription_retention_days: 4,
			},
			deleteKeys: [],
		});
	});

	it("updateRecordingsRetention stores retention settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateRecordingsRetention({
			mode: "time",
			amount: 250,
			unit: "hours",
			value: 12.5,
		});
		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: {
				recordings_retention_mode: "time",
				recordings_retention_amount: 250,
				recordings_retention_unit: "hours",
				recordings_retention_value: 12.5,
			},
			deleteKeys: [],
		});
	});

	it("updateTranscriptionRetentionPolicy persists mode and time retention", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateTranscriptionRetentionPolicy({
			mode: "time",
			amount: 2000,
			unit: "days",
			value: 2.2,
		});
		expect(invokeMock).toHaveBeenNthCalledWith(1, "settings_apply_patch", {
			patch: {
				transcription_retention_mode: "time",
				transcription_retention_amount: 2000,
				transcription_retention_unit: "days",
				transcription_retention_value: 2,
				transcription_retention_days: 2,
			},
			deleteKeys: [],
		});

		// amount mode disables time retention
		await tauriSettingsAPI.updateTranscriptionRetentionPolicy({
			mode: "amount",
			amount: 100,
			unit: "hours",
			value: 12,
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, "settings_apply_patch", {
			patch: {
				transcription_retention_mode: "amount",
				transcription_retention_amount: 100,
				transcription_retention_unit: "hours",
				transcription_retention_value: 0,
			},
			deleteKeys: [],
		});
	});
});
