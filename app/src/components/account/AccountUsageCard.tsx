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
					{used.toLocaleString(undefined, { maximumFractionDigits: 2 })} / {limit.toLocaleString(undefined, { maximumFractionDigits: 2 })} {unit}
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
    if (!loading && (!licenseState || !shouldShowManagedUsage(modeLabel, licenseState))) return null;

	return (
		<Card withBorder radius="lg" className="account-panel">
			<Stack gap="md">
				<Title order={3}>Your allowance</Title>
                {licenseState?.beta_access?.status === "approved" && <Text size="sm" c="dimmed">Complimentary Pro beta</Text>}
                {licenseState?.usage_period && <Text size="xs" c="dimmed">Monthly reset: {new Date(licenseState.usage_period.monthly_reset_at).toLocaleDateString("en-US", { timeZone: "UTC" })} · Daily reset: {new Date(licenseState.usage_period.daily_reset_at).toLocaleDateString("en-US", { timeZone: "UTC" })}, 00:00 UTC</Text>}

				{loading ? (
					<Stack gap="sm">
						<Skeleton height={18} width="65%" />
						<Skeleton height={16} width="100%" />
						<Skeleton height={16} width="100%" />
					</Stack>
				) : licenseState && (
					<Stack gap="md">
						<UsageMeter
							label="Audio hours"
							used={licenseState.usage.stt_seconds_used / 3600}
							limit={licenseState.limits.stt_seconds_monthly / 3600}
							unit="hours"
						/>
						<UsageMeter
							label="LLM tokens"
							used={licenseState.usage.llm_tokens_used}
							limit={licenseState.limits.llm_tokens_monthly}
							unit="tokens"
						/>
						<UsageMeter
							label="Daily managed requests"
							used={licenseState.usage.requests_today}
							limit={licenseState.limits.requests_per_day}
							unit="today"
						/>
					</Stack>
				)}
			</Stack>
		</Card>
	);
}
