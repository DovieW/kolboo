import { Badge, Button, Group, Stack, Text } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { useMutation } from "@tanstack/react-query";
import { Download, Shield } from "lucide-react";
import { formatErrorMessage } from "../../lib/formatError";
import { usePolicyState } from "../../lib/queries";
import { type PolicyState, policyAPI } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

export function formatPolicySourceLabel(source: PolicyState["source"]): string {
	if (source === "cloud") return "Cloud";
	if (source === "file") return "Local file";
	return "Unmanaged";
}

export function formatPolicyTimestampLabel(value: string | null): string {
	if (!value) return "—";
	const parsed = Date.parse(value);
	if (Number.isNaN(parsed)) return "—";
	return new Date(parsed).toLocaleString();
}

export function policyStatusSummary(policy: PolicyState): string {
	if (policy.source === "none") return "No active policy";
	if (!policy.is_valid) return "Policy invalid";
	return "Policy active";
}

export function diagnosticsToJson(payload: unknown): string {
	return JSON.stringify(payload, null, 2);
}

export function PolicySettings() {
	const policy = usePolicyState();

	const exportDiagnostics = useMutation({
		mutationFn: () => policyAPI.exportPolicyDiagnostics(),
		onSuccess: async (payload) => {
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
		},
		onError: (error) => {
			notifications.show({
				title: "Export failed",
				message: formatErrorMessage(error),
				color: "red",
			});
		},
	});

	const data = policy.data;
	const source = data ? formatPolicySourceLabel(data.source) : "Loading…";
	const status = data ? policyStatusSummary(data) : "Loading…";
	const enforcedCount = data?.enforced_fields.length ?? 0;

	return (
		<Stack gap="md">
			<SettingsRow
				label="Policy status"
				description="Enterprise posture and enforcement metadata for this device."
				right={
					<Group gap="xs" align="center" justify="flex-end">
						<Badge color={data?.is_valid === false ? "red" : "green"}>
							{status}
						</Badge>
						<Badge variant="light" color="gray">
							{source}
						</Badge>
					</Group>
				}
			/>

			<SettingsRow
				label="Policy metadata"
				description="Version and timing details for the active policy state."
				right={
					<Stack gap={2} align="flex-end">
						<Text size="xs" c="dimmed">
							Version: {data?.version ?? "—"}
						</Text>
						<Text size="xs" c="dimmed">
							Updated: {formatPolicyTimestampLabel(data?.last_updated ?? null)}
						</Text>
						<Text size="xs" c="dimmed">
							Expires: {formatPolicyTimestampLabel(data?.expires_at ?? null)}
						</Text>
					</Stack>
				}
			/>

			<SettingsRow
				label="Enforced fields"
				description="Settings currently controlled by policy."
				right={
					<Stack gap={4} align="flex-end" style={{ maxWidth: 520 }}>
						<Text size="xs" c="dimmed">
							{enforcedCount} enforced field{enforcedCount === 1 ? "" : "s"}
						</Text>
						{(data?.enforced_fields ?? []).map((field) => (
							<Group key={field.path} gap={6} justify="flex-end" wrap="wrap">
								<Badge variant="outline" color="orange">
									{field.path}
								</Badge>
								{field.reason ? (
									<Text size="xs" c="dimmed">
										{field.reason}
									</Text>
								) : null}
							</Group>
						))}
					</Stack>
				}
			/>

			<SettingsRow
				label="Diagnostics export"
				description="Copies a redacted JSON diagnostics payload for support workflows."
				right={
					<Button
						variant="default"
						size="xs"
						leftSection={<Download size={14} />}
						loading={exportDiagnostics.isPending}
						onClick={() => exportDiagnostics.mutate()}
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
