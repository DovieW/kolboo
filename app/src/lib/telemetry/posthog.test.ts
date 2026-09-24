import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TELEMETRY_DISCLOSURE_VERSION } from "../settings/telemetryDisclosure";

const { authContextMock } = vi.hoisted(() => ({
	authContextMock: vi.fn(async () => ({
		authenticated: false,
		subject_id: null as string | null,
	})),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: authContextMock }));

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
		app_version: "0.2.4-test" as string | null,
		api_base_url: null,
		managed_inference_gateway_url: null,
		cloudflare_access_client_id: null,
		cloudflare_access_client_secret: null,
		sentry_dsn: null,
		sentry_env: null as string | null,
		sentry_release: null,
		posthog_api_key: "phc_test_key" as string | null,
		posthog_host: "https://us.i.posthog.com" as string | null,
	})),
}));

vi.mock("../tauri/runtimeConfig", () => ({
	loadRuntimeConfig: loadRuntimeConfigMock,
}));

import {
	isPosthogConfigured,
	trackAggregateEvent,
	trackProductEvent,
} from "./posthog";

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
		authContextMock.mockReset();
		authContextMock.mockResolvedValue({
			authenticated: false,
			subject_id: null,
		});
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
		vi.unstubAllGlobals();
		vi.unstubAllEnvs();
		globalThis.localStorage?.clear();
	});

	it("returns configured only when host and key are present", async () => {
		await expect(isPosthogConfigured()).resolves.toBe(true);

		loadRuntimeConfigMock.mockResolvedValue({
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
		loadRuntimeConfigMock.mockResolvedValueOnce({
			...(await loadRuntimeConfigMock()),
			posthog_api_key: null,
		});
		await expect(isPosthogConfigured()).resolves.toBe(false);
		loadRuntimeConfigMock.mockRejectedValueOnce(
			new Error("bridge unavailable"),
		);
		await expect(isPosthogConfigured()).resolves.toBe(false);
	});

	it("accepts only approved ingestion origins", async () => {
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			posthog_host: "https://untrusted.example/capture",
		});
		await expect(isPosthogConfigured()).resolves.toBe(false);
		await trackAggregateEvent("desktop_opened");
		expect(fetch).not.toHaveBeenCalled();
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			posthog_host: null,
		});
		await expect(isPosthogConfigured()).resolves.toBe(false);
	});

	it("uses a fresh cryptographic event ID when the WebView provides one", async () => {
		storeGetMock.mockImplementation(async (key: string) =>
			key === "posthog_analytics_enabled"
				? false
				: key === "telemetry_disclosure_acknowledged_at"
					? "2026-09-24T10:00:00.000Z"
					: key === "telemetry_disclosure_version"
						? TELEMETRY_DISCLOSURE_VERSION
						: undefined,
		);
		const randomUUID = vi
			.fn()
			.mockReturnValueOnce("event-one")
			.mockReturnValueOnce("event-two");
		vi.stubGlobal("crypto", { randomUUID });
		await trackAggregateEvent("desktop_opened");
		await trackAggregateEvent("pipeline_error");
		expect(getPayload(0).distinct_id).toBe("event-one");
		expect(getPayload(1).distinct_id).toBe("event-two");
	});

	it("falls back to a fresh event ID when a WebView crypto shim lacks randomUUID", async () => {
		storeGetMock.mockImplementation(async (key: string) =>
			key === "posthog_analytics_enabled"
				? false
				: key === "telemetry_disclosure_acknowledged_at"
					? "2026-09-24T10:00:00.000Z"
					: key === "telemetry_disclosure_version"
						? TELEMETRY_DISCLOSURE_VERSION
						: undefined,
		);
		vi.stubGlobal("crypto", {});
		await trackAggregateEvent("desktop_opened");
		expect(getPayload().distinct_id).toMatch(/^\d+-[a-f0-9]+$/);
	});

	it("uses the configured EU host without its trailing slash", async () => {
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			posthog_host: "https://eu.i.posthog.com/",
		});
		await trackAggregateEvent("desktop_opened");
		expect(fetch).toHaveBeenCalledWith(
			"https://eu.i.posthog.com/i/v0/e/",
			expect.objectContaining({ method: "POST" }),
		);
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
		expect(payload.properties.count).toBeUndefined();
		expect(payload.properties.$lib).toBe("kolboo-desktop");
		expect(payload.properties.$lib_version).toBe("0.2.4-test");
		expect(typeof payload.distinct_id).toBe("string");
		expect(payload.properties.distinct_id).toBeUndefined();
		expect(payload.properties.$process_person_profile).toBe(false);
		expect(payload.properties.$geoip_disable).toBe(true);
	});

	it("sends unlinked basic counts even when per-installation analytics are off", async () => {
		const setItem = vi.fn();
		vi.stubGlobal("localStorage", {
			getItem: vi.fn(),
			setItem,
			clear: vi.fn(),
		});
		storeGetMock.mockImplementation(async (key: string) => {
			if (key === "posthog_analytics_enabled") return false;
			if (key === "telemetry_disclosure_acknowledged_at")
				return "2026-09-24T10:00:00.000Z";
			if (key === "telemetry_disclosure_version")
				return TELEMETRY_DISCLOSURE_VERSION;
			return undefined;
		});

		await trackAggregateEvent("desktop_opened");
		await trackAggregateEvent("pipeline_transcription_started");

		expect(fetch).toHaveBeenCalledTimes(2);
		expect(getPayload(0).distinct_id).not.toBe(getPayload(1).distinct_id);
		expect(getPayload(0).properties.$process_person_profile).toBe(false);
		expect(getPayload(0).properties.$geoip_disable).toBe(true);
		expect(getPayload(0).event).toBe("desktop_opened");
		expect(setItem).not.toHaveBeenCalled();
		expect(storeGetMock).toHaveBeenCalledWith("posthog_analytics_enabled");
	});

	it("blocks basic counts until the revised disclosure is reviewed", async () => {
		storeGetMock.mockResolvedValue(undefined);
		await trackAggregateEvent("desktop_opened");
		expect(fetch).not.toHaveBeenCalled();
	});

	it("fails closed when disclosure or organization policy cannot be read", async () => {
		storeLoadMock.mockRejectedValueOnce(new Error("store unavailable"));
		await trackAggregateEvent("desktop_opened");
		expect(fetch).not.toHaveBeenCalled();
	});

	it("blocks both streams when organization policy disables analytics", async () => {
		const original = storeGetMock.getMockImplementation();
		storeGetMock.mockImplementation(async (key: string) =>
			key === "policy_effective_values"
				? { disable_product_analytics: true }
				: original?.(key),
		);
		await trackAggregateEvent("desktop_opened");
		await trackProductEvent("cloud_sync_action_succeeded", { action: "push" });
		expect(fetch).not.toHaveBeenCalled();
	});

	it("rejects undeclared events and caller attempts to override reserved fields", async () => {
		await trackAggregateEvent("freeform_user_content" as "desktop_opened");
		await trackProductEvent("freeform_user_content", {});
		expect(fetch).not.toHaveBeenCalled();

		await trackProductEvent("cloud_sync_action_succeeded", {
			action: "push",
			user_id: "private-user-id",
			distinct_id: "attacker-id",
			$process_person_profile: true,
			$geoip_disable: false,
			$lib: "caller-lib",
		});
		const payload = getPayload();
		expect(payload.distinct_id).not.toBe("attacker-id");
		expect(payload.properties.distinct_id).toBeUndefined();
		expect(payload.properties.$process_person_profile).toBe(false);
		expect(payload.properties.$geoip_disable).toBe(true);
		expect(payload.properties.$lib).toBe("kolboo-desktop");
		expect(payload.properties.user_id).toBeUndefined();
	});

	it("keeps the optional installation ID stable across permitted events", async () => {
		const values = new Map<string, string>();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value),
			clear: () => values.clear(),
		});
		await trackProductEvent("cloud_sync_action_succeeded", { action: "push" });
		await trackProductEvent("cloud_sync_action_failed", { action: "pull" });
		expect(getPayload(0).distinct_id).toBe(getPayload(1).distinct_id);
		expect(values.size).toBe(1);
	});

	it("links opted-in events to a signed-in account without sending email or a person profile", async () => {
		authContextMock.mockResolvedValue({
			authenticated: true,
			subject_id: "3ca7fb19-5313-441f-8d42-bacfd190318d",
		});
		await trackAggregateEvent("desktop_opened");
		await trackProductEvent("cloud_sync_action_succeeded", { action: "push" });
		expect(getPayload(0).distinct_id).toBe(
			"kolboo-account:3ca7fb19-5313-441f-8d42-bacfd190318d",
		);
		expect(getPayload(1).distinct_id).toBe(getPayload(0).distinct_id);
		expect(getPayload(0).properties.$process_person_profile).toBe(false);
		expect(JSON.stringify(getPayload(0))).not.toContain("email");
	});

	it("does not use account identity after opt-out", async () => {
		authContextMock.mockResolvedValue({
			authenticated: true,
			subject_id: "3ca7fb19-5313-441f-8d42-bacfd190318d",
		});
		storeGetMock.mockImplementation(async (key: string) =>
			key === "posthog_analytics_enabled"
				? false
				: key === "telemetry_disclosure_acknowledged_at"
					? "2026-09-24T10:00:00.000Z"
					: key === "telemetry_disclosure_version"
						? TELEMETRY_DISCLOSURE_VERSION
						: undefined,
		);
		await trackAggregateEvent("desktop_opened");
		await trackAggregateEvent("page_viewed", { page: "home" });
		expect(getPayload(0).distinct_id).not.toBe(getPayload(1).distinct_id);
		expect(authContextMock).not.toHaveBeenCalled();
	});

	it("marks packaged beta and stable builds separately from local development", async () => {
		vi.stubEnv("DEV", false);
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			app_version: "0.2.5-beta.1",
			sentry_env: "production",
		});
		await trackAggregateEvent("desktop_opened");
		expect(getPayload(0).properties.environment).toBe("beta");
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			app_version: "0.2.5",
			sentry_env: "production",
		});
		await trackAggregateEvent("desktop_opened");
		expect(getPayload(1).properties.environment).toBe("production");
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			sentry_env: "development",
		});
		await trackAggregateEvent("desktop_opened");
		expect(getPayload(2).properties.environment).toBe("development");
		loadRuntimeConfigMock.mockResolvedValue({
			...(await loadRuntimeConfigMock()),
			app_version: null,
			sentry_env: "production",
		});
		await trackAggregateEvent("desktop_opened");
		expect(getPayload(3).properties.environment).toBe("production");
	});

	it("sends only allowlisted page and recording metadata", async () => {
		await trackAggregateEvent("page_viewed", {
			page: "settings",
			private: "secret",
		});
		await trackAggregateEvent("recording_finished", {
			status: "success",
			mode: "meeting",
			duration_seconds: 92.6,
			stt_provider: "openai",
			stt_model: "gpt-4o-transcribe-diarize",
			transcript: "private transcript",
		});
		await trackAggregateEvent("recording_finished", {
			status: "error",
			mode: "private mode",
			duration_seconds: -4,
			stt_provider: "secret-provider",
			stt_model: "sk-secret-key",
		});
		expect(getPayload(0).properties).toMatchObject({ page: "settings" });
		expect(getPayload(0).properties.private).toBeUndefined();
		expect(getPayload(1).properties).toMatchObject({
			status: "success",
			mode: "meeting",
			duration_seconds: 93,
			stt_provider: "openai",
			stt_model: "gpt-4o-transcribe-diarize",
		});
		expect(getPayload(1).properties.transcript).toBeUndefined();
		expect(getPayload(2).properties).toMatchObject({
			status: "error",
			mode: "unknown",
			stt_provider: "other",
			stt_model: "other",
		});
		expect(getPayload(2).properties.duration_seconds).toBeUndefined();
		await trackAggregateEvent("page_viewed", { page: "private-route" });
		await trackAggregateEvent("recording_finished", {
			status: "private failure",
		});
		expect(fetch).toHaveBeenCalledTimes(3);
	});

	it("does not fail optional analytics when local storage is unavailable", async () => {
		vi.stubGlobal("localStorage", {
			getItem: () => {
				throw new Error("unavailable");
			},
			clear: vi.fn(),
		});
		await trackProductEvent("cloud_sync_action_succeeded", { action: "push" });
		expect(getPayload().distinct_id).toMatch(/^kolboo-ephemeral-/);
	});

	it("reports only status when PostHog rejects a capture", async () => {
		const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
		vi.mocked(fetch).mockResolvedValueOnce(new Response(null, { status: 403 }));
		await trackAggregateEvent("desktop_opened");
		expect(warning).toHaveBeenCalledWith("PostHog capture rejected (HTTP 403)");
	});

	it("does not interrupt the app when the analytics request fails", async () => {
		const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
		vi.mocked(fetch).mockRejectedValueOnce(
			new Error("sensitive network detail"),
		);
		await expect(
			trackAggregateEvent("desktop_opened"),
		).resolves.toBeUndefined();
		expect(warning).toHaveBeenCalledWith("PostHog capture request failed");
	});

	it("does not reject an event while runtime configuration is unavailable", async () => {
		loadRuntimeConfigMock.mockRejectedValueOnce(
			new Error("bridge unavailable"),
		);
		await expect(
			trackAggregateEvent("desktop_opened"),
		).resolves.toBeUndefined();
		expect(fetch).not.toHaveBeenCalled();
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

		await trackProductEvent("cloud_sync_action_succeeded");
		state.posthogAnalyticsEnabled = false;
		await trackProductEvent("cloud_sync_action_failed");

		expect(fetch).toHaveBeenCalledTimes(1);
		expect(getPayload().event).toBe("cloud_sync_action_succeeded");
	});

	it("only sends the declared fields and categorical values for optional events", async () => {
		const privateData = {
			account_id: "private-user-id",
			transcript_text: "private transcript",
			access_token: "private-token",
			nested: { prompt: "private prompt" },
		};
		await trackProductEvent("cloud_sync_action_failed", {
			...privateData,
			action: "pull",
			error_kind: "TypeError",
		});
		await trackProductEvent("cloud_sync_action_failed", {
			...privateData,
			action: "private action",
			error_kind: "private failure text",
		});
		await trackProductEvent("cloud_sync_enabled_changed", {
			...privateData,
			enabled: false,
		});
		await trackProductEvent("cloud_sync_auto_push_changed", {
			...privateData,
			enabled: "private value",
		});
		await trackProductEvent("analytics_opted_in", {
			...privateData,
			surface: "settings",
		});
		await trackProductEvent("analytics_opted_in", {
			...privateData,
			surface: "private surface",
		});

		const optionalFields = (index: number) => {
			const {
				$process_person_profile,
				$geoip_disable,
				$lib,
				$lib_version,
				environment,
				...rest
			} = getPayload(index).properties;
			expect(getPayload(index).distinct_id).toBeTruthy();
			expect($process_person_profile).toBe(false);
			expect($geoip_disable).toBe(true);
			expect($lib).toBe("kolboo-desktop");
			expect($lib_version).toBe("0.2.4-test");
			expect(environment).toBe("development");
			return rest;
		};
		expect(optionalFields(0)).toEqual({
			action: "pull",
			error_kind: "TypeError",
		});
		expect(optionalFields(1)).toEqual({ error_kind: "other" });
		expect(optionalFields(2)).toEqual({ enabled: false });
		expect(optionalFields(3)).toEqual({});
		expect(optionalFields(4)).toEqual({ surface: "settings" });
		expect(optionalFields(5)).toEqual({});
	});
});
