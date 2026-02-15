import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	captureSentryException,
	redactTelemetryValue,
} from "../telemetry/sentry";
import { listenTyped } from "./events";
import type {
	LicenseState,
	LicenseStatus,
	LicenseTransitionPayload,
	SettingsChangedPayload,
} from "./types";

export type LicenseLoginRequest = {
	provider_hint?: string | null;
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
			return await invoke("license_get_state");
		} catch (error) {
			reportLicenseSentryError("license_get_state", error);
			throw error;
		}
	},

	startLogin: async (request?: LicenseLoginRequest): Promise<LicenseState> => {
		try {
			return await invoke("license_start_login", {
				request: {
					provider_hint: request?.provider_hint ?? null,
				},
			});
		} catch (error) {
			reportLicenseSentryError("license_start_login", error, {
				provider_hint: request?.provider_hint ?? null,
			});
			throw error;
		}
	},

	logout: async (): Promise<LicenseState> => {
		try {
			return await invoke("license_logout");
		} catch (error) {
			reportLicenseSentryError("license_logout", error);
			throw error;
		}
	},

	refreshEntitlement: async (
		simulateFailure?: boolean,
	): Promise<LicenseState> => {
		try {
			return await invoke("license_refresh_entitlement", {
				simulateFailure: simulateFailure ?? null,
			});
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
