import { Accordion, Alert, Button, Card, Stack, Text, Title } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";
import { isCloudServiceAvailable } from "../../lib/cloudService";
import { useLicenseAuthContext, useLicenseState, useLogoutLicense, useRefreshLicenseEntitlement } from "../../lib/queries";
import { tauriAPI } from "../../lib/tauri";
import { authReasonCodeToMessage } from "../../lib/tauri/license";
import { AccountActionsCard } from "./AccountActionsCard";
import { AccountAuthentication } from "./AccountAuthentication";
import { AccountAdvancedPanel } from "./AccountAdvancedPanel";
import { AccountIdentityCard } from "./AccountIdentityCard";
import { AccountSummaryCard } from "./AccountSummaryCard";
import { AccountUsageCard } from "./AccountUsageCard";
import { formatAccountStatusLabel, formatInternalTierLabel, getAccountModeDescription, getAccountModeLabel, getAccountStatusColor, isReauthRequiredForSession } from "./accountPresentation";

export function AccountView() {
    const license = useLicenseState();
    const auth = useLicenseAuthContext();
    const logout = useLogoutLicense();
    const refresh = useRefreshLicenseEntitlement();
    const [managePending, setManagePending] = useState(false);
    const state = license.data;
    const context = auth.data;
    const signedIn = Boolean(state && state.status !== "signed_out");
    const reauthRequired = isReauthRequiredForSession(signedIn, context?.reason_code);
    const modeLabel = getAccountModeLabel(state, context);
    const queryError = license.error ?? auth.error;
    function showError(title: string, error: unknown) {
        notifications.show({ title, message: formatErrorMessage(error), color: "red" });
    }
    function refreshAccess(simulateFailure = false) {
        refresh.mutate(simulateFailure, { onError: error => showError("Refresh failed", error) });
    }
    function signOut() {
        logout.mutate(undefined, { onError: error => showError("Sign-out failed", error) });
    }
    async function manageAccount() {
        setManagePending(true);
        try { await openUrl(await tauriAPI.getLicenseManagementUrl()); }
        catch (error) { showError("Unable to open account management", error); }
        finally { setManagePending(false); }
    }
    if (!isCloudServiceAvailable()) return <div className="main-content account-signin-layout">
        <div className="main-content-inner page-content-start"><Card withBorder radius="lg"><Stack>
            <Title order={2} size="h3">Community</Title>
            <Text>Accounts will be available in a later update. Use your own provider keys in Settings.</Text>
            {signedIn && <Button variant="subtle" color="gray" onClick={signOut} loading={logout.isPending}>Sign out</Button>}
        </Stack></Card></div>
    </div>;
    return <div className={`main-content ${signedIn ? "" : "account-signin-layout"}`}>
        <div className="main-content-inner page-content-start"><Stack gap="lg" className="account-page-stack">
            {queryError && <Alert color="red" title="Unable to load account details">{formatErrorMessage(queryError)}</Alert>}
            {state && signedIn && <>
                <AccountSummaryCard loading={license.isLoading || auth.isLoading}
                    modeLabel={modeLabel} modeDescription={getAccountModeDescription({ modeLabel, signedIn, reauthRequired })}
                    statusLabel={formatAccountStatusLabel(state.status)} statusColor={getAccountStatusColor({ status: state.status, reauthRequired })}
                    email={state.email ?? null} organizationLabel={state.org?.org_name ?? null} signedIn={signedIn} reauthRequired={reauthRequired} />
                <AccountUsageCard loading={license.isLoading} modeLabel={modeLabel} licenseState={state} />
            </>}
            <AccountActionsCard signedIn={signedIn} reauthRequired={reauthRequired}
                refreshPending={refresh.isPending} logoutPending={logout.isPending} managePending={managePending}
                manageAvailable={state?.portal_available === true} authForm={<AccountAuthentication />}
                onRefresh={() => refreshAccess()} onManage={() => void manageAccount()} onSignOut={signOut} />
            {state && signedIn && <Accordion variant="default"><Accordion.Item value="details">
                <Accordion.Control>Account details</Accordion.Control><Accordion.Panel><Stack gap="md">
                    <AccountIdentityCard loading={license.isLoading || auth.isLoading} email={state.email ?? null}
                        organizationLabel={state.org?.org_name ?? null} organizationId={state.org?.org_id ?? null}
                        subject={state.user_id ?? context?.subject_id ?? null} internalTierLabel={formatInternalTierLabel(state.tier)} />
                    <AccountAdvancedPanel authContext={context} authContextMessage={authReasonCodeToMessage(context?.reason_code ?? null) ?? "No auth issue detected."}
                        signedIn={signedIn} refreshPending={refresh.isPending} onSimulateAuthFailure={() => refreshAccess(true)} />
                </Stack></Accordion.Panel>
            </Accordion.Item></Accordion>}
        </Stack></div>
    </div>;
}
