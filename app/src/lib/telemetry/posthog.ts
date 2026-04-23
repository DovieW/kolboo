import { Store } from "@tauri-apps/plugin-store";
import { loadRuntimeConfig } from "../tauri/runtimeConfig";

const SENSITIVE_KEY_PATTERN =
	/(?:api[-_]?key|access[-_]?token|refresh[-_]?token|id[-_]?token|token|secret|password|passwd|authorization|bearer|cookie|set-cookie|code[-_]?verifier|code[-_]?challenge|auth(?:orization)?[-_]?code|text|transcript|prompt|completion|audio|wav|ocr)/i;

const JWT_LIKE_PATTERN =
	/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9._~+\-/=]+$/;
const BEARER_TOKEN_PATTERN = /^bearer\s+[A-Za-z0-9._~+\-/=]+$/i;

const ANALYTICS_ENABLED_KEY = "posthog_analytics_enabled";
const DISTINCT_ID_STORAGE_KEY = "kolboo_posthog_distinct_id_v1";

function trimOrEmpty(value: string | undefined): string {
	return (value ?? "").trim();
}

function normalizePosthogHost(host: string): string {
	return host.endsWith("/") ? host.slice(0, -1) : host;
}

function getDistinctId(): string {
	const uuid =
		typeof globalThis.crypto?.randomUUID === "function"
			? globalThis.crypto.randomUUID.bind(globalThis.crypto)
			: () => `${Date.now()}-${Math.random().toString(16).slice(2)}`;

	try {
		const existing = globalThis.localStorage?.getItem(DISTINCT_ID_STORAGE_KEY);
		if (existing && existing.trim().length > 0) {
			return existing.trim();
		}

		const next = `kolboo-${uuid()}`;
		globalThis.localStorage?.setItem(DISTINCT_ID_STORAGE_KEY, next);
		return next;
	} catch {
		return `kolboo-ephemeral-${uuid()}`;
	}
}

function sanitizeTelemetryValue(value: unknown): unknown {
	if (value == null) return value;

	if (Array.isArray(value)) {
		return value.slice(0, 20).map((entry) => sanitizeTelemetryValue(entry));
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
			out[key] = sanitizeTelemetryValue(entry);
		}
		return out;
	}

	if (typeof value === "string") {
		const trimmed = value.trim();
		if (
			BEARER_TOKEN_PATTERN.test(trimmed) ||
			(JWT_LIKE_PATTERN.test(trimmed) && trimmed.length >= 24)
		) {
			return "[REDACTED]";
		}
		if (trimmed.length > 256) {
			return `${trimmed.slice(0, 256)}…`;
		}
		return trimmed;
	}

	if (
		typeof value === "number" ||
		typeof value === "boolean" ||
		typeof value === "bigint"
	) {
		return value;
	}

	return String(value);
}

export async function isPosthogConfigured(): Promise<boolean> {
	const config = await loadRuntimeConfig();
	const apiKey = trimOrEmpty(config.posthog_api_key ?? undefined);
	const host = trimOrEmpty(config.posthog_host ?? undefined);
	return apiKey.length > 0 && host.length > 0;
}

async function isAnalyticsEnabled(): Promise<boolean> {
	try {
		const store = await Store.load("settings.json");
		return (await store.get<boolean>(ANALYTICS_ENABLED_KEY)) ?? true;
	} catch {
		return true;
	}
}

export async function trackProductEvent(
	event: string,
	properties: Record<string, unknown> = {},
): Promise<void> {
	const eventName = event.trim();
	if (!eventName) return;
	if (!(await isPosthogConfigured())) return;
	if (!(await isAnalyticsEnabled())) return;
	if (typeof fetch !== "function") return;

	const config = await loadRuntimeConfig();

	const apiKey = trimOrEmpty(config.posthog_api_key ?? undefined);
	const host = normalizePosthogHost(
		trimOrEmpty(config.posthog_host ?? undefined),
	);
	if (!apiKey || !host) return;

	const sanitizedProperties = sanitizeTelemetryValue(properties);
	const safeProperties: Record<string, unknown> =
		typeof sanitizedProperties === "object" &&
		sanitizedProperties !== null &&
		!Array.isArray(sanitizedProperties)
			? (sanitizedProperties as Record<string, unknown>)
			: {};

	const payload = {
		api_key: apiKey,
		event: eventName,
		properties: {
			distinct_id: getDistinctId(),
			$lib: "kolboo-desktop",
			$lib_version: trimOrEmpty(config.app_version ?? undefined) || "dev",
			...safeProperties,
		},
	};

	try {
		await fetch(`${host}/capture/`, {
			method: "POST",
			headers: {
				"content-type": "application/json",
			},
			body: JSON.stringify(payload),
		});
	} catch {
		// Never block product flow on analytics transport.
	}
}
