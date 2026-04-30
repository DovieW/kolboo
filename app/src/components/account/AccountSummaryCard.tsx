import {
	Badge,
	Card,
	Group,
	Skeleton,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import type { AccountModeLabel } from "./accountPresentation";

export function AccountSummaryCard(props: {
	loading: boolean;
	modeLabel: AccountModeLabel;
	modeDescription: string;
	statusLabel: string;
	statusColor: string;
	email: string | null;
	organizationLabel: string | null;
	signedIn: boolean;
	reauthRequired: boolean;
}) {
	const {
		loading,
		modeLabel,
		modeDescription,
		statusLabel,
		statusColor,
		email,
		organizationLabel,
		signedIn,
		reauthRequired,
	} = props;
	const summaryTitle = signedIn ? modeLabel : "Sign in to use managed access";

	if (loading) {
		return (
			<Card
				withBorder
				radius="lg"
				className="account-panel account-summary-card"
			>
				<Stack gap="sm">
					<Skeleton height={14} width={110} />
					<Skeleton height={34} width={240} />
					<Skeleton height={16} width="75%" />
					<Skeleton height={72} radius="md" />
				</Stack>
			</Card>
		);
	}

	return (
		<Card withBorder radius="lg" className="account-panel account-summary-card">
			<Stack gap="lg">
				<Group justify="space-between" align="flex-start" gap="md" wrap="wrap">
					<Stack gap={6}>
						<Text className="account-panel-kicker">Current setup</Text>
						<Title order={2} className="account-summary-title">
							{summaryTitle}
						</Title>
						<Text c="dimmed" size="sm" maw={620}>
							{modeDescription}
						</Text>
					</Stack>

					<Group gap="xs" wrap="wrap" justify="flex-end">
						<Badge color={statusColor}>{statusLabel}</Badge>
						{organizationLabel ? (
							<Badge variant="light" color="blue">
								{organizationLabel}
							</Badge>
						) : null}
					</Group>
				</Group>

				<div className="account-summary-meta-grid">
					<div className="account-meta-pill">
						<Text className="account-meta-label">Account</Text>
						<Text className="account-meta-value">
							{email ?? "Not signed in"}
						</Text>
					</div>
					<div className="account-meta-pill">
						<Text className="account-meta-label">Current mode</Text>
						<Text className="account-meta-value">{modeLabel}</Text>
					</div>
					<div className="account-meta-pill">
						<Text className="account-meta-label">Managed access</Text>
						<Text className="account-meta-value">{statusLabel}</Text>
					</div>
				</div>

				{reauthRequired ? (
					<div className="account-callout account-callout-warning">
						<Text size="sm">
							Your managed session expired or became invalid. Re-authenticate to
							restore managed access.
						</Text>
					</div>
				) : null}
			</Stack>
		</Card>
	);
}
