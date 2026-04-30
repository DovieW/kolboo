import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const sentryMock = vi.hoisted(() => {
	const init = vi.fn();
	const setTag = vi.fn();
	const withScope = vi.fn((cb: (scope: { setTag: typeof setTag }) => void) => {
		cb({ setTag });
	});
	const captureException = vi.fn();
	return { init, setTag, withScope, captureException };
});

vi.mock("@sentry/react", () => sentryMock);

vi.mock("../tauri/runtimeConfig", () => ({
  loadRuntimeConfig: vi.fn(async () => ({
    app_version: "0.2.4-test",
    api_base_url: null,
    managed_inference_gateway_url: null,
    cloudflare_access_client_id: null,
    cloudflare_access_client_secret: null,
    sentry_dsn: "https://dsn.example/123",
    sentry_env: "test",
    sentry_release: "kolboo-frontend@test",
    posthog_api_key: null,
    posthog_host: null,
  })),
}));

import {
	initSentry,
	redactTelemetryValue,
	setSentryLicenseIdentityTags,
} from "./sentry";

describe("sentry identity tags", () => {
	beforeEach(() => {
		sentryMock.init.mockClear();
		sentryMock.setTag.mockClear();
		sentryMock.withScope.mockClear();
		sentryMock.captureException.mockClear();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("sets tier + hashed identity tags", async () => {
		await initSentry("main");

		await setSentryLicenseIdentityTags({
			tier: "personal",
			user_id: "user-123",
			org: { org_id: "org-456" },
		});

		const tags = sentryMock.setTag.mock.calls.map(([key, value]) => [
			String(key),
			String(value),
		]);

		expect(tags).toContainEqual(["tier", "personal"]);

		const userHash = tags.find(([key]) => key === "user_hash")?.[1] ?? "";
		const orgHash = tags.find(([key]) => key === "org_hash")?.[1] ?? "";

		expect(userHash).not.toBe("user-123");
		expect(orgHash).not.toBe("org-456");
		expect(userHash).toMatch(/^[a-f0-9_]+$/i);
		expect(orgHash).toMatch(/^[a-f0-9_]+$/i);
	});

	it("uses none when user/org are absent", async () => {
		await initSentry("main");

		await setSentryLicenseIdentityTags({
			tier: "community",
			user_id: null,
			org: null,
		});

		expect(sentryMock.setTag).toHaveBeenCalledWith("tier", "community");
		expect(sentryMock.setTag).toHaveBeenCalledWith("user_hash", "none");
		expect(sentryMock.setTag).toHaveBeenCalledWith("org_hash", "none");
	});

	it("redacts token-like strings in telemetry payload values", () => {
		const sample = {
			context: {
				authorization: "Bearer abc.def.ghi",
				nested: {
					session_blob:
						"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyLTEyMyJ9.signature",
				},
			},
		};

		const redacted = redactTelemetryValue(sample) as {
			context: { authorization: string; nested: { session_blob: string } };
		};

		expect(redacted.context.authorization).toBe("[REDACTED]");
		expect(redacted.context.nested.session_blob).toBe("[REDACTED]");
	});
});
