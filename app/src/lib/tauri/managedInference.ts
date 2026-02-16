import { invoke } from "@tauri-apps/api/core";
import type {
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

const GATEWAY_BASE_URL =
	(import.meta.env.VITE_MANAGED_INFERENCE_GATEWAY_URL as string | undefined) ??
	"";

function resolveManagedGatewayBaseUrl(): string | null {
	const trimmed = GATEWAY_BASE_URL.trim();
	return trimmed.length > 0 ? trimmed.replace(/\/$/, "") : null;
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
		request_id: typeof p?.request_id === "string" ? p.request_id : null,
		retry_after_seconds:
			typeof p?.retry_after_seconds === "number" ? p.retry_after_seconds : null,
	};
}

export async function postManagedJson<TReq, TRes>(params: {
	path: string;
	request: TReq;
	idempotencyKey: string;
}): Promise<TRes> {
	const baseUrl = resolveManagedGatewayBaseUrl();
	const isAbsolutePath = /^https?:\/\//i.test(params.path);
	if (!baseUrl && !isAbsolutePath) {
		throw {
			category: "temporarily_unavailable",
			code: "MANAGED_GATEWAY_NOT_CONFIGURED",
			message: "Managed inference gateway URL is not configured",
		} satisfies ManagedError;
	}

	const url = isAbsolutePath ? params.path : `${baseUrl}${params.path}`;

	const response = await fetch(url, {
		method: "POST",
		headers: {
			"content-type": "application/json",
			"x-idempotency-key": params.idempotencyKey,
		},
		body: JSON.stringify(params.request),
	});

	if (!response.ok) {
		const payload = await response
			.json()
			.catch(() => ({ message: response.statusText }));
		throw normalizeManagedError(payload, response.status);
	}

	return (await response.json()) as TRes;
}

export function createIdempotencyKey(prefix = "kolboo"): string {
	const random =
		typeof crypto !== "undefined" && "randomUUID" in crypto
			? crypto.randomUUID()
			: `${Date.now()}-${Math.random().toString(16).slice(2)}`;
	return `${prefix}-${random}`;
}

export const managedInferenceAPI = {
	transcribe: async (request: ManagedSttRequest, idempotencyKey: string) => {
		if (!resolveManagedGatewayBaseUrl()) {
			return invoke("managed_inference_stt_transcribe", {
				request,
				idempotencyKey,
			});
		}

		return postManagedJson<ManagedSttRequest, unknown>({
			path: "/v1/stt/transcribe",
			request,
			idempotencyKey,
		});
	},

	complete: async (request: ManagedLlmRequest, idempotencyKey: string) => {
		if (!resolveManagedGatewayBaseUrl()) {
			return invoke("managed_inference_llm_complete", {
				request,
				idempotencyKey,
			});
		}

		return postManagedJson<ManagedLlmRequest, unknown>({
			path: "/v1/llm/complete",
			request,
			idempotencyKey,
		});
	},

	getUsageState: async (): Promise<ManagedUsageState> => {
		const baseUrl = resolveManagedGatewayBaseUrl();
		if (!baseUrl) {
			return invoke("managed_inference_get_usage_state");
		}

		const response = await fetch(`${baseUrl}/v1/usage/state`);
		if (!response.ok) {
			const payload = await response
				.json()
				.catch(() => ({ message: response.statusText }));
			throw normalizeManagedError(payload, response.status);
		}

		return response.json();
	},
};
