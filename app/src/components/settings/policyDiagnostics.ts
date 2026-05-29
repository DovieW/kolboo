import type {
	LicenseAuthContext,
	LicenseState,
	PolicyDiagnosticExport,
	PolicyState,
	RequestLog,
} from "../../lib/tauri";
import type { RuntimeConfig } from "../../lib/tauri/runtimeConfig";

export interface SupportSafeTargetSummary {
	target_scope: string | null;
	target_hash: string | null;
}

export interface SupportRequestLogSummary {
	request_id: string;
	kind: RequestLog["kind"] | null;
	status: RequestLog["status"];
	started_at: string;
	ended_at: string | null;
	stt_provider: string;
	stt_model: string | null;
	llm_provider: string | null;
	llm_model: string | null;
	managed_inference: boolean | null;
	total_duration_ms: number | null;
	error_present: boolean;
}

export interface SupportDiagnosticsBundle {
	generated_at: string;
	support_handoff_version: number;
	redaction_applied: boolean;
	app: {
		version: string | null;
		sentry_env: string | null;
		sentry_release: string | null;
	};
	operator_handoff: {
		request_ids: string[];
		user_target: SupportSafeTargetSummary;
		org_target: SupportSafeTargetSummary;
		recommended_workflow: string[];
	};
	license: {
		tier: LicenseState["tier"];
		status: LicenseState["status"];
		scope: "personal" | "enterprise";
		expires_at: string | null;
		cached_at: string;
		last_validated_at: string | null;
		usage: LicenseState["usage"];
		limits: LicenseState["limits"];
	} | null;
	auth_context: {
		authenticated: boolean;
		secure_session_present: boolean;
		mode: LicenseAuthContext["mode"];
		policy_status: LicenseAuthContext["policy_status"];
		reason_code: LicenseAuthContext["reason_code"];
		issuer: string | null;
		user_target: SupportSafeTargetSummary;
		org_target: SupportSafeTargetSummary;
	} | null;
	policy: PolicyDiagnosticExport;
	recent_request_logs: SupportRequestLogSummary[];
}

async function defaultHashSupportScope(
	targetScope: string,
): Promise<string | null> {
	if (typeof globalThis.crypto?.subtle?.digest !== "function") {
		return null;
	}

	const encoded = new TextEncoder().encode(targetScope);
	const digest = await globalThis.crypto.subtle.digest("SHA-256", encoded);
	return Array.from(new Uint8Array(digest), (value) =>
		value.toString(16).padStart(2, "0"),
	).join("");
}

async function buildSupportSafeTargetSummary(
	kind: "user" | "org",
	rawId: string | null | undefined,
	hashSupportScope: (targetScope: string) => Promise<string | null>,
): Promise<SupportSafeTargetSummary> {
	const trimmed = rawId?.trim();
	if (!trimmed) {
		return {
			target_scope: null,
			target_hash: null,
		};
	}

	const targetHash = await hashSupportScope(`${kind}:${trimmed}`);
	return {
		target_scope: targetHash ? `${kind}:${targetHash.slice(0, 12)}` : null,
		target_hash: targetHash,
	};
}

function summarizeSupportRequestLog(log: RequestLog): SupportRequestLogSummary {
	return {
		request_id: log.id,
		kind: log.kind ?? null,
		status: log.status,
		started_at: log.started_at,
		ended_at: log.ended_at,
		stt_provider: log.stt_provider,
		stt_model: log.stt_model,
		llm_provider: log.llm_provider,
		llm_model: log.llm_model,
		managed_inference: log.managed_inference ?? null,
		total_duration_ms: log.total_duration_ms,
		error_present: Boolean(log.error_message),
	};
}

export async function buildSupportDiagnosticsBundle(
	input: {
		policyExport: PolicyDiagnosticExport;
		runtimeConfig: RuntimeConfig | null;
		licenseState: LicenseState | null;
		authContext: LicenseAuthContext | null;
		requestLogs: RequestLog[];
	},
	hashSupportScope: (
		targetScope: string,
	) => Promise<string | null> = defaultHashSupportScope,
): Promise<SupportDiagnosticsBundle> {
	const recentRequestLogs = input.requestLogs
		.slice(0, 10)
		.map((log) => summarizeSupportRequestLog(log));
	const requestIds = Array.from(
		new Set(recentRequestLogs.map((log) => log.request_id).filter(Boolean)),
	);

	const userId =
		input.authContext?.subject_id ?? input.licenseState?.user_id ?? null;
	const orgId =
		input.authContext?.org_id ?? input.licenseState?.org?.org_id ?? null;
	const [userTarget, orgTarget] = await Promise.all([
		buildSupportSafeTargetSummary("user", userId, hashSupportScope),
		buildSupportSafeTargetSummary("org", orgId, hashSupportScope),
	]);

	return {
		generated_at: input.policyExport.generated_at,
		support_handoff_version: 1,
		redaction_applied: true,
		app: {
			version: input.runtimeConfig?.app_version ?? null,
			sentry_env: input.runtimeConfig?.sentry_env ?? null,
			sentry_release: input.runtimeConfig?.sentry_release ?? null,
		},
		operator_handoff: {
			request_ids: requestIds,
			user_target: userTarget,
			org_target: orgTarget,
			recommended_workflow: [
				"Use Operator Console account lookup or `kolops entitlement lookup` with the customer email or user ID from the support ticket, then compare the returned targetHash to `user_target.target_hash` when present.",
				"If this is an enterprise org, load org overview or billing/webhook failures with the org ID from the authenticated support workflow and compare the returned targetHash to `org_target.target_hash` when present.",
				"Use the exported `request_ids` to correlate desktop reports with structured logs, Sentry events, and audited operator actions.",
			],
		},
		license: input.licenseState
			? {
					tier: input.licenseState.tier,
					status: input.licenseState.status,
					scope: input.licenseState.org ? "enterprise" : "personal",
					expires_at: input.licenseState.expires_at,
					cached_at: input.licenseState.cached_at,
					last_validated_at: input.licenseState.last_validated_at,
					usage: input.licenseState.usage,
					limits: input.licenseState.limits,
				}
			: null,
		auth_context: input.authContext
			? {
					authenticated: input.authContext.authenticated,
					secure_session_present: input.authContext.secure_session_present,
					mode: input.authContext.mode,
					policy_status: input.authContext.policy_status,
					reason_code: input.authContext.reason_code,
					issuer: input.authContext.issuer,
					user_target: userTarget,
					org_target: orgTarget,
				}
			: null,
		policy: input.policyExport,
		recent_request_logs: recentRequestLogs,
	};
}

export function formatPolicySourceLabel(source: PolicyState["source"]): string {
	if (source === "cloud") return "Cloud";
	if (source === "cached") return "Cached";
	if (source === "degraded_expired") return "Degraded";
	if (source === "file") return "Local file";
	return "Unmanaged";
}

export function formatPolicyTimestampLabel(value: string | null): string {
	if (!value) return "—";
	const parsed = Date.parse(value);
	if (Number.isNaN(parsed)) return "—";
	return new Date(parsed).toLocaleString();
}

export function policyStatusSummary(policy: PolicyState): string {
	if (policy.source === "none") return "No active policy";
	if (policy.source === "degraded_expired") return "Policy degraded (expired)";
	if (policy.source === "cached") return "Using cached policy";
	if (!policy.is_valid) return "Policy invalid";
	return "Policy active";
}

export function policyStatusColor(policy: PolicyState): string {
	if (policy.source === "degraded_expired" || !policy.is_valid) return "red";
	if (policy.source === "cached") return "yellow";
	if (policy.source === "none") return "gray";
	return "green";
}

export function diagnosticsToJson(payload: unknown): string {
	return JSON.stringify(payload, null, 2);
}
