import { Button, Card, Group, Stack, Text, Title } from "@mantine/core";

export function AccountActionsCard(props: {
  signedIn: boolean;
  reauthRequired: boolean;
  loginPending: boolean;
  refreshPending: boolean;
  logoutPending: boolean;
  managePending: boolean;
  onSignIn: () => void;
  onRefresh: () => void;
  onManage: () => void;
  onSignOut: () => void;
}) {
  const {
    signedIn,
    reauthRequired,
    loginPending,
    refreshPending,
    logoutPending,
    managePending,
    onSignIn,
    onRefresh,
    onManage,
    onSignOut,
  } = props;

  const showBrowserSignIn = !signedIn || reauthRequired;

  return (
    <Card withBorder radius="lg" className="account-panel">
      <Stack gap="md">
        <Text className="account-panel-kicker">Actions</Text>
        <Title order={3}>Manage your session</Title>
        <Text c="dimmed" size="sm">
          Use secure browser sign-in to connect or restore managed access, then
          refresh entitlement or open account management when needed.
        </Text>

        {showBrowserSignIn ? (
          <Stack gap="sm">
            <Text c="dimmed" size="sm">
              {reauthRequired
                ? "Kolboo will reopen your browser so you can restore managed access without entering credentials in-app."
                : "Kolboo will open your browser for secure sign-in and finish the desktop session after the callback returns."}
            </Text>

            <Group gap="sm" wrap="wrap">
              <Button onClick={onSignIn} loading={loginPending}>
                {reauthRequired ? "Re-authenticate" : "Sign in"}
              </Button>
              {signedIn ? (
                <Button
                  variant="default"
                  onClick={onSignOut}
                  loading={logoutPending}
                >
                  Sign out
                </Button>
              ) : null}
            </Group>
          </Stack>
        ) : null}

        <Group gap="sm" wrap="wrap">
          {signedIn ? (
            <>
              <Button
                variant="default"
                onClick={onRefresh}
                loading={refreshPending}
              >
                Refresh access
              </Button>
              <Button
                variant="light"
                onClick={onManage}
                loading={managePending}
              >
                Manage account
              </Button>
              <Button
                color="red"
                variant="subtle"
                onClick={onSignOut}
                loading={logoutPending}
              >
                Sign out
              </Button>
            </>
          ) : (
            <Text c="dimmed" size="sm">
              Sign in to unlock managed Personal or Managed Business access.
            </Text>
          )}
        </Group>
      </Stack>
    </Card>
  );
}
