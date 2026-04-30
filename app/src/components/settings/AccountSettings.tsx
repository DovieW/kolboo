import type { LicenseState } from "../../lib/tauri";
import type { AuthReasonCode } from "../../lib/tauri/types";

export function formatLicenseTierLabel(tier: LicenseState["tier"]): string {
	if (tier === "enterprise") return "Enterprise";
	if (tier === "personal") return "Personal";
	return "Community";
}

export function formatLicenseStatusLabel(
	status: LicenseState["status"],
): string {
	if (status === "active") return "Active";
	if (status === "grace") return "Grace";
	if (status === "expired") return "Expired";
	return "Signed out";
}

export function isLicenseStatusDegraded(
	status: LicenseState["status"],
): boolean {
	return status === "grace" || status === "expired";
}

export function getOrgDisplayName(
	state: LicenseState | null | undefined,
): string {
	return state?.org?.org_name ?? "—";
}

export function shouldShowOrgId(
	state: LicenseState | null | undefined,
): boolean {
	return Boolean(state?.org?.org_id);
}

export function isReauthRequiredReason(
	reasonCode: AuthReasonCode | null | undefined,
): boolean {
	return reasonCode === "reauth_required" || reasonCode === "token_invalid";
}
