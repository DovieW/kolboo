import type {
	AuthReasonCode,
	LicenseAuthContext,
	LicenseState,
	LicenseStatus,
} from "../../lib/tauri";

export type AccountModeLabel = "BYOK" | "Personal" | "Managed Business";

export function formatAccountStatusLabel(status: LicenseStatus): string {
	if (status === "active") return "Active";
	if (status === "grace") return "Grace";
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
		return "Personal";
	}
	return "BYOK";
}

export function getAccountModeDescription(params: {
	modeLabel: AccountModeLabel;
	signedIn: boolean;
	reauthRequired: boolean;
}): string {
	if (!params.signedIn) {
		return "Sign in to save a Community/BYOK session now. Upgrade later to Personal/Pro or Managed Business for settings sync and managed inference.";
	}
	if (params.reauthRequired) {
		return "Your managed access needs attention. Try Refresh access to check your current entitlement.";
	}
	if (params.modeLabel === "Managed Business") {
		return "Managed access is active with organization policy and usage controls applied.";
	}
	if (params.modeLabel === "Personal") {
		return "Managed personal access is active for this account.";
	}
	return "You're signed in and currently running in Community/BYOK mode using your own providers and keys. Upgrade to Personal/Pro or Managed Business later for settings sync and managed inference.";
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
	if (modeLabel === "BYOK") return false;
	return Boolean(licenseState);
}

export function calculateUsagePercent(used: number, limit: number): number {
	if (!Number.isFinite(used) || !Number.isFinite(limit) || limit <= 0) return 0;
	return Math.min(100, Math.round((used / limit) * 100));
}
