import { Badge, Button, Group, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { ExternalLink, RefreshCcw, ShieldUser } from "lucide-react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	useLicenseState,
	useLogoutLicense,
	useRefreshLicenseEntitlement,
	useStartLicenseLogin,
} from "../../lib/queries";
import { type LicenseState, licenseAPI } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

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

export function AccountSettings() {
	const license = useLicenseState();
	const login = useStartLicenseLogin();
	const logout = useLogoutLicense();
	const refresh = useRefreshLicenseEntitlement();

	const data = license.data;
	const signedIn = data?.status !== "signed_out";
	const statusLabel = data ? formatLicenseStatusLabel(data.status) : "Loading…";
	const tierLabel = data ? formatLicenseTierLabel(data.tier) : "—";

	const statusColor = !data
		? "gray"
		: data.status === "active"
			? "green"
			: data.status === "grace"
				? "yellow"
				: data.status === "expired"
					? "red"
					: "gray";

	const openManageSubscription = async () => {
		try {
			const url = await licenseAPI.getManagementUrl();
			window.open(url, "_blank", "noopener,noreferrer");
		} catch (error) {
			notifications.show({
				title: "Could not open account page",
				message: formatErrorMessage(error),
				color: "red",
			});
		}
	};

	return (
		<Stack gap="md">
			<SettingsRow
				label="Account state"
				description="Optional sign-in for managed features. Baseline mode remains available when signed out."
				right={
					<Group gap="xs" align="center" justify="flex-end">
						<Badge color={statusColor}>{statusLabel}</Badge>
						<Badge variant="light" color="gray">
							{tierLabel}
						</Badge>
					</Group>
				}
			/>

			<SettingsRow
				label="Identity"
				description="Signed-in account details and optional organization context."
				right={
					<Stack gap={2} align="flex-end">
						<Text size="xs" c="dimmed">
							User: {data?.email ?? "—"}
						</Text>
						<Text size="xs" c="dimmed">
							Org: {getOrgDisplayName(data)}
						</Text>
						{shouldShowOrgId(data) ? (
							<Text size="xs" c="dimmed">
								Org ID: {data?.org?.org_id}
							</Text>
						) : null}
					</Stack>
				}
			/>

			<SettingsRow
				label="Session actions"
				description="Sign in, sign out, refresh entitlement, or open account management."
				right={
					<Group gap="xs" justify="flex-end">
						<Button
							variant="default"
							size="xs"
							leftSection={<ShieldUser size={14} />}
							loading={login.isPending}
							onClick={() => login.mutate({ provider_hint: "personal" })}
							disabled={signedIn}
						>
							Sign in
						</Button>
						<Button
							variant="default"
							size="xs"
							leftSection={<RefreshCcw size={14} />}
							loading={refresh.isPending}
							onClick={() => refresh.mutate(false)}
							disabled={!signedIn}
						>
							Refresh
						</Button>
						<Button
							variant="default"
							size="xs"
							leftSection={<ExternalLink size={14} />}
							onClick={openManageSubscription}
						>
							Manage
						</Button>
						<Button
							variant="light"
							color="red"
							size="xs"
							loading={logout.isPending}
							onClick={() => logout.mutate()}
							disabled={!signedIn}
						>
							Sign out
						</Button>
					</Group>
				}
			/>

			{data && isLicenseStatusDegraded(data.status) ? (
				<Text size="xs" c="dimmed">
					Your account is currently in a degraded entitlement state (
					{statusLabel.toLowerCase()}). Baseline non-account functionality
					remains available.
				</Text>
			) : null}
		</Stack>
	);
}
