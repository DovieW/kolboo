import { invoke } from "@tauri-apps/api/core";
import { authReasonCodeToMessage, normalizeAuthReasonCode } from "./license";
import { loadRuntimeConfig } from "./runtimeConfig";
import type {
	AuthReasonCode,
	LicenseAuthContext,
	ManagedError,
	ManagedErrorCategory,
	ManagedUsageState,
} from "./types";

export interface ManagedSttRequest {
	provider: string;
	model: string;
	audio_ref: string;
	metadata?: Record<string, unknown>;
}

export interface ManagedLlmRequest {
	provider: string;
	model: string;
	operation: "rewrite" | "complete" | "quick_ask";
	input_ref?: string | null;
	metadata?: Record<string, unknown>;
}

export interface ManagedModel {
	id: string;
	display_name: string;
	provider: string;
	capabilities: Array<"chat_completions" | "responses">;
	default_for_provider: boolean;
}

export interface ManagedModelCatalogResponse {
	models: ManagedModel[];
	request_id: string;
}

export function hasManagedInferenceAccess(
	context: LicenseAuthContext | null | undefined,
): boolean {
	return (
		context?.authenticated === true &&
		context.policy_status === "allow" &&
		context.entitlements.includes("managed_inference")
	);
}

function hostnameForUrl(value: string | null): string | null {
	if (!value) return null;
	try {
		return new URL(value).hostname.toLowerCase();
	} catch {
		return null;
	}
}

function shouldAttachCloudflareAccessHeaders(
	url: string,
	config: Awaited<ReturnType<typeof loadRuntimeConfig>>,
): boolean {
	const targetHost = hostnameForUrl(url);
	if (!targetHost) return false;

	const allowedHosts = [
		hostnameForUrl(config.api_base_url),
		hostnameForUrl(config.managed_inference_gateway_url),
	].filter((host): host is string => Boolean(host));

	return allowedHosts.includes(targetHost);
}

function attachCloudflareAccessHeaders(
	headers: Record<string, string>,
	url: string,
	config: Awaited<ReturnType<typeof loadRuntimeConfig>>,
): void {
	if (!shouldAttachCloudflareAccessHeaders(url, config)) return;

	const clientId = normalizeOptionalToken(config.cloudflare_access_client_id);
	const clientSecret = normalizeOptionalToken(
		config.cloudflare_access_client_secret,
	);

	if (!clientId || !clientSecret) return;

	headers["CF-Access-Client-Id"] = clientId;
	headers["CF-Access-Client-Secret"] = clientSecret;
}

function normalizeOptionalToken(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : null;
}

function codeForReason(reasonCode: AuthReasonCode | null): string {
	if (!reasonCode) return "MANAGED_AUTH_REQUIRED";
	return reasonCode.toUpperCase();
}

async function resolveManagedSessionAccessToken(): Promise<string | null> {
	try {
		return normalizeOptionalToken(
			await invoke<string | null>("license_get_session_access_token"),
		);
	} catch {
		return null;
	}
}

function createManagedAuthError(
	reasonCode: AuthReasonCode | null,
): ManagedError {
	return {
		category:
			reasonCode === "auth_not_configured"
				? "temporarily_unavailable"
				: "unauthorized",
		code: codeForReason(reasonCode),
		message:
			authReasonCodeToMessage(reasonCode) ??
			"Managed authentication is unavailable right now.",
		reason_code: reasonCode,
		request_id: null,
		retry_after_seconds: null,
	};
}

type ManagedInvokeFallback = {
	command: string;
	args?: Record<string, unknown>;
};

async function invokeManagedFallback<TRes>(
	fallbackInvoke: ManagedInvokeFallback,
): Promise<TRes> {
	return invoke<TRes>(fallbackInvoke.command, fallbackInvoke.args ?? {});
}

async function resolveManagedAuthUnavailableReason(): Promise<AuthReasonCode | null> {
	try {
		const context = await invoke<{ reason_code?: unknown }>(
			"license_get_auth_context",
		);
		return normalizeAuthReasonCode(context?.reason_code);
	} catch {
		return "reauth_required";
	}
}

function categoryForStatus(status: number): ManagedErrorCategory {
	if (status === 401 || status === 403) return "unauthorized";
	if (status === 409 || status === 412) return "ineligible";
	if (status === 402 || status === 429) return "over_quota";
	return "temporarily_unavailable";
}

function normalizeManagedError(payload: unknown, status: number): ManagedError {
	const p = payload as Record<string, unknown> | null;
	const category =
		typeof p?.category === "string"
			? (p.category as ManagedErrorCategory)
			: categoryForStatus(status);

	return {
		category,
		code: typeof p?.code === "string" ? p.code : `HTTP_${status}`,
		message:
			typeof p?.message === "string"
				? p.message
				: "Managed inference request failed",
		reason_code: normalizeAuthReasonCode(p?.reason_code),
		request_id: typeof p?.request_id === "string" ? p.request_id : null,
		retry_after_seconds:
			typeof p?.retry_after_seconds === "number" ? p.retry_after_seconds : null,
	};
}

async function requestManagedJson<TReq, TRes>(params: {
	method: "GET" | "POST";
	path: string;
	request?: TReq;
	idempotencyKey?: string;
	fallbackInvoke?: ManagedInvokeFallback;
}): Promise<TRes> {
	const config = await loadRuntimeConfig();
	const trimmedBaseUrl = config.managed_inference_gateway_url?.trim() ?? "";
	const baseUrl =
		trimmedBaseUrl.length > 0 ? trimmedBaseUrl.replace(/\/$/, "") : null;
	const isAbsolutePath = /^https?:\/\//i.test(params.path);
	if (!baseUrl && !isAbsolutePath) {
		if (params.fallbackInvoke) {
			return invokeManagedFallback(params.fallbackInvoke);
		}

		throw {
			category: "temporarily_unavailable",
			code: "MANAGED_GATEWAY_NOT_CONFIGURED",
			message: "Managed inference gateway URL is not configured",
		} satisfies ManagedError;
	}

	const url = isAbsolutePath ? params.path : `${baseUrl}${params.path}`;
	const headers: Record<string, string> = {
		"content-type": "application/json",
	};

	if (params.idempotencyKey) {
		headers["x-idempotency-key"] = params.idempotencyKey;
	}

	if (!isAbsolutePath) {
		const accessToken = await resolveManagedSessionAccessToken();
		if (!accessToken) {
			if (params.fallbackInvoke) {
				return invokeManagedFallback(params.fallbackInvoke);
			}

			throw createManagedAuthError(await resolveManagedAuthUnavailableReason());
		}

		headers.authorization = `Bearer ${accessToken}`;
	}

	attachCloudflareAccessHeaders(headers, url, config);

	const response = await fetch(url, {
		method: params.method,
		headers,
		body:
			params.method === "POST"
				? JSON.stringify(params.request ?? null)
				: undefined,
	});

	if (!response.ok) {
		const payload = await response
			.json()
			.catch(() => ({ message: response.statusText }));
		throw normalizeManagedError(payload, response.status);
	}

	return (await response.json()) as TRes;
}

export async function postManagedJson<TReq, TRes>(params: {
	path: string;
	request: TReq;
	idempotencyKey: string;
	fallbackInvoke?: ManagedInvokeFallback;
}): Promise<TRes> {
	return requestManagedJson<TReq, TRes>({
		method: "POST",
		path: params.path,
		request: params.request,
		idempotencyKey: params.idempotencyKey,
		fallbackInvoke: params.fallbackInvoke,
	});
}

async function getManagedJson<TRes>(params: {
	path: string;
	fallbackInvoke?: ManagedInvokeFallback;
}): Promise<TRes> {
	return requestManagedJson<never, TRes>({
		method: "GET",
		path: params.path,
		fallbackInvoke: params.fallbackInvoke,
	});
}

export function createIdempotencyKey(prefix = "kolboo"): string {
	const random =
		typeof crypto !== "undefined" && "randomUUID" in crypto
			? crypto.randomUUID()
			: `${Date.now()}-${Math.random().toString(16).slice(2)}`;
	return `${prefix}-${random}`;
}

export const managedInferenceAPI = {
	getModels: async (): Promise<ManagedModelCatalogResponse> => {
		return getManagedJson<ManagedModelCatalogResponse>({
			path: "/v1/managed/models",
		});
	},

	transcribe: async (request: ManagedSttRequest, idempotencyKey: string) => {
		return postManagedJson<ManagedSttRequest, unknown>({
			path: "/v1/stt/transcribe",
			request,
			idempotencyKey,
			fallbackInvoke: {
				command: "managed_inference_stt_transcribe",
				args: {
					request,
					idempotencyKey,
				},
			},
		});
	},

	complete: async (request: ManagedLlmRequest, idempotencyKey: string) => {
		return postManagedJson<ManagedLlmRequest, unknown>({
			path: "/v1/llm/complete",
			request,
			idempotencyKey,
			fallbackInvoke: {
				command: "managed_inference_llm_complete",
				args: {
					request,
					idempotencyKey,
				},
			},
		});
	},

	getUsageState: async (): Promise<ManagedUsageState> => {
		return getManagedJson<ManagedUsageState>({
			path: "/v1/usage/state",
			fallbackInvoke: {
				command: "managed_inference_get_usage_state",
			},
		});
	},
};
