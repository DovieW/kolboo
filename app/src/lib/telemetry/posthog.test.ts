import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { storeGetMock, storeLoadMock } = vi.hoisted(() => {
	const get = vi.fn<() => Promise<boolean>>();
	const load = vi.fn(async () => ({
		get,
	}));

	return {
		storeGetMock: get,
		storeLoadMock: load,
	};
});

vi.mock("@tauri-apps/plugin-store", () => ({
	Store: {
		load: storeLoadMock,
	},
}));

import { isPosthogConfigured, trackProductEvent } from "./posthog";

describe("posthog telemetry", () => {
	beforeEach(() => {
		vi.stubEnv("VITE_POSTHOG_API_KEY", "phc_test_key");
		vi.stubEnv("VITE_POSTHOG_HOST", "https://us.i.posthog.com");
		vi.stubEnv("VITE_APP_VERSION", "0.2.4-test");
		storeGetMock.mockReset();
		storeGetMock.mockResolvedValue(true);
		storeLoadMock.mockClear();
		vi.spyOn(globalThis, "fetch").mockResolvedValue(
			new Response(null, { status: 200 }),
		);
	});

	afterEach(() => {
		vi.unstubAllEnvs();
		vi.restoreAllMocks();
		globalThis.localStorage?.clear();
	});

	it("returns configured only when host and key are present", () => {
		expect(isPosthogConfigured()).toBe(true);

		vi.stubEnv("VITE_POSTHOG_API_KEY", "");
		expect(isPosthogConfigured()).toBe(false);
	});

	it("captures event when configured and consent enabled", async () => {
		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
			count: 2,
		});

		expect(storeLoadMock).toHaveBeenCalledWith("settings.json");
		expect(fetch).toHaveBeenCalledTimes(1);
		const [, request] = vi.mocked(fetch).mock.calls[0] ?? [];
		const payload = JSON.parse(String((request as RequestInit).body));
		expect(payload.api_key).toBe("phc_test_key");
		expect(payload.event).toBe("cloud_sync_action_succeeded");
		expect(payload.properties.action).toBe("push");
		expect(payload.properties.count).toBe(2);
		expect(payload.properties.$lib).toBe("kolboo-desktop");
		expect(payload.properties.$lib_version).toBe("0.2.4-test");
		expect(typeof payload.properties.distinct_id).toBe("string");
	});

	it("does not capture when consent is disabled", async () => {
		storeGetMock.mockResolvedValue(false);

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).not.toHaveBeenCalled();
	});

	it("redacts sensitive fields in properties", async () => {
		await trackProductEvent("cloud_sync_action_failed", {
			access_token: "secret-token",
			provider: "groq",
		});

		const [, request] = vi.mocked(fetch).mock.calls[0] ?? [];
		const payload = JSON.parse(String((request as RequestInit).body));
		expect(payload.properties.access_token).toBe("[REDACTED]");
		expect(payload.properties.provider).toBe("groq");
	});
});
