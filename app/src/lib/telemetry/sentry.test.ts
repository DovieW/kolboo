import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const sentryMock = vi.hoisted(() => {
	const init = vi.fn();
	const setTag = vi.fn();
	const setContext = vi.fn();
	const withScope = vi.fn(
		(
			cb: (scope: {
				setContext: typeof setContext;
				setTag: typeof setTag;
			}) => void,
		) => {
			cb({ setContext, setTag });
		},
	);
	const captureException = vi.fn();
	const reactErrorHandler = vi.fn((callback?: (...args: unknown[]) => void) => {
		const handler = vi.fn((...args: unknown[]) => {
			callback?.(...args);
		});
		return handler;
	});

	return {
		captureException,
		init,
		reactErrorHandler,
		setContext,
		setTag,
		withScope,
	};
});

const runtimeConfigState = vi.hoisted(() => ({
	value: {
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
	},
}));

vi.mock("@sentry/react", () => sentryMock);

vi.mock("../tauri/runtimeConfig", () => ({
	loadRuntimeConfig: vi.fn(async () => runtimeConfigState.value),
}));

async function loadSentryModule() {
	vi.resetModules();
	return import("./sentry");
}

describe("sentry telemetry", () => {
	beforeEach(() => {
		runtimeConfigState.value = {
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
		};
		sentryMock.captureException.mockClear();
		sentryMock.init.mockClear();
		sentryMock.reactErrorHandler.mockClear();
		sentryMock.setContext.mockClear();
		sentryMock.setTag.mockClear();
		sentryMock.withScope.mockClear();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("sets tier + hashed identity tags", async () => {
		const { initSentry, setSentryLicenseIdentityTags } =
			await loadSentryModule();
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
		const { initSentry, setSentryLicenseIdentityTags } =
			await loadSentryModule();
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

	it("redacts product-content fields inside beforeSend and filters network breadcrumbs", async () => {
		const { initSentry } = await loadSentryModule();
		await initSentry("main");

		const config = sentryMock.init.mock.calls[0]?.[0] as {
			beforeBreadcrumb?: (breadcrumb: Record<string, unknown>) => unknown;
			beforeSend?: (event: Record<string, unknown>) => Record<string, unknown>;
		};

		const safeEvent = config.beforeSend?.({
			user: { id: "user-123" },
			request: { url: "https://example.test" },
			extra: {
				clipboard_contents: "copied text",
				prompt_text: "rewrite this",
				nested: {
					completion_text: "rewritten answer",
					safe_value: "ok",
				},
			},
			contexts: {
				device: { model: "PC" },
				ocr_payload: "screen text",
			},
		});
		expect(safeEvent).toBeDefined();
		const safe = safeEvent as {
			contexts: Record<string, unknown>;
			extra: Record<string, unknown>;
			request?: unknown;
			user?: unknown;
		};

		expect(safe.user).toBeUndefined();
		expect(safe.request).toBeUndefined();
		expect(safe.extra.clipboard_contents).toBe("[REDACTED]");
		expect(safe.extra.prompt_text).toBe("[REDACTED]");
		expect((safe.extra.nested as Record<string, unknown>).completion_text).toBe(
			"[REDACTED]",
		);
		expect((safe.extra.nested as Record<string, unknown>).safe_value).toBe(
			"ok",
		);
		expect(safe.contexts.ocr_payload).toBe("[REDACTED]");
		expect((safe.contexts.device as Record<string, unknown>).model).toBe("PC");

		expect(config.beforeBreadcrumb?.({ category: "fetch" })).toBeNull();
		expect(config.beforeBreadcrumb?.({ category: "xhr" })).toBeNull();
		expect(config.beforeBreadcrumb?.({ category: "ui.click" })).toEqual({
			category: "ui.click",
		});
	});

	it("captures redacted exception context for explicit frontend reports", async () => {
		const { captureSentryException, initSentry } = await loadSentryModule();
		await initSentry("main");

		captureSentryException(new Error("boom"), {
			action: "license_refresh_entitlement",
			extra: {
				clipboard_contents: "copied text",
				nested: { prompt_text: "rewrite this", safe_value: "ok" },
			},
			surface: "main",
		});

		expect(sentryMock.setTag).toHaveBeenCalledWith("surface", "main");
		expect(sentryMock.setTag).toHaveBeenCalledWith(
			"action",
			"license_refresh_entitlement",
		);
		expect(sentryMock.setContext).toHaveBeenCalledWith("license", {
			clipboard_contents: "[REDACTED]",
			nested: { prompt_text: "[REDACTED]", safe_value: "ok" },
		});
		expect(sentryMock.captureException).toHaveBeenCalledWith(expect.any(Error));
	});

	it("builds React 19 root handlers once Sentry is configured", async () => {
		const { getSentryReactRootOptions, initSentry } = await loadSentryModule();
		await initSentry("overlay");

		const options = getSentryReactRootOptions("overlay");

		expect(options).toMatchObject({
			onCaughtError: expect.any(Function),
			onRecoverableError: expect.any(Function),
			onUncaughtError: expect.any(Function),
		});
		expect(sentryMock.reactErrorHandler).toHaveBeenCalledTimes(3);
	});
});
