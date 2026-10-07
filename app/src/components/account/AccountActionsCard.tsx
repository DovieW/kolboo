import { Button, Card, Group, Stack, Title } from "@mantine/core";
import type { ReactNode } from "react";

export function AccountActionsCard(props: {
    signedIn: boolean; reauthRequired: boolean; refreshPending: boolean;
    logoutPending: boolean; managePending: boolean; manageAvailable: boolean;
    authForm: ReactNode; onRefresh: () => void; onManage: () => void; onSignOut: () => void;
}) {
    return <Card withBorder radius="lg" className="account-panel account-actions">
        <Stack gap="md">
            <Title order={2} size="h3">{!props.signedIn || props.reauthRequired ? "Sign in to Kolboo" : "Your account"}</Title>
            {(!props.signedIn || props.reauthRequired) && props.authForm}
            {props.signedIn && <Group gap="sm">
                <Button variant="default" onClick={props.onRefresh} loading={props.refreshPending}>Refresh access</Button>
                {props.manageAvailable && <Button variant="light" onClick={props.onManage} loading={props.managePending}>Manage account</Button>}
                <Button color="gray" variant="subtle" onClick={props.onSignOut} loading={props.logoutPending}>Sign out</Button>
            </Group>}
        </Stack>
    </Card>;
}
