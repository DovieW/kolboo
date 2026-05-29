import { Button, Group, Modal, Stack, Text } from "@mantine/core";

type TelemetryDisclosureModalProps = {
	opened: boolean;
	analyticsEnabled: boolean;
	analyticsPolicyEnforced: boolean;
	analyticsPolicyReason: string | null;
	loading: boolean;
	onDisableAnalytics: () => void;
	onContinue: () => void;
};

export function TelemetryDisclosureModal({
	opened,
	analyticsEnabled,
	analyticsPolicyEnforced,
	analyticsPolicyReason,
	loading,
	onDisableAnalytics,
	onContinue,
}: TelemetryDisclosureModalProps) {
	const disableLabel = analyticsEnabled
		? "Disable analytics"
		: "Keep analytics disabled";

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
			title="Review product analytics"
			size="lg"
		>
			<Stack gap="md">
				<Text size="sm">
					{analyticsPolicyEnforced
						? "Your organization has disabled product analytics for this installation. Reviewing this notice acknowledges the privacy posture, but analytics will remain off until that policy changes."
						: "Kolboo can send privacy-safe product analytics to help improve feature quality and reliability. Nothing is sent until you make a choice here."}
				</Text>

				<div>
					<Text size="sm" fw={600} mb={6}>
						This analytics flow is intentionally limited:
					</Text>
					<ul style={{ margin: 0, paddingLeft: 18 }}>
						<li>
							<Text size="sm">
								Event-only analytics only; no transcripts, prompts, audio, or
								OCR content are included.
							</Text>
						</li>
						<li>
							<Text size="sm">
								Session replay and desktop autocapture are off.
							</Text>
						</li>
						<li>
							<Text size="sm">
								A local distinct ID is used for counting installs and product
								behavior, not for capturing raw content.
							</Text>
						</li>
					</ul>
				</div>

				<Text size="sm" c="dimmed">
					{analyticsPolicyEnforced
						? "Settings → Data will show that this control is currently managed by organization policy."
						: "You can change this later in Settings → Data."}
				</Text>

				{analyticsPolicyReason ? (
					<Text size="sm" c="dimmed">
						Policy reason: {analyticsPolicyReason}
					</Text>
				) : null}

				<Group justify="space-between" gap="sm" wrap="wrap-reverse">
					<Button
						variant="default"
						disabled={loading}
						onClick={onDisableAnalytics}
					>
						{disableLabel}
					</Button>
					<Button disabled={loading} onClick={onContinue}>
						Continue
					</Button>
				</Group>
			</Stack>
		</Modal>
	);
}
