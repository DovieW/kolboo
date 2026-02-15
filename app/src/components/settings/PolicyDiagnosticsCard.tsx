import { Badge, Group, Stack, Text } from "@mantine/core";
import type { PolicyState } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

export function formatPolicySourceLabel(source: PolicyState["source"]): string {
	if (source === "cloud") return "Cloud";
	if (source === "cached") return "Cached";
	if (source === "degraded_expired") return "Degraded";
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
	if (policy.source === "degraded_expired") return "Policy degraded (expired)";
	if (policy.source === "cached") return "Using cached policy";
	if (!policy.is_valid) return "Policy invalid";
	return "Policy active";
}

export function policyStatusColor(policy: PolicyState): string {
	if (policy.source === "degraded_expired" || !policy.is_valid) return "red";
	if (policy.source === "cached") return "yellow";
	if (policy.source === "none") return "gray";
	return "green";
}

export function PolicyDiagnosticsCard({
	policy,
}: {
	policy: PolicyState | undefined;
}) {
	const source = policy ? formatPolicySourceLabel(policy.source) : "Loading…";
	const status = policy ? policyStatusSummary(policy) : "Loading…";
	const enforcedCount = policy?.enforced_fields.length ?? 0;

	return (
		<Stack gap="md">
			<SettingsRow
				label="Policy status"
				description="Enterprise posture and enforcement metadata for this device."
				right={
					<Group gap="xs" align="center" justify="flex-end">
						<Badge color={policy ? policyStatusColor(policy) : "gray"}>
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
							Version: {policy?.version ?? "—"}
						</Text>
						<Text size="xs" c="dimmed">
							Synced: {formatPolicyTimestampLabel(policy?.last_sync_at ?? null)}
						</Text>
						<Text size="xs" c="dimmed">
							Success:{" "}
							{formatPolicyTimestampLabel(policy?.last_success_at ?? null)}
						</Text>
						<Text size="xs" c="dimmed">
							Updated:{" "}
							{formatPolicyTimestampLabel(policy?.last_updated ?? null)}
						</Text>
						<Text size="xs" c="dimmed">
							Expires: {formatPolicyTimestampLabel(policy?.expires_at ?? null)}
						</Text>
						{policy?.failure_reason ? (
							<Text size="xs" c="orange">
								Failure: {policy.failure_reason}
							</Text>
						) : null}
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
						{(policy?.enforced_fields ?? []).map((field) => (
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
		</Stack>
	);
}
