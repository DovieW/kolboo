import { Alert, Stack, Text, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMemo, useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	useLicenseAuthContext,
	useLicenseState,
	useLogoutLicense,
	useRefreshLicenseEntitlement,
	useStartLicenseLogin,
} from "../../lib/queries";
import { tauriAPI } from "../../lib/tauri";
import { authReasonCodeToMessage } from "../../lib/tauri/license";
import { AccountActionsCard } from "./AccountActionsCard";
import { AccountAdvancedPanel } from "./AccountAdvancedPanel";
import { AccountIdentityCard } from "./AccountIdentityCard";
import { AccountSummaryCard } from "./AccountSummaryCard";
import { AccountUsageCard } from "./AccountUsageCard";
import {
	formatAccountStatusLabel,
	formatInternalTierLabel,
	getAccountModeDescription,
	getAccountModeLabel,
	getAccountStatusColor,
	isReauthRequiredReason,
} from "./accountPresentation";

export function AccountView() {
	const licenseState = useLicenseState();
	const authContext = useLicenseAuthContext();
	const startLogin = useStartLicenseLogin();
	const logout = useLogoutLicense();
	const refresh = useRefreshLicenseEntitlement();

	const [managePending, setManagePending] = useState(false);

	const state = licenseState.data;
	const context = authContext.data;
	const signedIn = state?.status !== "signed_out";
	const reauthRequired = isReauthRequiredReason(context?.reason_code);
	const modeLabel = getAccountModeLabel(state, context);
	const modeDescription = getAccountModeDescription({
		modeLabel,
		signedIn,
		reauthRequired,
	});
	const statusLabel = state
		? formatAccountStatusLabel(state.status)
		: "Loading…";
	const statusColor = getAccountStatusColor({
		status: state?.status,
		reauthRequired,
	});
	const internalTierLabel = state
		? formatInternalTierLabel(state.tier)
		: "Loading…";
	const authContextMessage =
		authReasonCodeToMessage(context?.reason_code ?? null) ??
		"No auth issue detected.";
	const headerSubtitle = useMemo(() => {
		if (modeLabel === "Managed Business") {
			return "A cleaner view of managed access, identity, usage, and recovery.";
		}
		if (modeLabel === "Personal") {
			return "Everything about your managed personal account in one place.";
		}
		return "See your current setup, usage, and sign-in actions without digging through settings.";
	}, [modeLabel]);

	const queryError = licenseState.error ?? authContext.error;
	const loginTierHint =
    state?.tier === "enterprise" ? "enterprise" : "personal";

	const handleSignIn = () => {
		startLogin.mutate(
      { provider_hint: loginTierHint },
      {
        onSuccess: () => {
          notifications.show({
            title: reauthRequired ? "Re-authenticated" : "Signed in",
            message: reauthRequired
              ? "Managed access has been restored."
              : "Your account is now connected.",
            color: "green",
          });
        },
        onError: (error) => {
          notifications.show({
            title: reauthRequired
              ? "Re-authentication failed"
              : "Sign-in failed",
            message: formatErrorMessage(error),
            color: "red",
          });
        },
      },
    );
	};

	const handleRefresh = () => {
		refresh.mutate(undefined, {
			onSuccess: () => {
				notifications.show({
					title: "Access refreshed",
					message: "Latest entitlement data has been loaded.",
					color: "green",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Refresh failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleSignOut = () => {
		logout.mutate(undefined, {
      onSuccess: () => {
        notifications.show({
          title: "Signed out",
          message: "Managed session data has been cleared from this device.",
          color: "blue",
        });
      },
      onError: (error) => {
        notifications.show({
          title: "Sign-out failed",
          message: formatErrorMessage(error),
          color: "red",
        });
      },
    });
	};

	const handleManage = async () => {
		setManagePending(true);
		try {
			const url = await tauriAPI.getLicenseManagementUrl();
			window.open(url, "_blank", "noopener,noreferrer");
		} catch (error) {
			notifications.show({
				title: "Unable to open account management",
				message: formatErrorMessage(error),
				color: "red",
			});
		} finally {
			setManagePending(false);
		}
	};

	const handleSimulateAuthFailure = () => {
		refresh.mutate(true, {
			onSuccess: () => {
				notifications.show({
					title: "Auth failure simulated",
					message: "You should now see the re-authentication path.",
					color: "orange",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Simulation failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	return (
    <div className="main-content">
      <header className="tv-page-header animate-in">
        <Title order={1} mb={4}>
          Account
        </Title>
        <Text c="dimmed" size="sm">
          {headerSubtitle}
        </Text>
      </header>

      <div className="main-content-inner">
        <Stack gap="lg" className="account-page-stack">
          {queryError ? (
            <Alert
              color="red"
              variant="light"
              title="Unable to load account details"
            >
              {formatErrorMessage(queryError)}
            </Alert>
          ) : null}

          <AccountSummaryCard
            loading={licenseState.isLoading || authContext.isLoading}
            modeLabel={modeLabel}
            modeDescription={modeDescription}
            statusLabel={statusLabel}
            statusColor={statusColor}
            email={state?.email ?? null}
            organizationLabel={state?.org?.org_name ?? null}
            reauthRequired={reauthRequired}
          />

          <div className="account-page-grid">
            <AccountIdentityCard
              loading={licenseState.isLoading || authContext.isLoading}
              email={state?.email ?? null}
              organizationLabel={state?.org?.org_name ?? null}
              organizationId={state?.org?.org_id ?? null}
              subject={state?.user_id ?? context?.subject_id ?? null}
              internalTierLabel={internalTierLabel}
            />
            <AccountUsageCard
              loading={licenseState.isLoading}
              modeLabel={modeLabel}
              licenseState={state}
            />
          </div>

          <AccountActionsCard
            signedIn={signedIn}
            reauthRequired={reauthRequired}
            loginPending={startLogin.isPending}
            refreshPending={refresh.isPending}
            logoutPending={logout.isPending}
            managePending={managePending}
            onSignIn={handleSignIn}
            onRefresh={handleRefresh}
            onManage={handleManage}
            onSignOut={handleSignOut}
          />

          <AccountAdvancedPanel
            authContext={context}
            authContextMessage={authContextMessage}
            signedIn={signedIn}
            refreshPending={refresh.isPending}
            onSimulateAuthFailure={handleSimulateAuthFailure}
          />
        </Stack>
      </div>
    </div>
  );
}
