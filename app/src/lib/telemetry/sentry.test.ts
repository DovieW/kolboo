import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeConfig } from "../tauri/runtimeConfig";

const loadRuntimeConfigMock = vi.hoisted(() => vi.fn());
const frontendLogMock = vi.hoisted(() => ({
	debug: vi.fn(),
	error: vi.fn(),
	info: vi.fn(),
	warn: vi.fn(),
}));
const isTauriState = vi.hoisted(() => ({ value: true }));

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
	const flush = vi.fn(async () => true);
	const makeFetchTransport = vi.fn(() => ({
		flush: vi.fn(async () => true),
		send: vi.fn(async () => ({
			headers: {
				"retry-after": null,
				"x-sentry-rate-limits": null,
			},
			statusCode: 200,
		})),
	}));
	const reactErrorHandler = vi.fn((callback?: (...args: unknown[]) => void) => {
		const handler = vi.fn((...args: unknown[]) => {
			callback?.(...args);
		});
		return handler;
	});

	return {
		captureException,
		flush,
		init,
		makeFetchTransport,
		reactErrorHandler,
		setContext,
		setTag,
		withScope,
	};
});

const runtimeConfigState = vi.hoisted((): { value: RuntimeConfig } => ({
	value: {
		app_version: "0.2.4-test",
		api_base_url: null,
		managed_inference_gateway_url: null,
		cloudflare_access_client_id: null,
		cloudflare_access_client_secret: null,
		sentry_dsn: "https://dsn.example/123",
		sentry_env: "test",
		sentry_release: "kolboo-frontend@test",
		sentry_smoke: null,
		posthog_api_key: null,
		posthog_host: null,
	},
}));

vi.mock("@sentry/react", () => sentryMock);

vi.mock("@tauri-apps/api", () => ({
	core: {
		get isTauri() {
			return isTauriState.value;
		},
	},
}));

vi.mock("../tauri/runtimeConfig", () => ({
	isTauriRuntimeAvailable: () => isTauriState.value,
	loadRuntimeConfig: loadRuntimeConfigMock,
}));

vi.mock("../frontendLog", () => ({
	frontendLog: frontendLogMock,
}));

async function loadSentryModule() {
	vi.resetModules();
	return import("./sentry");
}

describe("sentry telemetry", () => {
	beforeEach(() => {
		vi.useRealTimers();
		runtimeConfigState.value = {
			app_version: "0.2.4-test",
			api_base_url: null,
			managed_inference_gateway_url: null,
			cloudflare_access_client_id: null,
			cloudflare_access_client_secret: null,
			sentry_dsn: "https://dsn.example/123",
			sentry_env: "test",
			sentry_release: "kolboo-frontend@test",
			sentry_smoke: null,
			posthog_api_key: null,
			posthog_host: null,
		};
		sentryMock.captureException.mockClear();
		sentryMock.init.mockClear();
		sentryMock.reactErrorHandler.mockClear();
		sentryMock.setContext.mockClear();
		sentryMock.setTag.mockClear();
		sentryMock.withScope.mockClear();
		sentryMock.flush.mockClear();
		sentryMock.makeFetchTransport.mockClear();
		sentryMock.flush.mockResolvedValue(true);
		frontendLogMock.debug.mockClear();
		frontendLogMock.error.mockClear();
		frontendLogMock.info.mockClear();
		frontendLogMock.warn.mockClear();
		isTauriState.value = true;
		loadRuntimeConfigMock.mockReset();
		loadRuntimeConfigMock.mockImplementation(
			async () => runtimeConfigState.value,
		);
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
		vi.unstubAllEnvs();
	});

	it("retries Tauri runtime-config loading after an all-default fallback", async () => {
		vi.useFakeTimers();

		const fallbackConfig: RuntimeConfig = {
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
		};

		loadRuntimeConfigMock
			.mockResolvedValueOnce(fallbackConfig)
			.mockResolvedValueOnce(fallbackConfig)
			.mockResolvedValueOnce(runtimeConfigState.value);

		const { initSentry } = await loadSentryModule();
		const initPromise = initSentry("main");

		await vi.runAllTimersAsync();
		await initPromise;

		expect(loadRuntimeConfigMock).toHaveBeenCalledTimes(3);
		expect(sentryMock.init).toHaveBeenCalledWith(
			expect.objectContaining({
				dsn: "https://dsn.example/123",
				environment: "test",
				release: "kolboo-frontend@test",
			}),
		);
		expect(frontendLogMock.warn).toHaveBeenCalledWith(
			"sentry",
			expect.stringContaining(
				"runtime config unavailable surface=main attempt=1/20",
			),
		);
		expect(frontendLogMock.info).toHaveBeenCalledWith(
			"sentry",
			expect.stringContaining(
				"runtime config recovered surface=main attempt=2",
			),
		);
	});

	it("falls back to Vite env values for browser-only verification", async () => {
		isTauriState.value = false;
		runtimeConfigState.value = {
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
		};
		vi.stubEnv("VITE_SENTRY_DSN", "https://dsn.example/browser");
		vi.stubEnv("VITE_SENTRY_ENV", "preview");
		vi.stubEnv("VITE_SENTRY_RELEASE", "kolboo@preview-local");

		const { initSentry } = await loadSentryModule();
		await initSentry("main");

		expect(sentryMock.init).toHaveBeenCalledWith(
			expect.objectContaining({
				dsn: "https://dsn.example/browser",
				environment: "preview",
				release: "kolboo@preview-local",
			}),
		);
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

	it("captures a non-production desktop smoke event when requested", async () => {
		const { initSentry, maybeCaptureSentrySmokeTest } =
			await loadSentryModule();
		await initSentry("main");

		expect(
			await maybeCaptureSentrySmokeTest("main", "?kolboo_sentry_smoke=1"),
		).toBe(true);

		expect(sentryMock.setTag).toHaveBeenCalledWith("surface", "main");
		expect(sentryMock.setTag).toHaveBeenCalledWith("action", "smoke_test");
		expect(sentryMock.setTag).toHaveBeenCalledWith("smoke_test", "true");
		expect(sentryMock.setTag).toHaveBeenCalledWith(
			"smoke_trigger",
			"query-param",
		);
		expect(sentryMock.setContext).toHaveBeenCalledWith("smoke_test", {
			environment: "test",
			query_param: "kolboo_sentry_smoke",
			release: "kolboo-frontend@test",
			runtime_env: "TAURI_SENTRY_SMOKE",
			smoke_trigger: "query-param",
			surface: "main",
		});
		expect(sentryMock.captureException).toHaveBeenCalledWith(expect.any(Error));
		expect(sentryMock.flush).toHaveBeenCalledWith(2000);
	});

	it("captures a non-production desktop smoke event when enabled by runtime env", async () => {
		runtimeConfigState.value = {
			...runtimeConfigState.value,
			sentry_smoke: true,
		};

		const { initSentry, maybeCaptureSentrySmokeTest } =
			await loadSentryModule();
		await initSentry("main");

		expect(await maybeCaptureSentrySmokeTest("main")).toBe(true);
		expect(sentryMock.init).toHaveBeenCalledWith(
			expect.objectContaining({
				debug: true,
				transport: expect.any(Function),
			}),
		);

		expect(sentryMock.setTag).toHaveBeenCalledWith(
			"smoke_trigger",
			"runtime-env",
		);
		expect(sentryMock.setContext).toHaveBeenCalledWith("smoke_test", {
			environment: "test",
			release: "kolboo-frontend@test",
			runtime_env: "TAURI_SENTRY_SMOKE",
			smoke_trigger: "runtime-env",
			surface: "main",
		});
		expect(sentryMock.captureException).toHaveBeenCalledWith(expect.any(Error));
		expect(sentryMock.flush).toHaveBeenCalledWith(2000);
	});

	it("blocks the desktop smoke event in production-like environments", async () => {
		runtimeConfigState.value = {
			...runtimeConfigState.value,
			sentry_env: "production",
			sentry_release: "kolboo@production",
			sentry_smoke: true,
		};

		const { initSentry, maybeCaptureSentrySmokeTest } =
			await loadSentryModule();
		await initSentry("main");

		expect(
			await maybeCaptureSentrySmokeTest("main", "?kolboo_sentry_smoke=1"),
		).toBe(false);
		expect(sentryMock.captureException).not.toHaveBeenCalled();
		expect(sentryMock.flush).not.toHaveBeenCalled();
	});
});
