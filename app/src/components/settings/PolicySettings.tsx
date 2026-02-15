import { Badge, Button, Group, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { Download, RefreshCw, Shield } from "lucide-react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	usePolicyDiagnosticsExport,
	usePolicyState,
	usePolicySync,
} from "../../lib/queries";
import {
	formatPolicySourceLabel,
	formatPolicyTimestampLabel,
	PolicyDiagnosticsCard,
	policyStatusSummary,
} from "./PolicyDiagnosticsCard";
import { SettingsRow } from "./SettingsRow";

export {
	formatPolicySourceLabel,
	formatPolicyTimestampLabel,
	policyStatusSummary,
};

export function diagnosticsToJson(payload: unknown): string {
	return JSON.stringify(payload, null, 2);
}

export function PolicySettings() {
	const policy = usePolicyState();
	const syncPolicy = usePolicySync();
	const exportDiagnostics = usePolicyDiagnosticsExport();

	const handleSyncPolicy = async () => {
		try {
			await syncPolicy.mutateAsync(undefined);
			notifications.show({
				title: "Policy sync complete",
				message: "Policy state refreshed.",
				color: "green",
			});
		} catch (error) {
			notifications.show({
				title: "Policy sync failed",
				message: formatErrorMessage(error),
				color: "red",
			});
		}
	};

	const handleExportDiagnostics = async () => {
		try {
			const payload = await exportDiagnostics.mutateAsync();
			const json = diagnosticsToJson(payload);
			try {
				await navigator.clipboard.writeText(json);
				notifications.show({
					title: "Diagnostics copied",
					message: "Redacted policy diagnostics JSON copied to your clipboard.",
					color: "green",
				});
			} catch {
				notifications.show({
					title: "Diagnostics ready",
					message:
						"Could not access clipboard. Open browser devtools and copy from network response.",
					color: "yellow",
				});
			}
		} catch (error) {
			notifications.show({
				title: "Export failed",
				message: formatErrorMessage(error),
				color: "red",
			});
		}
	};

	const data = policy.data;
	const source = data ? formatPolicySourceLabel(data.source) : "Loading…";
	const status = data ? policyStatusSummary(data) : "Loading…";

	return (
		<Stack gap="md">
			<SettingsRow
				label="Policy status"
				description="Enterprise posture and enforcement metadata for this device."
				right={
					<Group gap="xs" align="center" justify="flex-end" wrap="wrap">
						<Button
							variant="default"
							size="xs"
							leftSection={<RefreshCw size={14} />}
							loading={syncPolicy.isPending}
							onClick={() => {
								void handleSyncPolicy();
							}}
						>
							Sync now
						</Button>
						<Badge color={data?.is_valid === false ? "red" : "green"}>
							{status}
						</Badge>
						<Badge variant="light" color="gray">
							{source}
						</Badge>
					</Group>
				}
			/>

			<PolicyDiagnosticsCard policy={data} />

			<SettingsRow
				label="Diagnostics export"
				description="Copies a redacted JSON diagnostics payload for support workflows."
				right={
					<Button
						variant="default"
						size="xs"
						leftSection={<Download size={14} />}
						loading={exportDiagnostics.isPending}
						onClick={() => {
							void handleExportDiagnostics();
						}}
					>
						Export diagnostics
					</Button>
				}
			/>

			<Group gap={8} align="center">
				<Shield size={14} />
				<Text size="xs" c="dimmed">
					Diagnostics are redacted and exclude API keys, auth tokens, and
					transcript content.
				</Text>
			</Group>
		</Stack>
	);
}
