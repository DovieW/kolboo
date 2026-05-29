import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TELEMETRY_DISCLOSURE_VERSION } from "../settings/telemetryDisclosure";

const { storeGetMock, storeLoadMock } = vi.hoisted(() => {
	const get = vi.fn<(key: string) => Promise<unknown>>();
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

function getPayload(callIndex = 0) {
	const [, request] = vi.mocked(fetch).mock.calls[callIndex] ?? [];
	return JSON.parse(String((request as RequestInit).body));
}

describe("posthog telemetry", () => {
	beforeEach(() => {
		storeGetMock.mockReset();
		storeGetMock.mockImplementation(async (key: string) => {
			switch (key) {
				case "posthog_analytics_enabled":
					return true;
				case "telemetry_disclosure_acknowledged_at":
					return "2026-05-13T00:00:00.000Z";
				case "telemetry_disclosure_version":
					return TELEMETRY_DISCLOSURE_VERSION;
				default:
					return undefined;
			}
		});
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
		const payload = getPayload();
		expect(payload.api_key).toBe("phc_test_key");
		expect(payload.event).toBe("cloud_sync_action_succeeded");
		expect(payload.properties.action).toBe("push");
		expect(payload.properties.count).toBe(2);
		expect(payload.properties.$lib).toBe("kolboo-desktop");
		expect(payload.properties.$lib_version).toBe("0.2.4-test");
		expect(typeof payload.properties.distinct_id).toBe("string");
	});

	it("does not capture when consent is disabled", async () => {
		storeGetMock.mockImplementation(async (key: string) => {
			if (key === "posthog_analytics_enabled") {
				return false;
			}

			if (key === "telemetry_disclosure_acknowledged_at") {
				return "2026-05-13T00:00:00.000Z";
			}

			if (key === "telemetry_disclosure_version") {
				return TELEMETRY_DISCLOSURE_VERSION;
			}

			return undefined;
		});

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).not.toHaveBeenCalled();
	});

	it("does not capture before the disclosure is resolved", async () => {
		storeGetMock.mockImplementation(async (key: string) => {
			if (key === "posthog_analytics_enabled") {
				return undefined;
			}

			if (key === "telemetry_disclosure_acknowledged_at") {
				return null;
			}

			if (key === "telemetry_disclosure_version") {
				return null;
			}

			return undefined;
		});

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).not.toHaveBeenCalled();
	});

	it("captures when consent is unset but the disclosure was accepted", async () => {
		storeGetMock.mockImplementation(async (key: string) => {
			if (key === "posthog_analytics_enabled") {
				return undefined;
			}

			if (key === "telemetry_disclosure_acknowledged_at") {
				return "2026-05-13T00:00:00.000Z";
			}

			if (key === "telemetry_disclosure_version") {
				return TELEMETRY_DISCLOSURE_VERSION;
			}

			return undefined;
		});

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).toHaveBeenCalledTimes(1);
	});

	it("does not capture when settings store read fails", async () => {
		storeLoadMock.mockRejectedValueOnce(new Error("store unavailable"));

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).not.toHaveBeenCalled();
	});

	it("does not capture when the disclosure version is stale", async () => {
		storeGetMock.mockImplementation(async (key: string) => {
			if (key === "posthog_analytics_enabled") {
				return true;
			}

			if (key === "telemetry_disclosure_acknowledged_at") {
				return "2026-05-13T00:00:00.000Z";
			}

			if (key === "telemetry_disclosure_version") {
				return "older-copy";
			}

			return undefined;
		});

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
		});

		expect(fetch).not.toHaveBeenCalled();
	});

	it("reads the current analytics flag on every event so opt-out stops immediately", async () => {
		const state = {
			posthogAnalyticsEnabled: true,
			telemetryDisclosureAcknowledgedAt: "2026-05-13T00:00:00.000Z",
			telemetryDisclosureVersion: TELEMETRY_DISCLOSURE_VERSION,
		};

		storeGetMock.mockImplementation(async (key: string) => {
			switch (key) {
				case "posthog_analytics_enabled":
					return state.posthogAnalyticsEnabled;
				case "telemetry_disclosure_acknowledged_at":
					return state.telemetryDisclosureAcknowledgedAt;
				case "telemetry_disclosure_version":
					return state.telemetryDisclosureVersion;
				default:
					return undefined;
			}
		});

		await trackProductEvent("event_before_opt_out");
		state.posthogAnalyticsEnabled = false;
		await trackProductEvent("event_after_opt_out");

		expect(fetch).toHaveBeenCalledTimes(1);
		expect(getPayload().event).toBe("event_before_opt_out");
	});

	it("redacts sensitive fields across nested objects and arrays", async () => {
		const longNote = "x".repeat(300);
		const samples = Array.from({ length: 25 }, (_, index) => {
			if (index === 0) {
				return { prompt_preview: "secret prompt" };
			}
			if (index === 1) {
				return { clipboard_contents: "copied from clipboard" };
			}
			return `value-${index}`;
		});

		await trackProductEvent("cloud_sync_action_failed", {
			access_token: "secret-token",
			provider: "groq",
			session_blob: "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyLTEyMyJ9.signature",
			transcript_text: "hello world",
			prompt_preview: "rewrite this",
			completion_text: "rewritten text",
			audio_blob: "wav-bytes",
			ocr_payload: "ocr text",
			clipboard_contents: "clipboard text",
			api_key: "phc_real_secret",
			authorization_header: "Bearer very-secret-token",
			safe_status: "Bearer hidden-even-on-safe-key",
			long_note: longNote,
			nested: {
				safe: "ok",
				refresh_token: "refresh-secret",
				deeper: [{ token: "deep-secret" }, { note: "kept" }],
			},
			samples,
		});

		const payload = getPayload();
		expect(payload.properties.access_token).toBe("[REDACTED]");
		expect(payload.properties.session_blob).toBe("[REDACTED]");
		expect(payload.properties.transcript_text).toBe("[REDACTED]");
		expect(payload.properties.prompt_preview).toBe("[REDACTED]");
		expect(payload.properties.completion_text).toBe("[REDACTED]");
		expect(payload.properties.audio_blob).toBe("[REDACTED]");
		expect(payload.properties.ocr_payload).toBe("[REDACTED]");
		expect(payload.properties.clipboard_contents).toBe("[REDACTED]");
		expect(payload.properties.api_key).toBe("[REDACTED]");
		expect(payload.properties.authorization_header).toBe("[REDACTED]");
		expect(payload.properties.safe_status).toBe("[REDACTED]");
		expect(payload.properties.long_note).toBe(`${"x".repeat(256)}…`);
		expect(payload.properties.nested.safe).toBe("ok");
		expect(payload.properties.nested.refresh_token).toBe("[REDACTED]");
		expect(payload.properties.nested.deeper[0].token).toBe("[REDACTED]");
		expect(payload.properties.nested.deeper[1].note).toBe("kept");
		expect(payload.properties.samples).toHaveLength(20);
		expect(payload.properties.samples[0].prompt_preview).toBe("[REDACTED]");
		expect(payload.properties.samples[1].clipboard_contents).toBe("[REDACTED]");
		expect(payload.properties.provider).toBe("groq");
	});
});
