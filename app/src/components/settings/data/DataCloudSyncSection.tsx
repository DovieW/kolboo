import { Badge, Button, Checkbox, Group, Stack, Text } from "@mantine/core";
import { Download, Upload } from "lucide-react";
import type { CloudSyncAccessState } from "../../../lib/settings/dataBackupCloudSync";
import {
	type CloudSyncUiState,
	getCloudSyncDisplayState,
} from "../../../lib/settings/dataLifecycle";
import { SettingsRow } from "../SettingsRow";

type DataCloudSyncSectionProps = {
	isProfileScope: boolean;
	globalOnlyTooltip: string;
	analyticsPolicyEnforced: boolean;
	analyticsPolicyReason: string | null;
	cloudSyncState: CloudSyncUiState | null | undefined;
	cloudSyncStateLoading: boolean;
	runCloudSyncActionPending: boolean;
	cloudSyncAccess: CloudSyncAccessState;
	onPushCloudSync: () => void;
	onPullCloudSync: () => void;
	updateCloudSyncEnabledPending: boolean;
	onCloudSyncEnabledChange: (enabled: boolean) => void;
	updateCloudSyncAutoPushPending: boolean;
	onCloudSyncAutoPushChange: (enabled: boolean) => void;
	updatePosthogAnalyticsEnabledPending: boolean;
	onPosthogAnalyticsEnabledChange: (enabled: boolean) => void;
};

export function DataCloudSyncSection({
	isProfileScope,
	globalOnlyTooltip,
	analyticsPolicyEnforced,
	analyticsPolicyReason,
	cloudSyncState,
	cloudSyncStateLoading,
	runCloudSyncActionPending,
	cloudSyncAccess,
	onPushCloudSync,
	onPullCloudSync,
	updateCloudSyncEnabledPending,
	onCloudSyncEnabledChange,
	updateCloudSyncAutoPushPending,
	onCloudSyncAutoPushChange,
	updatePosthogAnalyticsEnabledPending,
	onPosthogAnalyticsEnabledChange,
}: DataCloudSyncSectionProps) {
	const cloudSyncDisplay = getCloudSyncDisplayState(cloudSyncState);
	// If someone downgrades after previously enabling cloud sync, keep the
	// persisted checks visible long enough for them to turn those flags off
	// locally. The plan gate only blocks turning sync back on.
	const enableCloudSyncDisabled =
		isProfileScope ||
		updateCloudSyncEnabledPending ||
		cloudSyncAccess.status === "loading" ||
		(!cloudSyncAccess.canUseCloudSync && !cloudSyncDisplay.enabled);
	const autoPushDisabled =
		isProfileScope ||
		updateCloudSyncAutoPushPending ||
		!cloudSyncDisplay.enabled ||
		cloudSyncAccess.status === "loading" ||
		(!cloudSyncAccess.canUseCloudSync && !cloudSyncDisplay.autoPush);
	const analyticsDescription = analyticsPolicyEnforced
		? "Privacy-safe, event-only analytics stay disabled because your organization enforces that posture. No transcripts, prompts, audio, OCR payloads, or session replay are sent."
		: cloudSyncDisplay.telemetryDisclosureResolved
			? "Privacy-safe, event-only analytics. No transcripts, prompts, audio, OCR payloads, or session replay. You can change this later in Settings → Data."
			: "Privacy-safe, event-only analytics. No transcripts, prompts, audio, OCR payloads, or session replay. Analytics stay paused until you review the first-run disclosure.";

	return (
		<>
			<SettingsRow
				label="Cloud sync (Personal+)"
				description="Sync settings via the managed cloud endpoint using your signed-in account token. Available on Personal/Pro and Managed Business."
				right={
					<Stack gap={8} style={{ width: "min(640px, 100%)" }}>
						{cloudSyncAccess.status !== "included" &&
						cloudSyncAccess.status !== "loading" ? (
							<Stack gap={4} align="flex-end">
								<Badge variant="light" color={cloudSyncAccess.badgeColor}>
									{cloudSyncAccess.badgeLabel}
								</Badge>
								<Text size="xs" c="dimmed" ta="right">
									{cloudSyncAccess.helperLabel}
								</Text>
							</Stack>
						) : null}

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Text size="xs" c="dimmed">
								Last push: {cloudSyncDisplay.lastPushedLabel}
							</Text>
							<Text size="xs" c="dimmed">
								Last pull: {cloudSyncDisplay.lastPulledLabel}
							</Text>
						</Group>

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Button
								variant="default"
								size="xs"
								leftSection={<Upload size={14} />}
								loading={runCloudSyncActionPending}
								disabled={
									isProfileScope ||
									!cloudSyncAccess.canUseCloudSync ||
									!cloudSyncDisplay.enabled ||
									cloudSyncStateLoading
								}
								onClick={onPushCloudSync}
							>
								Push now
							</Button>

							<Button
								variant="default"
								size="xs"
								leftSection={<Download size={14} />}
								loading={runCloudSyncActionPending}
								disabled={
									isProfileScope ||
									!cloudSyncAccess.canUseCloudSync ||
									!cloudSyncDisplay.enabled ||
									cloudSyncStateLoading
								}
								onClick={onPullCloudSync}
							>
								Pull now
							</Button>
						</Group>

						<Group gap="md" align="center" justify="flex-end">
							<Checkbox
								label="Enable cloud sync"
								checked={cloudSyncDisplay.enabled}
								disabled={enableCloudSyncDisabled}
								onChange={(event) => {
									onCloudSyncEnabledChange(event.currentTarget.checked);
								}}
							/>
							<Checkbox
								label="Auto-push changes"
								checked={cloudSyncDisplay.autoPush}
								disabled={autoPushDisabled}
								onChange={(event) => {
									onCloudSyncAutoPushChange(event.currentTarget.checked);
								}}
							/>
						</Group>

						<Text size="xs" c={cloudSyncDisplay.footerTone} ta="right">
							{cloudSyncDisplay.footerLabel}
						</Text>

						{isProfileScope ? (
							<Text size="xs" c="dimmed" ta="right">
								{globalOnlyTooltip}
							</Text>
						) : null}
					</Stack>
				}
			/>

			<SettingsRow
				label="Product analytics (PostHog)"
				description={analyticsDescription}
				right={
					<Stack gap={6} align="flex-end">
						{analyticsPolicyEnforced ? (
							<Group gap={6} justify="flex-end" wrap="wrap">
								<Badge variant="light" color="orange">
									Policy enforced
								</Badge>
								{analyticsPolicyReason ? (
									<Text size="xs" c="dimmed">
										{analyticsPolicyReason}
									</Text>
								) : null}
							</Group>
						) : null}

						<Group gap="md" justify="flex-end" wrap="nowrap">
							<Checkbox
								label="Enable privacy-safe analytics"
								checked={cloudSyncDisplay.posthogAnalyticsEnabled}
								disabled={
									isProfileScope ||
									updatePosthogAnalyticsEnabledPending ||
									analyticsPolicyEnforced
								}
								onChange={(event) => {
									onPosthogAnalyticsEnabledChange(event.currentTarget.checked);
								}}
							/>
						</Group>

						{analyticsPolicyEnforced ? (
							<Text size="xs" c="dimmed" ta="right">
								Organization policy currently keeps product analytics disabled.
							</Text>
						) : null}

						{!cloudSyncDisplay.telemetryDisclosureResolved ? (
							<Text size="xs" c="dimmed" ta="right">
								First-run disclosure pending. Analytics remain paused until you
								review it.
							</Text>
						) : null}
					</Stack>
				}
			/>
		</>
	);
}
