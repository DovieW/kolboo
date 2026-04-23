import { Card, Skeleton, Stack, Text, Title } from "@mantine/core";

function IdentityRow({ label, value }: { label: string; value: string }) {
	return (
		<div className="account-detail-row">
			<Text className="account-detail-label">{label}</Text>
			<Text className="account-detail-value">{value}</Text>
		</div>
	);
}

export function AccountIdentityCard(props: {
	loading: boolean;
	email: string | null;
	organizationLabel: string | null;
	organizationId: string | null;
	subject: string | null;
	internalTierLabel: string;
}) {
	const {
		loading,
		email,
		organizationLabel,
		organizationId,
		subject,
		internalTierLabel,
	} = props;

	return (
		<Card withBorder radius="lg" className="account-panel">
			<Stack gap="md">
				<Text className="account-panel-kicker">Identity</Text>
				<Title order={3}>Who this device is connected as</Title>

				{loading ? (
					<Stack gap="sm">
						<Skeleton height={18} width="80%" />
						<Skeleton height={18} width="60%" />
						<Skeleton height={18} width="70%" />
					</Stack>
				) : (
					<Stack gap="sm">
						<IdentityRow label="Email" value={email ?? "Not signed in"} />
						<IdentityRow
							label="Organization"
							value={organizationLabel ?? "No organization connected"}
						/>
						{organizationId ? (
							<IdentityRow label="Organization ID" value={organizationId} />
						) : null}
						{subject ? <IdentityRow label="User ID" value={subject} /> : null}
						<IdentityRow label="Internal tier" value={internalTierLabel} />
					</Stack>
				)}
			</Stack>
		</Card>
	);
}
