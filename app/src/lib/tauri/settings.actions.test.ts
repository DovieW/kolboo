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
const emitTypedMock = vi.fn();
const storeLoadMock = vi.fn(async () => currentStore);

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-store", () => ({
	Store: {
		load: storeLoadMock,
	},
}));

vi.mock("./events", () => ({
	emitTyped: emitTypedMock,
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
		emitTypedMock.mockReset();
		storeLoadMock.mockClear();
	});

	it("updateHotkeyDebugEnabled syncs runtime and emits settings-changed", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateHotkeyDebugEnabled(true);

		expect(invokeMock).toHaveBeenCalledWith(
			"set_hotkey_debug_enabled_runtime",
			{
				enabled: true,
			},
		);
		expect(currentStore.data.get("hotkey_debug_enabled")).toBe(true);
		expect(currentStore.saveCalls).toBe(1);
		expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
			hotkey_debug_enabled: true,
		});
	});

	it("updateOverlayMode persists and tells the backend", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateOverlayMode("always");

		expect(currentStore.data.get("overlay_mode")).toBe("always");
		expect(invokeMock).toHaveBeenCalledWith("set_overlay_mode", {
			mode: "always",
		});
		expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {});
	});

	it("updateAccentColor writes to the store and emits", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateAccentColor("#123456");

		expect(currentStore.data.get("accent_color")).toBe("#123456");
		expect(currentStore.saveCalls).toBe(1);
		expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
			accent_color: "#123456",
		});
	});

	it("updateOverlayMonitorTarget normalizes and repositions widget", async () => {
		vi.resetModules();
		currentStore = new FakeStore({ widget_position: "top-right" });
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateOverlayMonitorTarget("active_window");

		expect(currentStore.data.get("overlay_monitor_target")).toBe(
			"active_window",
		);
		expect(invokeMock).toHaveBeenCalledWith("set_widget_position", {
			position: "top-right",
		});
		expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
			overlay_monitor_target: "active_window",
		});
	});

	it("updateTranscriptionRetention keeps legacy days in sync", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateTranscriptionRetention({
			unit: "days",
			value: 3.6,
		});

		expect(currentStore.data.get("transcription_retention_unit")).toBe("days");
		expect(currentStore.data.get("transcription_retention_value")).toBe(4);
		expect(currentStore.data.get("transcription_retention_days")).toBe(4);
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

		expect(currentStore.data.get("recordings_retention_mode")).toBe("time");
		expect(currentStore.data.get("recordings_retention_amount")).toBe(250);
		expect(currentStore.data.get("recordings_retention_unit")).toBe("hours");
		expect(currentStore.data.get("recordings_retention_value")).toBe(12.5);
		expect(currentStore.saveCalls).toBe(1);
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

		expect(currentStore.data.get("transcription_retention_mode")).toBe("time");
		expect(currentStore.data.get("transcription_retention_amount")).toBe(2000);
		expect(currentStore.data.get("transcription_retention_unit")).toBe("days");
		expect(currentStore.data.get("transcription_retention_value")).toBe(2);
		expect(currentStore.data.get("transcription_retention_days")).toBe(2);
		
		// amount mode disables time retention
		await tauriSettingsAPI.updateTranscriptionRetentionPolicy({
			mode: "amount",
			amount: 100,
			unit: "hours",
			value: 12,
		});

		expect(currentStore.data.get("transcription_retention_mode")).toBe("amount");
		expect(currentStore.data.get("transcription_retention_amount")).toBe(100);
		expect(currentStore.data.get("transcription_retention_value")).toBe(0);
		expect(currentStore.saveCalls).toBe(2);
	});
});
