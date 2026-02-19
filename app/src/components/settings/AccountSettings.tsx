import {
	Badge,
	Button,
	Group,
	PasswordInput,
	Progress,
	Stack,
	Text,
	TextInput,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { ExternalLink, RefreshCcw, ShieldUser } from "lucide-react";
import { useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	useLicenseState,
	useLogoutLicense,
	useRefreshLicenseEntitlement,
	useStartLicenseLogin,
} from "../../lib/queries";
import { type LicenseState, licenseAPI } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";
import { TestPersonaIndicator } from "./TestPersonaIndicator";

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
	const [email, setEmail] = useState("");
	const [password, setPassword] = useState("");

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

	const monthlySttProgress = data
		? data.limits.stt_seconds_monthly > 0
			? Math.min(
					100,
					Math.round(
						(data.usage.stt_seconds_used / data.limits.stt_seconds_monthly) *
							100,
					),
				)
			: 0
		: 0;

	const monthlyLlmProgress = data
		? data.limits.llm_tokens_monthly > 0
			? Math.min(
					100,
					Math.round(
						(data.usage.llm_tokens_used / data.limits.llm_tokens_monthly) * 100,
					),
				)
			: 0
		: 0;

	const canSubmitSignIn =
		email.trim().length > 0 && password.trim().length > 0 && !signedIn;

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

	const handleSignIn = () => {
		if (!canSubmitSignIn) {
			notifications.show({
				title: "Enter your credentials",
				message: "Email and password are required to sign in.",
				color: "yellow",
			});
			return;
		}

		login.mutate(
			{
				provider_hint: "personal",
				email: email.trim(),
				password,
			},
			{
				onSuccess: () => {
					setPassword("");
				},
				onError: (error) => {
					notifications.show({
						title: "Sign in failed",
						message: formatErrorMessage(error),
						color: "red",
					});
				},
			},
		);
	};

	return (
    <Stack gap="md">
      <TestPersonaIndicator />

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

      {data?.tier === "personal" && signedIn ? (
        <SettingsRow
          label="Managed usage"
          description="Current personal-tier usage against monthly managed limits."
          right={
            <Stack gap={6} w={240}>
              <Text size="xs" c="dimmed">
                STT: {data.usage.stt_seconds_used.toLocaleString()} /{" "}
                {data.limits.stt_seconds_monthly.toLocaleString()} sec
              </Text>
              <Progress value={monthlySttProgress} size="sm" />
              <Text size="xs" c="dimmed">
                LLM: {data.usage.llm_tokens_used.toLocaleString()} /{" "}
                {data.limits.llm_tokens_monthly.toLocaleString()} tokens
              </Text>
              <Progress value={monthlyLlmProgress} size="sm" />
            </Stack>
          }
        />
      ) : null}

      {signedIn ? (
        <SettingsRow
          label="Managed recovery"
          description="If managed inference is temporarily unavailable, you can continue by selecting BYOK providers in Speech and Rewrite settings."
          right={
            <Text size="xs" c="dimmed" ta="right" maw={260}>
              Managed outages are usually short. Retry first, then switch to
              BYOK if you need uninterrupted flow.
            </Text>
          }
        />
      ) : null}

      <SettingsRow
        label="Session actions"
        description="Sign in with your Supabase account, sign out, refresh entitlement, or open account management."
        right={
          <Stack gap="xs" w={360}>
            {!signedIn ? (
              <>
                <TextInput
                  size="xs"
                  label="Email"
                  placeholder="you@example.com"
                  value={email}
                  onChange={(event) => setEmail(event.currentTarget.value)}
                  autoComplete="email"
                />
                <PasswordInput
                  size="xs"
                  label="Password"
                  placeholder="Enter your password"
                  value={password}
                  onChange={(event) => setPassword(event.currentTarget.value)}
                  autoComplete="current-password"
                />
              </>
            ) : null}

            <Group gap="xs" justify="flex-end">
              <Button
                variant="default"
                size="xs"
                leftSection={<ShieldUser size={14} />}
                loading={login.isPending}
                onClick={handleSignIn}
                disabled={!canSubmitSignIn}
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
          </Stack>
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
