import { core } from "@tauri-apps/api";
import { invoke } from "@tauri-apps/api/core";

export interface RuntimeConfig {
	app_version: string | null;
	api_base_url: string | null;
	managed_inference_gateway_url: string | null;
	cloudflare_access_client_id: string | null;
	cloudflare_access_client_secret: string | null;
	sentry_dsn: string | null;
	sentry_env: string | null;
	sentry_release: string | null;
	sentry_smoke: boolean | null;
	posthog_api_key: string | null;
	posthog_host: string | null;
}

const DEFAULT_RUNTIME_CONFIG: RuntimeConfig = {
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

let runtimeConfigPromise: Promise<RuntimeConfig> | null = null;

const RUNTIME_CONFIG_TAURI_RETRY_ATTEMPTS = 50;
const RUNTIME_CONFIG_NON_TAURI_RETRY_ATTEMPTS = 1;
const RUNTIME_CONFIG_RETRY_DELAY_MS = 100;

function normalizeOptionalString(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function normalizeOptionalBoolean(value: unknown): boolean | null {
	if (typeof value === "boolean") return value;
	if (typeof value === "string") {
		const normalized = value.trim().toLowerCase();
		if (["1", "true", "yes", "on"].includes(normalized)) {
			return true;
		}
		if (["0", "false", "no", "off"].includes(normalized)) {
			return false;
		}
	}
	return null;
}

function normalizeConfig(
	raw: Partial<RuntimeConfig> | null | undefined,
): RuntimeConfig {
	return {
		app_version: normalizeOptionalString(raw?.app_version),
		api_base_url: normalizeOptionalString(raw?.api_base_url),
		managed_inference_gateway_url: normalizeOptionalString(
			raw?.managed_inference_gateway_url,
		),
		cloudflare_access_client_id: normalizeOptionalString(
			raw?.cloudflare_access_client_id,
		),
		cloudflare_access_client_secret: normalizeOptionalString(
			raw?.cloudflare_access_client_secret,
		),
		sentry_dsn: normalizeOptionalString(raw?.sentry_dsn),
		sentry_env: normalizeOptionalString(raw?.sentry_env),
		sentry_release: normalizeOptionalString(raw?.sentry_release),
		sentry_smoke: normalizeOptionalBoolean(raw?.sentry_smoke),
		posthog_api_key: normalizeOptionalString(raw?.posthog_api_key),
		posthog_host: normalizeOptionalString(raw?.posthog_host),
	};
}

function waitForRuntimeConfigRetry(): Promise<void> {
	return new Promise((resolve) => {
		globalThis.setTimeout(resolve, RUNTIME_CONFIG_RETRY_DELAY_MS);
	});
}

export function isTauriRuntimeAvailable(): boolean {
	const isTauri = (
		core as typeof core & {
			isTauri?: boolean | (() => boolean);
		}
	).isTauri;

	return typeof isTauri === "function" ? isTauri() : Boolean(isTauri);
}

async function fetchRuntimeConfig(): Promise<RuntimeConfig> {
	const retryAttempts = isTauriRuntimeAvailable()
		? RUNTIME_CONFIG_TAURI_RETRY_ATTEMPTS
		: RUNTIME_CONFIG_NON_TAURI_RETRY_ATTEMPTS;

	for (let attempt = 1; attempt <= retryAttempts; attempt += 1) {
		try {
			const config = await invoke<RuntimeConfig>("get_runtime_config");
			return normalizeConfig(config);
		} catch {
			if (attempt === retryAttempts) {
				return DEFAULT_RUNTIME_CONFIG;
			}

			await waitForRuntimeConfigRetry();
		}
	}

	return DEFAULT_RUNTIME_CONFIG;
}

export function loadRuntimeConfig(): Promise<RuntimeConfig> {
	if (!runtimeConfigPromise) {
		runtimeConfigPromise = fetchRuntimeConfig().then((config) => {
			if (config === DEFAULT_RUNTIME_CONFIG) {
				runtimeConfigPromise = null;
			}

			return config;
		});
	}

	return runtimeConfigPromise;
}
