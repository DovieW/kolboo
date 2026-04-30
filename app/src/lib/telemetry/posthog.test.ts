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

const { loadRuntimeConfigMock } = vi.hoisted(() => ({
  loadRuntimeConfigMock: vi.fn(async () => ({
    app_version: "0.2.4-test",
    api_base_url: null,
    managed_inference_gateway_url: null,
    cloudflare_access_client_id: null,
    cloudflare_access_client_secret: null,
    sentry_dsn: null,
    sentry_env: null,
    sentry_release: null,
    posthog_api_key: "phc_test_key",
    posthog_host: "https://us.i.posthog.com",
  })),
}));

vi.mock("../tauri/runtimeConfig", () => ({
	loadRuntimeConfig: loadRuntimeConfigMock,
}));

import { isPosthogConfigured, trackProductEvent } from "./posthog";

describe("posthog telemetry", () => {
	beforeEach(() => {
		storeGetMock.mockReset();
		storeGetMock.mockResolvedValue(true);
		storeLoadMock.mockClear();
		loadRuntimeConfigMock.mockClear();
		loadRuntimeConfigMock.mockResolvedValue({
      app_version: "0.2.4-test",
      api_base_url: null,
      managed_inference_gateway_url: null,
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: "phc_test_key",
      posthog_host: "https://us.i.posthog.com",
    });
		vi.spyOn(globalThis, "fetch").mockResolvedValue(
			new Response(null, { status: 200 }),
		);
	});

	afterEach(() => {
		vi.restoreAllMocks();
		globalThis.localStorage?.clear();
	});

	it("returns configured only when host and key are present", async () => {
		await expect(isPosthogConfigured()).resolves.toBe(true);

		loadRuntimeConfigMock.mockResolvedValueOnce({
      app_version: "0.2.4-test",
      api_base_url: null,
      managed_inference_gateway_url: null,
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: "",
      posthog_host: "https://us.i.posthog.com",
    });
		await expect(isPosthogConfigured()).resolves.toBe(false);
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

	it("captures when consent key is unset (default enabled)", async () => {
		storeGetMock.mockResolvedValue(undefined as unknown as boolean);

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("captures when settings store read fails (default enabled)", async () => {
		storeLoadMock.mockRejectedValueOnce(new Error("store unavailable"));

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("redacts sensitive fields in properties", async () => {
		await trackProductEvent("cloud_sync_action_failed", {
			access_token: "secret-token",
			provider: "groq",
			session_blob: "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyLTEyMyJ9.signature",
		});

		const [, request] = vi.mocked(fetch).mock.calls[0] ?? [];
		const payload = JSON.parse(String((request as RequestInit).body));
		expect(payload.properties.access_token).toBe("[REDACTED]");
		expect(payload.properties.session_blob).toBe("[REDACTED]");
		expect(payload.properties.provider).toBe("groq");
	});
});
