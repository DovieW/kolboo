import {
	Accordion,
	Badge,
	Button,
	Card,
	Group,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import {
	formatEnterprisePersonaLabel,
	useEnterprisePersonaState,
} from "../../lib/queries/enterprisePersona";
import type { LicenseAuthContext } from "../../lib/tauri";

function AdvancedDetail({ label, value }: { label: string; value: string }) {
	return (
		<div className="account-detail-row">
			<Text className="account-detail-label">{label}</Text>
			<Text className="account-detail-value">{value}</Text>
		</div>
	);
}

export function AccountAdvancedPanel(props: {
	authContext: LicenseAuthContext | null | undefined;
	authContextMessage: string;
	signedIn: boolean;
	refreshPending: boolean;
	onSimulateAuthFailure: () => void;
}) {
	const {
		authContext,
		authContextMessage,
		signedIn,
		refreshPending,
		onSimulateAuthFailure,
	} = props;
	const personaState = useEnterprisePersonaState();
	const persona = personaState.data;
	const showPersona = Boolean(
		persona?.context_key ||
			persona?.persona_type ||
			persona?.test_access_active,
	);

	return (
		<Accordion
			radius="lg"
			variant="contained"
			className="account-advanced-accordion"
		>
			<Accordion.Item value="advanced">
				<Accordion.Control>
					<Stack gap={2}>
						<Text fw={600}>Advanced account details</Text>
						<Text size="sm" c="dimmed">
							Diagnostics, test persona info, and support-friendly auth context.
						</Text>
					</Stack>
				</Accordion.Control>
				<Accordion.Panel>
					<div className="account-advanced-grid">
						<Card withBorder radius="lg" className="account-panel">
							<Stack gap="md">
								<Group justify="space-between" align="center" wrap="wrap">
									<div>
										<Text className="account-panel-kicker">Auth context</Text>
										<Title order={4}>Support details</Title>
									</div>
									<Badge color={authContext?.authenticated ? "green" : "gray"}>
										{authContext?.authenticated
											? "Authenticated"
											: "Not authenticated"}
									</Badge>
								</Group>

								<Stack gap="sm">
									<AdvancedDetail label="Reason" value={authContextMessage} />
									<AdvancedDetail
										label="Policy status"
										value={authContext?.policy_status ?? "unknown"}
									/>
									<AdvancedDetail
										label="Secure session"
										value={
											authContext?.secure_session_present
												? "Present"
												: "Missing"
										}
									/>
									{authContext?.org_id ? (
										<AdvancedDetail
											label="Organization ID"
											value={authContext.org_id}
										/>
									) : null}
									{authContext?.subject_id ? (
										<AdvancedDetail
											label="User ID"
											value={authContext.subject_id}
										/>
									) : null}
								</Stack>
							</Stack>
						</Card>

						{showPersona ? (
							<Card withBorder radius="lg" className="account-panel">
								<Stack gap="md">
									<Text className="account-panel-kicker">Test persona</Text>
									<Title order={4}>Non-production context</Title>
									<Stack gap="sm">
										<AdvancedDetail
											label="Environment"
											value={persona?.environment?.toUpperCase() ?? "UNKNOWN"}
										/>
										<AdvancedDetail
											label="Persona"
											value={formatEnterprisePersonaLabel(
												persona?.persona_type ?? null,
											)}
										/>
										{persona?.context_key ? (
											<AdvancedDetail
												label="Context key"
												value={persona.context_key}
											/>
										) : null}
										<AdvancedDetail
											label="Test access"
											value={
												persona?.test_access_active ? "Enabled" : "Disabled"
											}
										/>
									</Stack>
								</Stack>
							</Card>
						) : null}

						<Card withBorder radius="lg" className="account-panel">
							<Stack gap="md">
								<Text className="account-panel-kicker">Testing tools</Text>
								<Title order={4}>Manual verification</Title>
								<Text c="dimmed" size="sm">
									Trigger the auth failure path so you can verify the re-auth
									UX.
								</Text>
								<Button
									variant="default"
									onClick={onSimulateAuthFailure}
									loading={refreshPending}
									disabled={!signedIn}
								>
									Simulate auth failure
								</Button>
								{!signedIn ? (
									<Text size="xs" c="dimmed">
										Sign in first to test the refresh failure path.
									</Text>
								) : null}
							</Stack>
						</Card>
					</div>
				</Accordion.Panel>
			</Accordion.Item>
		</Accordion>
	);
}
