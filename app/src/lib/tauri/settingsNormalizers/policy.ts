import { evaluateTokenExchangeDecision } from "../../auth/tokenExchangeGate";
import type {
	LicenseState,
	OrgInferenceMode,
	PolicyState,
	TokenExchangeTriggerSet,
} from "../types";
import { isRecord } from "./shared";

export function normalizePolicySource(value: unknown): PolicyState["source"] {
	if (
		value === "none" ||
		value === "file" ||
		value === "cloud" ||
		value === "cached" ||
		value === "degraded_expired"
	) {
		return value;
	}
	return "none";
}

export function normalizePolicyTimestamp(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

export function normalizePolicyEnforcedFields(
	value: unknown,
): PolicyState["enforced_fields"] {
	return Array.isArray(value)
		? value
				.map((field): PolicyState["enforced_fields"][number] | null => {
					if (!isRecord(field)) return null;
					const path = typeof field.path === "string" ? field.path.trim() : "";
					if (!path) return null;
					const reason = typeof field.reason === "string" ? field.reason : null;
					return { path, reason };
				})
				.filter(
					(field): field is PolicyState["enforced_fields"][number] =>
						field !== null,
				)
		: [];
}

export function normalizePolicyState(value: unknown): PolicyState {
	const v = isRecord(value) ? value : {};
	const source = normalizePolicySource(v.source);
	const eligible = typeof v.eligible === "boolean" ? v.eligible : false;
	const active_policy_id =
		typeof v.active_policy_id === "string" ? v.active_policy_id : null;
	const active_version =
		typeof v.active_version === "number" && Number.isFinite(v.active_version)
			? Math.max(0, Math.trunc(v.active_version))
			: null;
	const last_sync_at = normalizePolicyTimestamp(v.last_sync_at);
	const last_success_at = normalizePolicyTimestamp(v.last_success_at);
	const last_updated = normalizePolicyTimestamp(v.last_updated);
	const expires_at = normalizePolicyTimestamp(v.expires_at);
	const failure_reason =
		typeof v.failure_reason === "string" ? v.failure_reason : null;
	const enforced_fields = normalizePolicyEnforcedFields(v.enforced_fields);
	const enforced_count =
		typeof v.enforced_count === "number" && Number.isFinite(v.enforced_count)
			? Math.max(0, Math.trunc(v.enforced_count))
			: null;
	const version =
		typeof v.version === "string"
			? v.version
			: typeof v.version === "number" && Number.isFinite(v.version)
				? String(Math.trunc(v.version))
				: null;

	const now = Date.now();
	const expiresAtMs = expires_at == null ? null : Date.parse(expires_at);
	const expired =
		expiresAtMs != null && Number.isFinite(expiresAtMs) && expiresAtMs < now;

	const baseValid = typeof v.is_valid === "boolean" ? v.is_valid : true;
	const is_valid = source === "none" ? true : baseValid && !expired;

	return {
		source,
		eligible,
		is_valid,
		active_policy_id,
		active_version,
		last_sync_at,
		last_success_at,
		last_updated,
		expires_at,
		failure_reason,
		enforced_count: enforced_count ?? enforced_fields.length,
		version,
		enforced_fields,
	};
}

export function normalizeLicenseTier(value: unknown): LicenseState["tier"] {
	if (value === "enterprise" || value === "personal" || value === "community") {
		return value;
	}
	return "community";
}

export function normalizeLicenseStatus(value: unknown): LicenseState["status"] {
	if (
		value === "signed_out" ||
		value === "active" ||
		value === "grace" ||
		value === "expired"
	) {
		return value;
	}
	return "signed_out";
}

export function normalizeLicenseTimestamp(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

export function normalizeLicenseState(value: unknown): LicenseState {
	const nowIso = new Date().toISOString();
	const v = isRecord(value) ? value : {};

	const tier = normalizeLicenseTier(v.tier);
	const status = normalizeLicenseStatus(v.status);
	const user_id = typeof v.user_id === "string" ? v.user_id : null;
	const email = typeof v.email === "string" ? v.email : null;

	const org = isRecord(v.org)
		? (() => {
				const org_id =
					typeof v.org.org_id === "string" ? v.org.org_id.trim() : "";
				const org_name =
					typeof v.org.org_name === "string" ? v.org.org_name.trim() : "";
				const inference_mode: OrgInferenceMode | null =
					v.org.inference_mode === "org_byok" ||
					v.org.inference_mode === "managed"
						? v.org.inference_mode
						: null;
				if (!org_id || !org_name) return null;
				return { org_id, org_name, inference_mode };
			})()
		: null;

	const usage = isRecord(v.usage)
		? {
				stt_seconds_used:
					typeof v.usage.stt_seconds_used === "number" &&
					Number.isFinite(v.usage.stt_seconds_used)
						? Math.max(0, Math.trunc(v.usage.stt_seconds_used))
						: 0,
				llm_tokens_used:
					typeof v.usage.llm_tokens_used === "number" &&
					Number.isFinite(v.usage.llm_tokens_used)
						? Math.max(0, Math.trunc(v.usage.llm_tokens_used))
						: 0,
				requests_today:
					typeof v.usage.requests_today === "number" &&
					Number.isFinite(v.usage.requests_today)
						? Math.max(0, Math.trunc(v.usage.requests_today))
						: 0,
			}
		: {
				stt_seconds_used: 0,
				llm_tokens_used: 0,
				requests_today: 0,
			};

	const limits = isRecord(v.limits)
		? {
				stt_seconds_monthly:
					typeof v.limits.stt_seconds_monthly === "number" &&
					Number.isFinite(v.limits.stt_seconds_monthly)
						? Math.max(0, Math.trunc(v.limits.stt_seconds_monthly))
						: 0,
				llm_tokens_monthly:
					typeof v.limits.llm_tokens_monthly === "number" &&
					Number.isFinite(v.limits.llm_tokens_monthly)
						? Math.max(0, Math.trunc(v.limits.llm_tokens_monthly))
						: 0,
				requests_per_day:
					typeof v.limits.requests_per_day === "number" &&
					Number.isFinite(v.limits.requests_per_day)
						? Math.max(0, Math.trunc(v.limits.requests_per_day))
						: 0,
			}
		: {
				stt_seconds_monthly: 0,
				llm_tokens_monthly: 0,
				requests_per_day: 0,
			};

	return {
		tier,
		status,
		user_id,
		email,
		org,
		expires_at: normalizeLicenseTimestamp(v.expires_at),
		cached_at: normalizeLicenseTimestamp(v.cached_at) ?? nowIso,
		last_validated_at: normalizeLicenseTimestamp(v.last_validated_at),
		usage,
		limits,
		portal_available: v.portal_available === true,
	};
}

export function normalizeTokenExchangeReviewedAt(
	value: unknown,
): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

export function normalizeTokenExchangeTriggerSet(
	value: unknown,
): TokenExchangeTriggerSet {
	const v = isRecord(value) ? value : {};
	const multi_idp_required = Boolean(v.multi_idp_required);
	const kill_switch_required = Boolean(v.kill_switch_required);
	const embedded_claims_required = Boolean(v.embedded_claims_required);
	const desktop_idp_agnostic_required = Boolean(
		v.desktop_idp_agnostic_required,
	);
	const reviewed_at = normalizeTokenExchangeReviewedAt(v.reviewed_at);

	return {
		multi_idp_required,
		kill_switch_required,
		embedded_claims_required,
		desktop_idp_agnostic_required,
		reviewed_at,
		decision: evaluateTokenExchangeDecision({
			multi_idp_required,
			kill_switch_required,
			embedded_claims_required,
			desktop_idp_agnostic_required,
		}),
	};
}
