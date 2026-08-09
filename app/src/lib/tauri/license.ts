import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	captureSentryException,
	redactTelemetryValue,
	setSentryLicenseIdentityTags,
} from "../telemetry/sentry";
import { listenTyped } from "./events";
import type {
	AuthReasonCode,
	LicenseAuthContext,
	LicenseState,
	LicenseStatus,
	LicenseTransitionPayload,
	SessionExchangeResponse,
	SettingsChangedPayload,
} from "./types";

export type LicenseLoginRequest = {
	provider_hint?: string | null;
	auth_provider?: string | null;
	email?: string | null;
	password?: string | null;
};

export type LicenseSignupRequest = {
	email: string;
	password: string;
};

export type LicenseSignupResponse = {
	state: LicenseState;
	confirmation_required: boolean;
	email: string | null;
};

export function buildLicenseSentryContext(
	context: Record<string, unknown>,
): Record<string, unknown> {
	return redactTelemetryValue(context) as Record<string, unknown>;
}

export function reportLicenseSentryError(
	action: string,
	error: unknown,
	context: Record<string, unknown> = {},
) {
	captureSentryException(error, {
		surface: "main",
		action,
		extra: buildLicenseSentryContext(context),
	});
}

export function getLicenseErrorMessage(error: unknown): string {
	if (error instanceof Error && typeof error.message === "string") {
		const msg = error.message.trim();
		if (msg.length > 0) return msg;
	}
	if (typeof error === "string" && error.trim().length > 0) {
		return error.trim();
	}
	return "Something went wrong while updating account status.";
}

const VALID_AUTH_REASON_CODES = new Set<AuthReasonCode>([
	"reauth_required",
	"token_invalid",
	"membership_missing",
	"insufficient_tier",
	"policy_denied",
	"auth_not_configured",
	"unknown",
]);

export function normalizeAuthReasonCode(value: unknown): AuthReasonCode | null {
	if (typeof value !== "string") return null;
	return VALID_AUTH_REASON_CODES.has(value as AuthReasonCode)
		? (value as AuthReasonCode)
		: null;
}

export function authReasonCodeToMessage(
	code: AuthReasonCode | null,
): string | null {
	if (!code) return null;
	if (code === "reauth_required") return "Please sign in again.";
	if (code === "token_invalid") {
		return "Your token is no longer valid. Please sign in again.";
	}
	if (code === "membership_missing") {
		return "No active organization membership was found.";
	}
	if (code === "insufficient_tier") {
		return "Your current tier does not allow this managed action.";
	}
	if (code === "policy_denied") {
		return "This action was denied by your organization policy.";
	}
	if (code === "auth_not_configured") {
		return "Managed authentication is not configured in this environment.";
	}
	return "Authentication context could not be resolved.";
}

export function getLicenseTransitionFromSettingsPayload(
	payload: SettingsChangedPayload | null | undefined,
): LicenseTransitionPayload | null {
	if (!payload || typeof payload !== "object") return null;
	const candidate = (payload as Record<string, unknown>).license_transition;
	if (!candidate || typeof candidate !== "object") return null;

	const from = (candidate as Record<string, unknown>).from;
	const to = (candidate as Record<string, unknown>).to;
	const occurred_at = (candidate as Record<string, unknown>).occurred_at;
	const reason = (candidate as Record<string, unknown>).reason;
	const validStatuses = new Set<LicenseStatus>([
		"signed_out",
		"active",
		"grace",
		"expired",
	]);

	if (
		typeof from !== "string" ||
		typeof to !== "string" ||
		typeof occurred_at !== "string" ||
		typeof reason !== "string"
	) {
		return null;
	}

	if (!validStatuses.has(from as LicenseStatus)) return null;
	if (!validStatuses.has(to as LicenseStatus)) return null;

	return {
		from: from as LicenseStatus,
		to: to as LicenseStatus,
		occurred_at,
		reason,
	};
}

export const tauriLicenseAPI = {
	getState: async (): Promise<LicenseState> => {
		try {
			const state = await invoke<LicenseState>("license_get_state");
			await setSentryLicenseIdentityTags(state);
			return state;
		} catch (error) {
			reportLicenseSentryError("license_get_state", error);
			throw error;
		}
	},

	getAuthContext: async (): Promise<LicenseAuthContext> => {
		try {
			const context = await invoke<LicenseAuthContext>(
				"license_get_auth_context",
			);
			return {
				...context,
				reason_code: normalizeAuthReasonCode(context.reason_code),
			};
		} catch (error) {
			reportLicenseSentryError("license_get_auth_context", error);
			throw error;
		}
	},

	startLogin: async (request?: LicenseLoginRequest): Promise<LicenseState> => {
		try {
			const state = await invoke<LicenseState>("license_start_login", {
				request: {
					provider_hint: request?.provider_hint ?? null,
					auth_provider: request?.auth_provider ?? null,
					email: request?.email ?? null,
					password: request?.password ?? null,
				},
			});
			await setSentryLicenseIdentityTags(state);
			return state;
		} catch (error) {
			reportLicenseSentryError("license_start_login", error, {
				provider_hint: request?.provider_hint ?? null,
				auth_provider: request?.auth_provider ?? null,
			});
			throw error;
		}
	},

	signUp: async (
		request: LicenseSignupRequest,
	): Promise<LicenseSignupResponse> => {
		try {
			const response = await invoke<LicenseSignupResponse>("license_sign_up", {
				request: {
					email: request.email,
					password: request.password,
				},
			});
			await setSentryLicenseIdentityTags(response.state);
			return response;
		} catch (error) {
			reportLicenseSentryError("license_sign_up", error, {
				email_present: request.email.trim().length > 0,
				password_present: request.password.length > 0,
			});
			throw error;
		}
	},

	requestPasswordReset: async (email: string): Promise<void> => {
		try {
			await invoke("license_request_password_reset", { email });
		} catch (error) {
			reportLicenseSentryError("license_request_password_reset", error, {
				email_present: email.trim().length > 0,
			});
			throw error;
		}
	},

	exchangeSession: async (
		upstreamAccessToken: string,
	): Promise<SessionExchangeResponse> => {
		try {
			return await invoke<SessionExchangeResponse>("license_exchange_session", {
				request: {
					upstream_access_token: upstreamAccessToken,
				},
			});
		} catch (error) {
			reportLicenseSentryError("license_exchange_session", error, {
				upstream_token_present: upstreamAccessToken.trim().length > 0,
			});
			throw error;
		}
	},

	logout: async (): Promise<LicenseState> => {
		try {
			const state = await invoke<LicenseState>("license_logout");
			await setSentryLicenseIdentityTags(state);
			return state;
		} catch (error) {
			reportLicenseSentryError("license_logout", error);
			throw error;
		}
	},

	refreshEntitlement: async (
		simulateFailure?: boolean,
	): Promise<LicenseState> => {
		try {
			const state = await invoke<LicenseState>("license_refresh_entitlement", {
				simulateFailure: simulateFailure ?? null,
			});
			await setSentryLicenseIdentityTags(state);
			return state;
		} catch (error) {
			reportLicenseSentryError("license_refresh_entitlement", error, {
				simulateFailure: simulateFailure ?? null,
			});
			throw error;
		}
	},

	getManagementUrl: async (): Promise<string> => {
		try {
			return await invoke("license_get_management_url");
		} catch (error) {
			reportLicenseSentryError("license_get_management_url", error);
			throw error;
		}
	},

	onTransition: (
		handler: (payload: LicenseTransitionPayload) => void,
	): Promise<UnlistenFn> =>
		listenTyped("settings-changed", (payload) => {
			const transition = getLicenseTransitionFromSettingsPayload(payload);
			if (transition) {
				handler(transition);
			}
		}),
};
