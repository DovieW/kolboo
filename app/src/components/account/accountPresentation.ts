import type {
	AuthReasonCode,
	LicenseAuthContext,
	LicenseState,
	LicenseStatus,
} from "../../lib/tauri";

export type AccountModeLabel = "Community" | "Pro" | "Managed Business";

export function formatAccountStatusLabel(status: LicenseStatus): string {
	if (status === "active") return "Active";
	if (status === "grace") return "Offline access";
	if (status === "expired") return "Expired";
	return "Signed out";
}

export function formatInternalTierLabel(tier: LicenseState["tier"]): string {
	if (tier === "enterprise") return "Enterprise";
	if (tier === "personal") return "Personal";
	return "Community";
}

export function isReauthRequiredReason(
	reasonCode: AuthReasonCode | null | undefined,
): boolean {
	return reasonCode === "reauth_required" || reasonCode === "token_invalid";
}

export function isReauthRequiredForSession(
	signedIn: boolean,
	reasonCode: AuthReasonCode | null | undefined,
): boolean {
	return signedIn && isReauthRequiredReason(reasonCode);
}

export function isManagedAccountContext(
	licenseState: LicenseState | null | undefined,
	authContext: LicenseAuthContext | null | undefined,
): boolean {
	if (!licenseState || !authContext?.authenticated) return false;
	if (authContext.policy_status !== "allow") return false;
    if (licenseState.status !== "active" && licenseState.status !== "grace") return false;
	return licenseState.tier === "personal" || licenseState.tier === "enterprise";
}

export function getAccountModeLabel(
	licenseState: LicenseState | null | undefined,
	authContext: LicenseAuthContext | null | undefined,
): AccountModeLabel {
	if (
		licenseState?.tier === "enterprise" &&
		isManagedAccountContext(licenseState, authContext)
	) {
		return "Managed Business";
	}
	if (
		licenseState?.tier === "personal" &&
		isManagedAccountContext(licenseState, authContext)
	) {
		return "Pro";
	}
	return "Community";
}

export function getAccountModeDescription(params: {
	modeLabel: AccountModeLabel;
	signedIn: boolean;
	reauthRequired: boolean;
}): string {
	if (!params.signedIn) {
		return "Community/BYOK works without an account. Sign in for eligible managed access and settings sync.";
	}
	if (params.reauthRequired) {
		return "Your managed access needs attention. Try Refresh access to check your current entitlement.";
	}
	if (params.modeLabel === "Managed Business") {
		return "Your organization provides managed access, subject to its policies.";
	}
	if (params.modeLabel === "Pro") {
		return "Managed models and settings sync. Your own API keys remain available.";
	}
	return "You're signed in with Community/BYOK access. Approved beta accounts include managed models and settings sync.";
}

export function getAccountStatusColor(params: {
	status: LicenseStatus | null | undefined;
	reauthRequired: boolean;
}): string {
	if (params.reauthRequired) return "yellow";
	if (params.status === "active") return "green";
	if (params.status === "grace") return "yellow";
	if (params.status === "expired") return "red";
	return "gray";
}

export function shouldShowManagedUsage(
	modeLabel: AccountModeLabel,
	licenseState: LicenseState | null | undefined,
): boolean {
	if (modeLabel === "Community") return false;
	return Boolean(licenseState);
}

export function calculateUsagePercent(used: number, limit: number): number {
	if (!Number.isFinite(used) || !Number.isFinite(limit) || limit <= 0) return 0;
	return Math.min(100, Math.round((used / limit) * 100));
}
