import { Button, Checkbox, Group, Stack, Text } from "@mantine/core";
import { Download, Upload } from "lucide-react";
import {
	type CloudSyncUiState,
	getCloudSyncDisplayState,
} from "../../../lib/settings/dataLifecycle";
import { SettingsRow } from "../SettingsRow";

type DataCloudSyncSectionProps = {
	isProfileScope: boolean;
	globalOnlyTooltip: string;
	cloudSyncState: CloudSyncUiState | null | undefined;
	cloudSyncStateLoading: boolean;
	runCloudSyncActionPending: boolean;
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
	cloudSyncState,
	cloudSyncStateLoading,
	runCloudSyncActionPending,
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

	return (
		<>
			<SettingsRow
				label="Cloud sync (Personal+)"
				description="Sync settings via the managed cloud endpoint using your signed-in account token."
				right={
					<Stack gap={8} style={{ width: "min(640px, 100%)" }}>
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
								disabled={isProfileScope || updateCloudSyncEnabledPending}
								onChange={(event) => {
									onCloudSyncEnabledChange(event.currentTarget.checked);
								}}
							/>
							<Checkbox
								label="Auto-push changes"
								checked={cloudSyncDisplay.autoPush}
								disabled={
									isProfileScope ||
									updateCloudSyncAutoPushPending ||
									!cloudSyncDisplay.enabled
								}
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
				description="Enabled by default (privacy-safe). Can be enforced by policy in managed enterprise environments."
				right={
					<Group gap="md" justify="flex-end" wrap="nowrap">
						<Checkbox
							label="Enable privacy-safe analytics"
							checked={cloudSyncDisplay.posthogAnalyticsEnabled}
							disabled={isProfileScope || updatePosthogAnalyticsEnabledPending}
							onChange={(event) => {
								onPosthogAnalyticsEnabledChange(event.currentTarget.checked);
							}}
						/>
					</Group>
				}
			/>
		</>
	);
}
