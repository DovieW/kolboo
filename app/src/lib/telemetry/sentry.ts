import * as Sentry from "@sentry/react";
import { loadRuntimeConfig } from "../tauri/runtimeConfig";

export type SentrySurface = "main" | "overlay" | "overlay_hover" | "quick_ask";

const SENSITIVE_KEY_PATTERN =
	/(?:api[-_]?key|access[-_]?token|refresh[-_]?token|token|secret|password|passwd|authorization|bearer|cookie|set-cookie)/i;

let initialized = false;
let sentryConfigured = false;

type SentryLicenseIdentityInput = {
	tier?: string | null;
	user_id?: string | null;
	org?: {
		org_id?: string | null;
	} | null;
};

export function isSentryConfigured(): boolean {
	return sentryConfigured;
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
		return `${value.slice(0, 512)}…`;
	}

	return value;
}

export async function initSentry(surface: SentrySurface): Promise<boolean> {
	const runtimeConfig = await loadRuntimeConfig();
	const dsn = runtimeConfig.sentry_dsn?.trim();
	sentryConfigured = Boolean(dsn);
	if (!dsn || initialized) return false;

	Sentry.init({
		dsn,
		enabled: true,
		environment: runtimeConfig.sentry_env?.trim() || import.meta.env.MODE,
		release:
			runtimeConfig.sentry_release?.trim() ||
			runtimeConfig.app_version?.trim() ||
			undefined,
		sampleRate: 1,
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
	return true;
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
