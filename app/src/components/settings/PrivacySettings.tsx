import {
	Badge,
	Button,
	Group,
	Stack,
	Switch,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
	useSettings,
	useUpdateRequestLogsPrivacyMode,
} from "../../lib/queries";
import { getPolicyPathEnforcement } from "../../lib/tauri";
import { SettingsRow } from "./SettingsRow";

const AGPL_URL = "https://www.gnu.org/licenses/agpl-3.0.en.html";

type SettingsTabId =
	| "ai"
	| "ui"
	| "audio"
	| "hotkeys"
	| "api-keys"
	| "data"
	| "network";

export function PrivacySettings({
	onNavigateToTab,
}: {
	onNavigateToTab: (tab: SettingsTabId) => void;
}) {
	const { data: settings } = useSettings();
	const updateRequestLogsPrivacyMode = useUpdateRequestLogsPrivacyMode();
	const requestLogsPrivacyMode = settings?.request_logs_privacy_mode ?? false;
	const requestLogsPrivacyPolicy = getPolicyPathEnforcement(
		settings?.policy_state,
		"request_logs_privacy_mode",
	);

	const tryOpenUrl = async (url: string) => {
		try {
			await openUrl(url);
		} catch {
			notifications.show({
				title: "Couldn't open link",
				message:
					"Failed to open your browser. You can copy/paste the link manually.",
				color: "red",
			});
		}
	};

	return (
		<Stack gap="lg">
			<div>
				<Title order={3} mb={6}>
					Privacy & Data
				</Title>
				<Text size="sm" c="dimmed">
					This page explains (in plain English) what Kolboo can store locally,
					and what it might send to third-party providers.
				</Text>
			</div>

			<Stack gap="xs">
				<Text size="sm">
					<strong>Microphone audio:</strong> When you record, the app captures
					mic audio to transcribe it.
				</Text>
				<Text size="sm">
					<strong>Third-party providers:</strong> If you choose an STT/LLM
					provider and configure an API key, your audio/transcripts and prompts
					may be sent to that provider to generate results. Provider pricing,
					retention, and privacy policies apply.
				</Text>
				<Text size="sm">
					<strong>Request logs:</strong> For debugging, the app can keep recent
					request logs <em>in memory</em> (not on disk) and you can export them.
					These logs are redacted as a last line of defense, but you should
					still treat exported logs as sensitive.
				</Text>
			</Stack>

			<SettingsRow
				label="Privacy mode for payloads"
				description="When on, payloads hide full request content (like prompts and context). Turn off to see exact payloads in the modal."
				right={
					<Stack gap={4} align="flex-end">
						{requestLogsPrivacyPolicy.enforced ? (
							<Group gap={6} justify="flex-end" wrap="wrap">
								<Badge variant="light" color="orange">
									Policy enforced
								</Badge>
								{requestLogsPrivacyPolicy.reason ? (
									<Text size="xs" c="dimmed">
										{requestLogsPrivacyPolicy.reason}
									</Text>
								) : null}
							</Group>
						) : null}
						<Switch
							checked={requestLogsPrivacyMode}
							onChange={(event) =>
								updateRequestLogsPrivacyMode.mutate(event.currentTarget.checked)
							}
							disabled={
								!settings ||
								updateRequestLogsPrivacyMode.isPending ||
								requestLogsPrivacyPolicy.enforced
							}
							color="gray"
							size="md"
						/>
					</Stack>
				}
			/>

			<Stack gap="xs">
				<Title order={4}>What gets stored on your computer</Title>
				<Text size="sm" c="dimmed">
					Depending on your settings, Kolboo may store some of these locally:
				</Text>
				<ul style={{ margin: 0, paddingLeft: 18 }}>
					<li>
						<Text size="sm">
							<strong>History</strong> (your past transcriptions/rewrites)
						</Text>
					</li>
					<li>
						<Text size="sm">
							<strong>Recordings</strong> (WAV files, if enabled)
						</Text>
					</li>
					<li>
						<Text size="sm">
							<strong>Stats / cost ledger</strong> (usage/cost events)
						</Text>
					</li>
					<li>
						<Text size="sm">
							<strong>Settings</strong> (preferences; API keys are stored
							securely in your OS credential manager)
						</Text>
					</li>
				</ul>

				<Group gap="sm" wrap="wrap">
					<Button variant="default" onClick={() => onNavigateToTab("data")}>
						Manage stored data
					</Button>
					<Button variant="default" onClick={() => onNavigateToTab("api-keys")}>
						Manage provider keys
					</Button>
					<Button variant="default" onClick={() => onNavigateToTab("network")}>
						Network settings
					</Button>
				</Group>
			</Stack>

			<Stack gap="xs">
				<Title order={3} mb={0}>
					Licensing & third-party disclaimer
				</Title>
				<Text size="sm">
					Kolboo is open source and licensed under the GNU Affero General Public
					License (AGPL).
				</Text>
				<Group gap="sm" wrap="wrap">
					<Button
						variant="subtle"
						onClick={() => {
							void tryOpenUrl(AGPL_URL);
						}}
					>
						Read AGPL license
					</Button>
				</Group>
				<Text size="sm" c="dimmed">
					If you connect third-party AI services, they may charge money and may
					process/store data according to their own policies. Kolboo can’t
					override those provider policies—so only enable providers you trust.
				</Text>
			</Stack>
		</Stack>
	);
}
