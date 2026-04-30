import { describe, expect, it, vi } from "vitest";
import type { RuntimeConfig } from "./runtimeConfig";

const { invokeMock, loadRuntimeConfigMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  loadRuntimeConfigMock: vi.fn(
    async (): Promise<RuntimeConfig> => ({
      app_version: null,
      api_base_url: null,
      managed_inference_gateway_url: null,
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: null,
      posthog_host: null,
    }),
  ),
}));

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
}));

vi.mock("./runtimeConfig", () => ({
	loadRuntimeConfig: loadRuntimeConfigMock,
}));

import {
	createIdempotencyKey,
	managedInferenceAPI,
	postManagedJson,
} from "./managedInference";

describe("managedInference", () => {
	it("attaches bearer auth for relative managed gateway requests", async () => {
		loadRuntimeConfigMock.mockResolvedValueOnce({
      app_version: null,
      api_base_url: null,
      managed_inference_gateway_url: "https://gateway.example/",
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: null,
      posthog_host: null,
    });
		invokeMock.mockImplementation(async (command) => {
			if (command === "license_get_session_access_token") {
				return "session-token";
			}
			throw new Error(`Unexpected invoke command: ${command}`);
		});

		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			json: async () => ({ ok: true }),
		});
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			postManagedJson({
				path: "/v1/stt/transcribe",
				request: { foo: "bar" },
				idempotencyKey: "idem-auth",
			}),
		).resolves.toEqual({ ok: true });

		expect(fetchMock).toHaveBeenCalledTimes(1);
		const [url, init] = fetchMock.mock.calls[0] as [
			string,
			{ headers: Record<string, string> },
		];
		expect(url).toBe("https://gateway.example/v1/stt/transcribe");
		expect(init.headers.authorization).toBe("Bearer session-token");
		expect(init.headers["x-idempotency-key"]).toBe("idem-auth");
	});

	it("attaches Cloudflare Access headers only for configured edge hosts", async () => {
    loadRuntimeConfigMock.mockResolvedValueOnce({
      app_version: null,
      api_base_url: "https://kolboo.dovie.dev",
      managed_inference_gateway_url: "https://kolboo.dovie.dev/",
      cloudflare_access_client_id: "cf-client-id",
      cloudflare_access_client_secret: "cf-client-secret",
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: null,
      posthog_host: null,
    });
    invokeMock.mockImplementation(async (command) => {
      if (command === "license_get_session_access_token") {
        return "session-token";
      }
      throw new Error(`Unexpected invoke command: ${command}`);
    });

    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ ok: true }),
    });
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      postManagedJson({
        path: "/v1/llm/complete",
        request: { foo: "bar" },
        idempotencyKey: "idem-cf",
      }),
    ).resolves.toEqual({ ok: true });

    const [, init] = fetchMock.mock.calls[0] as [
      string,
      { headers: Record<string, string> },
    ];
    expect(init.headers["CF-Access-Client-Id"]).toBe("cf-client-id");
    expect(init.headers["CF-Access-Client-Secret"]).toBe("cf-client-secret");
  });

	it("createIdempotencyKey creates prefixed unique keys", () => {
		const a = createIdempotencyKey("test");
		const b = createIdempotencyKey("test");
		expect(a.startsWith("test-")).toBe(true);
		expect(b.startsWith("test-")).toBe(true);
		expect(a).not.toBe(b);
	});

	it("postManagedJson sends idempotency header", async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: true,
			json: async () => ({ ok: true }),
		});
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			postManagedJson({
				path: "http://example.test/v1/stt/transcribe",
				request: { foo: "bar" },
				idempotencyKey: "idem-123",
			}),
		).resolves.toEqual({ ok: true });

		expect(fetchMock).toHaveBeenCalledTimes(1);
		const [, init] = fetchMock.mock.calls[0] as [
			string,
			{ headers: Record<string, string> },
		];
		expect(init.headers["x-idempotency-key"]).toBe("idem-123");
	});

	it("maps quota HTTP errors to deterministic managed category", async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: false,
			status: 429,
			statusText: "Too Many Requests",
			json: async () => ({ code: "OVER_QUOTA", message: "limit hit" }),
		});
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			postManagedJson({
				path: "http://example.test/v1/llm/complete",
				request: { foo: "bar" },
				idempotencyKey: "idem-429",
			}),
		).rejects.toMatchObject({
			category: "over_quota",
			code: "OVER_QUOTA",
			message: "limit hit",
		});
	});

	it("preserves explicit auth reason codes from managed failures", async () => {
		const fetchMock = vi.fn().mockResolvedValue({
			ok: false,
			status: 403,
			statusText: "Forbidden",
			json: async () => ({
				code: "POLICY_DENIED",
				reason_code: "policy_denied",
				message: "policy blocked",
			}),
		});
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			postManagedJson({
				path: "http://example.test/v1/llm/complete",
				request: { foo: "bar" },
				idempotencyKey: "idem-403",
			}),
		).rejects.toMatchObject({
			category: "unauthorized",
			reason_code: "policy_denied",
			code: "POLICY_DENIED",
		});
	});

	it("falls back to tauri transport when managed auth token is unavailable", async () => {
		loadRuntimeConfigMock.mockResolvedValueOnce({
      app_version: null,
      api_base_url: null,
      managed_inference_gateway_url: "https://gateway.example",
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: null,
      posthog_host: null,
    });
		invokeMock.mockImplementation(async (command, args) => {
			if (command === "license_get_session_access_token") {
				return null;
			}
			if (command === "managed_inference_llm_complete") {
				return { ok: true, args };
			}
			throw new Error(`Unexpected invoke command: ${command}`);
		});

		const fetchMock = vi.fn();
		vi.stubGlobal("fetch", fetchMock);

		await expect(
			managedInferenceAPI.complete(
				{
					provider: "openai",
					model: "gpt-5",
					operation: "complete",
				},
				"idem-fallback",
			),
		).resolves.toMatchObject({
			ok: true,
			args: {
				request: {
					provider: "openai",
					model: "gpt-5",
					operation: "complete",
				},
				idempotencyKey: "idem-fallback",
			},
		});

		expect(fetchMock).not.toHaveBeenCalled();
	});

	it("falls back to tauri usage-state when managed auth token is unavailable", async () => {
		loadRuntimeConfigMock.mockResolvedValueOnce({
      app_version: null,
      api_base_url: null,
      managed_inference_gateway_url: "https://gateway.example",
      cloudflare_access_client_id: null,
      cloudflare_access_client_secret: null,
      sentry_dsn: null,
      sentry_env: null,
      sentry_release: null,
      posthog_api_key: null,
      posthog_host: null,
    });
		invokeMock.mockImplementation(async (command) => {
			if (command === "license_get_session_access_token") {
				return null;
			}
			if (command === "managed_inference_get_usage_state") {
				return {
					tier: "personal",
					mode: "managed",
					counters: [],
				};
			}
			throw new Error(`Unexpected invoke command: ${command}`);
		});

		const fetchMock = vi.fn();
		vi.stubGlobal("fetch", fetchMock);

		await expect(managedInferenceAPI.getUsageState()).resolves.toEqual({
			tier: "personal",
			mode: "managed",
			counters: [],
		});

		expect(fetchMock).not.toHaveBeenCalled();
	});
});
