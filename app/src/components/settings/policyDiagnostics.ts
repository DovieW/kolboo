import type { PolicyState } from "../../lib/tauri";

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
