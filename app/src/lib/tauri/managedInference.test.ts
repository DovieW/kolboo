import { describe, expect, it, vi } from "vitest";

vi.mock("./runtimeConfig", () => ({
	loadRuntimeConfig: vi.fn(async () => ({
		app_version: null,
		api_base_url: null,
		managed_inference_gateway_url: null,
		sentry_dsn: null,
		sentry_env: null,
		sentry_release: null,
		posthog_api_key: null,
		posthog_host: null,
	})),
}));

import { createIdempotencyKey, postManagedJson } from "./managedInference";

describe("managedInference", () => {
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
});
