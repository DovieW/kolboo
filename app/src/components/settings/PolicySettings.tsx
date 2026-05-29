import { Badge, Button, Group, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { Download, RefreshCw, Shield } from "lucide-react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	usePolicyDiagnosticsExport,
	usePolicyState,
	usePolicySync,
} from "../../lib/queries";
import { logsAPI, tauriAPI } from "../../lib/tauri";
import { loadRuntimeConfig } from "../../lib/tauri/runtimeConfig";
import { PolicyDiagnosticsCard } from "./PolicyDiagnosticsCard";
import {
	buildSupportDiagnosticsBundle,
	diagnosticsToJson,
	formatPolicySourceLabel,
	policyStatusSummary,
} from "./policyDiagnostics";
import { SettingsRow } from "./SettingsRow";

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
			const policyPayload = await exportDiagnostics.mutateAsync();
			const [
				licenseStateResult,
				authContextResult,
				requestLogsResult,
				runtimeConfigResult,
			] = await Promise.allSettled([
				tauriAPI.getLicenseState(),
				tauriAPI.getLicenseAuthContext(),
				logsAPI.getRequestLogs(10),
				loadRuntimeConfig(),
			]);

			const payload = await buildSupportDiagnosticsBundle({
				policyExport: policyPayload,
				runtimeConfig:
					runtimeConfigResult.status === "fulfilled"
						? runtimeConfigResult.value
						: null,
				licenseState:
					licenseStateResult.status === "fulfilled"
						? licenseStateResult.value
						: null,
				authContext:
					authContextResult.status === "fulfilled"
						? authContextResult.value
						: null,
				requestLogs:
					requestLogsResult.status === "fulfilled"
						? requestLogsResult.value
						: [],
			});
			const json = diagnosticsToJson(payload);
			try {
				await navigator.clipboard.writeText(json);
				notifications.show({
					title: "Diagnostics copied",
					message:
						"Redacted support diagnostics bundle copied to your clipboard.",
					color: "green",
				});
			} catch {
				notifications.show({
					title: "Diagnostics ready",
					message:
						"Could not access the clipboard. Please retry after granting clipboard access.",
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
				description="Copies a redacted support bundle with policy state, request IDs, and operator handoff hints."
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
					Diagnostics are redacted and exclude API keys, auth tokens, transcript
					content, raw org names, and raw internal IDs.
				</Text>
			</Group>
		</Stack>
	);
}
