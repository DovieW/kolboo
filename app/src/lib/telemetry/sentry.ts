import * as Sentry from "@sentry/react";
import { frontendLog } from "../frontendLog";
import {
	isTauriRuntimeAvailable,
	loadRuntimeConfig,
} from "../tauri/runtimeConfig";

export type SentrySurface = "main" | "overlay" | "overlay_hover" | "quick_ask";

const DESKTOP_SENTRY_SMOKE_QUERY_PARAM = "kolboo_sentry_smoke";
const DESKTOP_SENTRY_SMOKE_ENV = "TAURI_SENTRY_SMOKE";
const SENTRY_RUNTIME_CONFIG_RETRY_ATTEMPTS = 20;
const SENTRY_RUNTIME_CONFIG_RETRY_DELAY_MS = 250;

const SENSITIVE_KEY_PATTERN =
	/(?:api[-_]?key|access[-_]?client[-_]?id|access[-_]?token|refresh[-_]?token|id[-_]?token|token|secret|password|passwd|authorization|bearer|cookie|set-cookie|code[-_]?verifier|code[-_]?challenge|auth(?:orization)?[-_]?code|clipboard|transcript|prompt|completion|audio|wav|ocr)/i;

const JWT_LIKE_PATTERN =
	/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9._~+\-/=]+$/;
const BEARER_TOKEN_PATTERN = /^bearer\s+[A-Za-z0-9._~+\-/=]+$/i;

let initialized = false;
let sentryConfigured = false;
let sentryEnvironment: string | null = null;
let sentryRelease: string | null = null;
let sentrySmokeRequestedFromRuntime = false;

type SentryLicenseIdentityInput = {
	tier?: string | null;
	user_id?: string | null;
	org?: {
		org_id?: string | null;
	} | null;
};

export type SentryReactRootOptions = {
	onUncaughtError: ReturnType<typeof Sentry.reactErrorHandler>;
	onCaughtError: ReturnType<typeof Sentry.reactErrorHandler>;
	onRecoverableError: ReturnType<typeof Sentry.reactErrorHandler>;
};

export function isSentryConfigured(): boolean {
	return sentryConfigured;
}

function isRuntimeConfigFallback(config: {
	app_version?: string | null;
	api_base_url?: string | null;
	managed_inference_gateway_url?: string | null;
	cloudflare_access_client_id?: string | null;
	cloudflare_access_client_secret?: string | null;
	sentry_dsn?: string | null;
	sentry_env?: string | null;
	sentry_release?: string | null;
	sentry_smoke?: boolean | null;
	posthog_api_key?: string | null;
	posthog_host?: string | null;
}): boolean {
	return (
		config.app_version == null &&
		config.api_base_url == null &&
		config.managed_inference_gateway_url == null &&
		config.cloudflare_access_client_id == null &&
		config.cloudflare_access_client_secret == null &&
		config.sentry_dsn == null &&
		config.sentry_env == null &&
		config.sentry_release == null &&
		config.sentry_smoke == null &&
		config.posthog_api_key == null &&
		config.posthog_host == null
	);
}

function waitForRuntimeConfigRetry(): Promise<void> {
	return new Promise((resolve) => {
		globalThis.setTimeout(resolve, SENTRY_RUNTIME_CONFIG_RETRY_DELAY_MS);
	});
}

async function loadRuntimeConfigForSentry(
	surface: SentrySurface,
): Promise<Awaited<ReturnType<typeof loadRuntimeConfig>>> {
	let runtimeConfig = await loadRuntimeConfig();

	if (!isTauriRuntimeAvailable() || !isRuntimeConfigFallback(runtimeConfig)) {
		return runtimeConfig;
	}

	for (
		let attempt = 1;
		attempt <= SENTRY_RUNTIME_CONFIG_RETRY_ATTEMPTS;
		attempt += 1
	) {
		frontendLog.warn(
			"sentry",
			`runtime config unavailable surface=${surface} attempt=${attempt}/${SENTRY_RUNTIME_CONFIG_RETRY_ATTEMPTS}; retrying`,
		);
		await waitForRuntimeConfigRetry();
		runtimeConfig = await loadRuntimeConfig();
		if (!isRuntimeConfigFallback(runtimeConfig)) {
			frontendLog.info(
				"sentry",
				`runtime config recovered surface=${surface} attempt=${attempt}`,
			);
			return runtimeConfig;
		}
	}

	frontendLog.warn(
		"sentry",
		`runtime config remained unavailable surface=${surface} after ${SENTRY_RUNTIME_CONFIG_RETRY_ATTEMPTS} retries`,
	);

	return runtimeConfig;
}

function normalizeOptionalString(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function isProductionLikeEnvironment(
	value: string | null | undefined,
): boolean {
	const normalized = value?.trim().toLowerCase();
	return normalized === "prod" || normalized === "production";
}

function resolveFrontendSentryConfig(runtimeConfig: {
	app_version?: string | null;
	sentry_dsn?: string | null;
	sentry_env?: string | null;
	sentry_release?: string | null;
	sentry_smoke?: boolean | null;
}) {
	// Allow browser-only `pnpm dev:vite` verification to opt in via Vite envs when
	// the Tauri runtime bridge is unavailable. Real desktop/Tauri runtime config
	// still wins whenever it is present.
	const dsn =
		normalizeOptionalString(runtimeConfig.sentry_dsn) ||
		normalizeOptionalString(import.meta.env.VITE_SENTRY_DSN);
	const environment =
		normalizeOptionalString(runtimeConfig.sentry_env) ||
		normalizeOptionalString(import.meta.env.VITE_SENTRY_ENV) ||
		import.meta.env.MODE;
	const release =
		normalizeOptionalString(runtimeConfig.sentry_release) ||
		normalizeOptionalString(import.meta.env.VITE_SENTRY_RELEASE) ||
		normalizeOptionalString(runtimeConfig.app_version);

	return {
		dsn,
		environment,
		release,
		smokeRequested: runtimeConfig.sentry_smoke === true,
	};
}

function createLoggedSmokeTransport(enabled: boolean) {
	if (!enabled) {
		return undefined;
	}

	return (...args: Parameters<typeof Sentry.makeFetchTransport>) => {
		const transport = Sentry.makeFetchTransport(...args);

		return {
			...transport,
			async send(request: Parameters<typeof transport.send>[0]) {
				try {
					const response = await transport.send(request);
					const statusCode = response?.statusCode;
					const rateLimits = response?.headers?.["x-sentry-rate-limits"];
					const retryAfter = response?.headers?.["retry-after"];

					const logMessage =
						`transport send status=${statusCode ?? "none"} ` +
						`rate_limits=${rateLimits ?? "none"} retry_after=${retryAfter ?? "none"}`;

					if (
						typeof statusCode === "number" &&
						(statusCode < 200 || statusCode >= 300)
					) {
						frontendLog.warn("sentry", logMessage);
					} else {
						frontendLog.info("sentry", logMessage);
					}

					return response;
				} catch (error) {
					frontendLog.error(
						"sentry",
						`transport send failed ${error instanceof Error ? error.message : toPrimitiveString(error)}`,
					);
					throw error;
				}
			},
		};
	};
}

function toPrimitiveString(value: unknown): string {
	if (typeof value === "string") return value;
	if (
		typeof value === "number" ||
		typeof value === "boolean" ||
		typeof value === "bigint"
	) {
		return String(value);
	}
	return "unknown";
}

export function redactTelemetryValue(value: unknown): unknown {
	if (value == null) return value;

	if (Array.isArray(value)) {
		return value.map((entry) => redactTelemetryValue(entry));
	}

	if (typeof value === "object") {
		const out: Record<string, unknown> = {};
		for (const [key, entry] of Object.entries(
			value as Record<string, unknown>,
		)) {
			if (SENSITIVE_KEY_PATTERN.test(key)) {
				out[key] = "[REDACTED]";
				continue;
			}
			out[key] = redactTelemetryValue(entry);
		}
		return out;
	}

	if (typeof value === "string" && value.length > 512) {
		const trimmed = value.trim();
		if (
			BEARER_TOKEN_PATTERN.test(trimmed) ||
			(JWT_LIKE_PATTERN.test(trimmed) && trimmed.length >= 24)
		) {
			return "[REDACTED]";
		}
		return `${value.slice(0, 512)}…`;
	}

	if (typeof value === "string") {
		const trimmed = value.trim();
		if (
			BEARER_TOKEN_PATTERN.test(trimmed) ||
			(JWT_LIKE_PATTERN.test(trimmed) && trimmed.length >= 24)
		) {
			return "[REDACTED]";
		}
	}

	return value;
}

export async function initSentry(surface: SentrySurface): Promise<boolean> {
	const runtimeConfig = await loadRuntimeConfigForSentry(surface);
	const config = resolveFrontendSentryConfig(runtimeConfig);
	const dsn = config.dsn;
	sentryConfigured = Boolean(dsn);
	sentrySmokeRequestedFromRuntime = config.smokeRequested;
	if (!dsn) {
		frontendLog.warn(
			"sentry",
			`init skipped surface=${surface} reason=no_dsn env=${config.environment ?? "none"} release=${config.release ?? "none"} smoke_requested=${config.smokeRequested}`,
		);
		return false;
	}
	if (initialized) return false;
	sentryEnvironment = config.environment;
	sentryRelease = config.release ?? null;

	Sentry.init({
		dsn,
		enabled: true,
		debug: config.smokeRequested,
		environment: config.environment,
		release: config.release ?? undefined,
		sampleRate: 1,
		transport: createLoggedSmokeTransport(config.smokeRequested),
		tracesSampleRate: 0,
		beforeSend(event) {
			const safe = { ...event };
			delete safe.user;
			delete safe.request;
			if (safe.extra) {
				safe.extra = redactTelemetryValue(safe.extra) as Record<
					string,
					unknown
				>;
			}
			if (safe.contexts) {
				safe.contexts = redactTelemetryValue(
					safe.contexts,
				) as typeof safe.contexts;
			}
			return safe;
		},
		beforeBreadcrumb(breadcrumb) {
			if (
				breadcrumb.category?.toLowerCase().includes("xhr") ||
				breadcrumb.category?.toLowerCase().includes("fetch")
			) {
				return null;
			}
			return breadcrumb;
		},
	});

	Sentry.setTag("surface", surface);
	initialized = true;
	frontendLog.info(
		"sentry",
		`initialized surface=${surface} env=${config.environment ?? "none"} release=${config.release ?? "none"} smoke_requested=${config.smokeRequested}`,
	);
	return true;
}

function isSentrySmokeRequested(locationSearch: string): boolean {
	const params = new URLSearchParams(locationSearch);
	if (!params.has(DESKTOP_SENTRY_SMOKE_QUERY_PARAM)) {
		return false;
	}

	const rawValue =
		params.get(DESKTOP_SENTRY_SMOKE_QUERY_PARAM)?.trim().toLowerCase() ?? "";
	return rawValue === "" || ["1", "true", "yes", "on"].includes(rawValue);
}

function resolveSentrySmokeTrigger(
	locationSearch: string,
): "query-param" | "runtime-env" | null {
	if (isSentrySmokeRequested(locationSearch)) {
		return "query-param";
	}

	if (sentrySmokeRequestedFromRuntime) {
		return "runtime-env";
	}

	return null;
}

export async function maybeCaptureSentrySmokeTest(
	surface: SentrySurface,
	locationSearch = globalThis.location?.search ?? "",
): Promise<boolean> {
	const smokeTrigger = resolveSentrySmokeTrigger(locationSearch);
	if (!smokeTrigger) {
		return false;
	}

	if (!initialized || !isSentryConfigured()) {
		frontendLog.warn(
			"sentry",
			`smoke skipped surface=${surface} trigger=${smokeTrigger} reason=not_initialized`,
		);
		return false;
	}

	if (isProductionLikeEnvironment(sentryEnvironment)) {
		frontendLog.warn(
			"sentry",
			`smoke skipped surface=${surface} trigger=${smokeTrigger} reason=production_env env=${sentryEnvironment ?? "none"}`,
		);
		return false;
	}

	const release =
		sentryRelease ?? `kolboo@${sentryEnvironment ?? import.meta.env.MODE}`;
	frontendLog.info(
		"sentry",
		`smoke capture surface=${surface} trigger=${smokeTrigger} release=${release}`,
	);

	// Keep browser smoke verification explicit and non-production so the desktop
	// frontend can prove the release/tag wiring without normalizing fake crashes in
	// real customer sessions.
	Sentry.withScope((scope) => {
		scope.setTag("surface", surface);
		scope.setTag("action", "smoke_test");
		scope.setTag("smoke_test", "true");
		scope.setTag("smoke_trigger", smokeTrigger);
		scope.setContext("smoke_test", {
			environment: sentryEnvironment ?? import.meta.env.MODE,
			release,
			runtime_env: DESKTOP_SENTRY_SMOKE_ENV,
			smoke_trigger: smokeTrigger,
			surface,
			...(smokeTrigger === "query-param"
				? { query_param: DESKTOP_SENTRY_SMOKE_QUERY_PARAM }
				: {}),
		});
		Sentry.captureException(
			new Error(`Kolboo desktop frontend Sentry smoke test (${surface})`),
		);
	});

	// Browser-only smoke verification often runs in short-lived headless sessions,
	// so explicitly flush the queue before returning to make the proof path
	// reliable instead of timing-sensitive.
	await Sentry.flush(2000);
	frontendLog.info(
		"sentry",
		`smoke flushed surface=${surface} trigger=${smokeTrigger} release=${release}`,
	);

	return true;
}

export function getSentryReactRootOptions(
	surface: SentrySurface,
): SentryReactRootOptions | undefined {
	if (!initialized || !isSentryConfigured()) return undefined;

	// We intentionally keep our custom fallback UIs, but React 19's root hooks let
	// Sentry observe caught/uncaught/recoverable render failures across each
	// surface without replacing those UX paths.
	Sentry.setTag("surface", surface);

	return {
		onUncaughtError: Sentry.reactErrorHandler(),
		onCaughtError: Sentry.reactErrorHandler(),
		onRecoverableError: Sentry.reactErrorHandler(),
	};
}

function hashFallback(value: string): string {
	let hash = 2166136261;
	for (let i = 0; i < value.length; i += 1) {
		hash ^= value.charCodeAt(i);
		hash = Math.imul(hash, 16777619);
	}
	return `fnv1a_${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

async function hashStableIdentifier(value: string): Promise<string> {
	const trimmed = value.trim();
	if (!trimmed) return "none";

	try {
		const data = new TextEncoder().encode(trimmed);
		const digest = await globalThis.crypto?.subtle?.digest("SHA-256", data);
		if (!digest) return hashFallback(trimmed);
		return Array.from(new Uint8Array(digest))
			.map((x) => x.toString(16).padStart(2, "0"))
			.join("");
	} catch {
		return hashFallback(trimmed);
	}
}

export async function setSentryLicenseIdentityTags(
	identity: SentryLicenseIdentityInput | null | undefined,
): Promise<void> {
	if (!initialized || !isSentryConfigured()) return;

	const tier = identity?.tier?.trim() || "unknown";
	const userId = identity?.user_id?.trim() || "";
	const orgId = identity?.org?.org_id?.trim() || "";

	const [userHash, orgHash] = await Promise.all([
		userId ? hashStableIdentifier(`user:${userId}`) : Promise.resolve("none"),
		orgId ? hashStableIdentifier(`org:${orgId}`) : Promise.resolve("none"),
	]);

	Sentry.setTag("tier", tier);
	Sentry.setTag("user_hash", userHash);
	Sentry.setTag("org_hash", orgHash);
}

export function captureSentryException(
	error: unknown,
	params: {
		surface: SentrySurface;
		action: string;
		extra?: Record<string, unknown>;
	},
) {
	if (!isSentryConfigured()) return;

	Sentry.withScope((scope) => {
		scope.setTag("surface", params.surface);
		scope.setTag("action", params.action);
		if (params.extra) {
			scope.setContext(
				"license",
				redactTelemetryValue(params.extra) as Record<string, unknown>,
			);
		}

		if (error instanceof Error) {
			Sentry.captureException(error);
			return;
		}

		Sentry.captureException(new Error(toPrimitiveString(error)));
	});
}
