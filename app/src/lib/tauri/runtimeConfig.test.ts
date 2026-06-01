import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const isTauriState = { value: true };

vi.mock("@tauri-apps/api", () => ({
	core: {
		get isTauri() {
			return isTauriState.value;
		},
	},
}));

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
}));

describe("runtime config loading", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.clearAllMocks();
		vi.resetModules();
		isTauriState.value = true;
	});

	it("retries when the first runtime-config invoke fails", async () => {
		invokeMock
			.mockRejectedValueOnce(new Error("bridge not ready"))
			.mockResolvedValueOnce({
				sentry_dsn: "https://dsn.example/123",
				sentry_env: "preview",
				sentry_release: "kolboo@packagerehearsal.2026-05-29-a",
				sentry_smoke: true,
			});

		const { loadRuntimeConfig } = await import("./runtimeConfig");

		const promise = loadRuntimeConfig();
		await vi.runAllTimersAsync();
		const config = await promise;

		expect(invokeMock).toHaveBeenCalledTimes(2);
		expect(config).toMatchObject({
			sentry_dsn: "https://dsn.example/123",
			sentry_env: "preview",
			sentry_release: "kolboo@packagerehearsal.2026-05-29-a",
			sentry_smoke: true,
		});
	});

	it("falls back to defaults after exhausting retries", async () => {
		invokeMock.mockRejectedValue(new Error("still unavailable"));

		const { loadRuntimeConfig } = await import("./runtimeConfig");

		const promise = loadRuntimeConfig();
		await vi.runAllTimersAsync();
		const config = await promise;

		expect(invokeMock).toHaveBeenCalledTimes(50);
		expect(config).toEqual({
			app_version: null,
			api_base_url: null,
			managed_inference_gateway_url: null,
			cloudflare_access_client_id: null,
			cloudflare_access_client_secret: null,
			sentry_dsn: null,
			sentry_env: null,
			sentry_release: null,
			sentry_smoke: null,
			posthog_api_key: null,
			posthog_host: null,
		});
	});

	it("retries again on a later call after a fallback result", async () => {
		invokeMock.mockRejectedValue(new Error("still unavailable"));

		const { loadRuntimeConfig } = await import("./runtimeConfig");

		const firstPromise = loadRuntimeConfig();
		await vi.runAllTimersAsync();
		const firstConfig = await firstPromise;

		expect(firstConfig.sentry_dsn).toBeNull();
		expect(invokeMock).toHaveBeenCalledTimes(50);

		invokeMock.mockReset();
		invokeMock.mockResolvedValueOnce({
			sentry_dsn: "https://dsn.example/retry",
			sentry_env: "preview",
			sentry_release: "kolboo@packagerehearsal.2026-05-29-c",
			sentry_smoke: true,
		});

		const secondPromise = loadRuntimeConfig();
		await vi.runAllTimersAsync();
		const secondConfig = await secondPromise;

		expect(invokeMock).toHaveBeenCalledTimes(1);
		expect(secondConfig).toMatchObject({
			sentry_dsn: "https://dsn.example/retry",
			sentry_env: "preview",
			sentry_release: "kolboo@packagerehearsal.2026-05-29-c",
			sentry_smoke: true,
		});
	});

	it("does not spend time retrying in browser-only mode", async () => {
		isTauriState.value = false;
		invokeMock.mockRejectedValue(new Error("bridge absent"));

		const { loadRuntimeConfig } = await import("./runtimeConfig");

		const promise = loadRuntimeConfig();
		await vi.runAllTimersAsync();
		const config = await promise;

		expect(invokeMock).toHaveBeenCalledTimes(1);
		expect(config.sentry_dsn).toBeNull();
	});
});
