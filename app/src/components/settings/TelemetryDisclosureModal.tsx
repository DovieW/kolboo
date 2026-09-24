import { Accordion, Button, Group, Modal, Stack, Text } from "@mantine/core";

type TelemetryDisclosureModalProps = {
	opened: boolean;
	analyticsPolicyEnforced: boolean;
	analyticsPolicyReason: string | null;
	loading: boolean;
	onDisableAnalytics: () => void;
	onAllowAnalytics: () => void;
};

type TelemetryDisclosureContentProps = Omit<
	TelemetryDisclosureModalProps,
	"opened"
>;

export function TelemetryDisclosureContent({
	analyticsPolicyEnforced,
	analyticsPolicyReason,
	loading,
	onDisableAnalytics,
	onAllowAnalytics,
}: TelemetryDisclosureContentProps) {
	return (
		<Stack gap="lg">
			<Text size="sm" c="dimmed">
				{analyticsPolicyEnforced
					? "Your organization has turned off product analytics. No events will be sent."
					: "Kolboo counts basic usage without a persistent ID. You can also allow linked analytics to help us understand repeat use. No audio or transcript content is sent."}
			</Text>

			<Accordion variant="default">
				<Accordion.Item value="details">
					<Accordion.Control>What’s shared?</Accordion.Control>
					<Accordion.Panel>
						<Stack gap="xs">
							<Text size="sm" c="dimmed">
								App opens, page visits and recording outcomes. Recording events
								include rounded duration, mode and recognized provider/model.
							</Text>
							<Text size="sm" c="dimmed">
								No transcripts, prompts, audio, OCR content, secrets or email
								addresses. Session replay and autocapture are off.
							</Text>
							<Text size="sm" c="dimmed">
								Basic usage uses a fresh ID for each event. Linked analytics
								uses your account ID when signed in or a random installation ID
								when signed out. The analytics service can see the network
								request’s IP address.
							</Text>
							<Text size="sm" c="dimmed">
								{analyticsPolicyEnforced
									? "Your organization controls this setting."
									: "Change linked analytics later in Settings → Data."}
							</Text>
							{analyticsPolicyReason ? (
								<Text size="sm" c="dimmed">
									Policy reason: {analyticsPolicyReason}
								</Text>
							) : null}
						</Stack>
					</Accordion.Panel>
				</Accordion.Item>
			</Accordion>

			<Group justify="flex-end" gap="sm">
				{analyticsPolicyEnforced ? (
					<Button disabled={loading} onClick={onDisableAnalytics}>
						Continue
					</Button>
				) : (
					<>
						<Button
							variant="default"
							disabled={loading}
							onClick={onDisableAnalytics}
						>
							Basic usage only
						</Button>
						<Button disabled={loading} onClick={onAllowAnalytics}>
							Allow linked analytics
						</Button>
					</>
				)}
			</Group>
		</Stack>
	);
}

export function TelemetryDisclosureModal({
	opened,
	...contentProps
}: TelemetryDisclosureModalProps) {
	return (
		<Modal
			opened={opened}
			onClose={() => {
				// Intentionally no-op: the launch posture requires an explicit choice
				// before product analytics may begin sending any events.
			}}
			withCloseButton={false}
			closeOnClickOutside={false}
			closeOnEscape={false}
			centered
			title="Product analytics"
			size="md"
		>
			<TelemetryDisclosureContent {...contentProps} />
		</Modal>
	);
}
