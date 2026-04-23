import { Card, Progress, Skeleton, Stack, Text, Title } from "@mantine/core";
import type { LicenseState } from "../../lib/tauri";
import type { AccountModeLabel } from "./accountPresentation";
import {
	calculateUsagePercent,
	shouldShowManagedUsage,
} from "./accountPresentation";

function UsageMeter(props: {
	label: string;
	used: number;
	limit: number;
	unit: string;
}) {
	const { label, used, limit, unit } = props;
	const percent = calculateUsagePercent(used, limit);

	return (
		<Stack gap={6}>
			<div className="account-usage-header">
				<Text className="account-detail-label">{label}</Text>
				<Text className="account-detail-value">
					{used.toLocaleString()} / {limit.toLocaleString()} {unit}
				</Text>
			</div>
			<Progress
				value={percent}
				size="lg"
				radius="xl"
				color={percent >= 90 ? "red" : percent >= 70 ? "yellow" : "green"}
			/>
		</Stack>
	);
}

export function AccountUsageCard(props: {
	loading: boolean;
	modeLabel: AccountModeLabel;
	licenseState: LicenseState | null | undefined;
}) {
	const { loading, modeLabel, licenseState } = props;

	return (
		<Card withBorder radius="lg" className="account-panel">
			<Stack gap="md">
				<Text className="account-panel-kicker">Usage</Text>
				<Title order={3}>Managed usage and limits</Title>

				{loading ? (
					<Stack gap="sm">
						<Skeleton height={18} width="65%" />
						<Skeleton height={16} width="100%" />
						<Skeleton height={16} width="100%" />
					</Stack>
				) : shouldShowManagedUsage(modeLabel, licenseState) ? (
					<Stack gap="md">
						<UsageMeter
							label="Speech-to-text seconds"
							used={licenseState?.usage.stt_seconds_used ?? 0}
							limit={licenseState?.limits.stt_seconds_monthly ?? 0}
							unit="sec"
						/>
						<UsageMeter
							label="LLM tokens"
							used={licenseState?.usage.llm_tokens_used ?? 0}
							limit={licenseState?.limits.llm_tokens_monthly ?? 0}
							unit="tokens"
						/>
						<UsageMeter
							label="Daily managed requests"
							used={licenseState?.usage.requests_today ?? 0}
							limit={licenseState?.limits.requests_per_day ?? 0}
							unit="today"
						/>
					</Stack>
				) : (
					<Text c="dimmed" size="sm">
						Managed usage is only shown when you are signed in with Personal or
						Managed Business access. In BYOK mode, your own providers handle
						usage.
					</Text>
				)}
			</Stack>
		</Card>
	);
}
