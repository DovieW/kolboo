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
const emitMock = vi.fn(async () => undefined);

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
	emit: emitMock,
	listen: vi.fn(async () => () => undefined),
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
		emitMock.mockReset();
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

	it("updateOutputSmartPasteProtection patches settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateOutputSmartPasteProtection(true);

		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: { output_smart_paste_protection: true },
			deleteKeys: [],
		});
	});

	it("updateRequestLogsPrivacyMode patches settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateRequestLogsPrivacyMode(true);

		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: { request_logs_privacy_mode: true },
			deleteKeys: [],
		});
	});

	it("updateSTTLanguage patches settings", async () => {
		vi.resetModules();
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateSTTLanguage("es");

		expect(invokeMock).toHaveBeenCalledWith("settings_apply_patch", {
			patch: { stt_language: "es" },
			deleteKeys: [],
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

	it("blocks updates to policy-enforced fields", async () => {
		vi.resetModules();
		currentStore = new FakeStore({
			policy_state: {
				source: "cloud",
				is_valid: true,
				enforced_fields: [
					{
						path: "request_logs_privacy_mode",
						reason: "Managed by organization policy",
					},
				],
			},
		});
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateRequestLogsPrivacyMode(true);

		expect(invokeMock).not.toHaveBeenCalledWith(
			"settings_apply_patch",
			expect.anything(),
		);
		expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
	});

	it("persists normalized policy validity when policy metadata is stale", async () => {
		vi.resetModules();
		currentStore = new FakeStore({
			policy_state: {
				source: "cloud",
				is_valid: true,
				expires_at: "2000-01-01T00:00:00Z",
				enforced_fields: [],
			},
		});
		const { tauriSettingsAPI } = await import("./settings");

		await tauriSettingsAPI.updateSoundEnabled(false);

		expect(invokeMock).toHaveBeenNthCalledWith(1, "settings_apply_patch", {
			patch: expect.objectContaining({
				sound_enabled: false,
				policy_state: expect.objectContaining({
					source: "cloud",
					is_valid: false,
				}),
			}),
			deleteKeys: [],
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, "sync_pipeline_config");
	});

	it("derives per-path policy lock metadata", async () => {
		vi.resetModules();
		const { getPolicyPathEnforcement } = await import("./settings");

		expect(
			getPolicyPathEnforcement(undefined, "request_logs_privacy_mode"),
		).toEqual({
			path: "request_logs_privacy_mode",
			enforced: false,
			reason: null,
		});

		expect(
			getPolicyPathEnforcement(
				{
					source: "cloud",
					is_valid: true,
					last_updated: null,
					expires_at: null,
					version: "1",
					enforced_fields: [
						{
							path: "request_logs_privacy_mode",
							reason: "Managed by organization policy",
						},
					],
				},
				"request_logs_privacy_mode",
			),
		).toEqual({
			path: "request_logs_privacy_mode",
			enforced: true,
			reason: "Managed by organization policy",
		});
	});

	it("unlocks aliased path after policy removal", async () => {
		vi.resetModules();
		const { getPolicyPathEnforcement } = await import("./settings");

		const before = getPolicyPathEnforcement(
			{
				source: "cloud",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: "1",
				enforced_fields: [
					{
						path: "quick_ask_hotkey",
						reason: "Managed by organization policy",
					},
				],
			},
			"quick_ask_hold_hotkey",
		);
		expect(before.enforced).toBe(true);

		const after = getPolicyPathEnforcement(
			{
				source: "none",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: null,
				enforced_fields: [],
			},
			"quick_ask_hold_hotkey",
		);
		expect(after.enforced).toBe(false);
		expect(after.reason).toBeNull();
	});
});
