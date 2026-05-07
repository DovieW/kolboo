import { Button, Group, Stack, Text, TextInput } from "@mantine/core";
import { Download, Github, Upload } from "lucide-react";
import { SettingsRow } from "../SettingsRow";

type DataBackupSectionProps = {
	exportSettingsBackupPending: boolean;
	importSettingsBackupPending: boolean;
	onExportSettingsBackup: () => void;
	onImportSettingsBackup: () => void;
	githubBackupHasTokenLoading: boolean;
	githubBackupHasToken: boolean;
	onOpenGithubTokenModal: () => void;
	clearGithubTokenPending: boolean;
	onClearGithubToken: () => void;
	gistIdDraft: string;
	onGistIdDraftChange: (value: string) => void;
	saveGistIdPending: boolean;
	onSaveGistId: () => void;
	pushToGistPending: boolean;
	onPushToGist: () => void;
	pullFromGistPending: boolean;
	onPullFromGist: () => void;
};

export function DataBackupSection({
	exportSettingsBackupPending,
	importSettingsBackupPending,
	onExportSettingsBackup,
	onImportSettingsBackup,
	githubBackupHasTokenLoading,
	githubBackupHasToken,
	onOpenGithubTokenModal,
	clearGithubTokenPending,
	onClearGithubToken,
	gistIdDraft,
	onGistIdDraftChange,
	saveGistIdPending,
	onSaveGistId,
	pushToGistPending,
	onPushToGist,
	pullFromGistPending,
	onPullFromGist,
}: DataBackupSectionProps) {
	const githubTokenStatus = githubBackupHasTokenLoading
		? "checking"
		: githubBackupHasToken
			? "configured"
			: "not configured";

	return (
		<>
			<SettingsRow
				label="Settings backup"
				description="Export/import settings as JSON. API keys and other secrets are not included."
				right={
					<Group gap="xs" wrap="nowrap" justify="flex-end">
						<Button
							variant="default"
							size="xs"
							leftSection={<Download size={14} />}
							loading={exportSettingsBackupPending}
							onClick={onExportSettingsBackup}
						>
							Export
						</Button>
						<Button
							variant="default"
							size="xs"
							leftSection={<Upload size={14} />}
							loading={importSettingsBackupPending}
							onClick={onImportSettingsBackup}
						>
							Import
						</Button>
					</Group>
				}
			/>

			<SettingsRow
				label="GitHub Gist backup"
				description={
					<>
						Push/pull your settings to a private GitHub Gist. Requires a GitHub
						token with the <code>gist</code> scope (stored securely).
					</>
				}
				right={
					<Stack gap={8} style={{ width: "min(640px, 100%)" }}>
						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Text size="xs" c="dimmed">
								Token: {githubTokenStatus}
							</Text>

							<Button
								variant="default"
								size="xs"
								leftSection={<Github size={14} />}
								onClick={onOpenGithubTokenModal}
							>
								Set token
							</Button>

							<Button
								variant="default"
								size="xs"
								color="red"
								loading={clearGithubTokenPending}
								disabled={!githubBackupHasToken}
								onClick={onClearGithubToken}
							>
								Clear
							</Button>
						</Group>

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<TextInput
								value={gistIdDraft}
								onChange={(event) => {
									onGistIdDraftChange(event.currentTarget.value);
								}}
								placeholder="Gist id (optional for first push)"
								size="xs"
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										width: 280,
									},
								}}
							/>

							<Button
								variant="default"
								size="xs"
								loading={saveGistIdPending}
								onClick={onSaveGistId}
							>
								Save
							</Button>
						</Group>

						<Group gap="xs" align="center" wrap="nowrap" justify="flex-end">
							<Button
								variant="default"
								size="xs"
								leftSection={<Upload size={14} />}
								loading={pushToGistPending}
								disabled={!githubBackupHasToken}
								onClick={onPushToGist}
							>
								Push
							</Button>

							<Button
								variant="default"
								size="xs"
								leftSection={<Download size={14} />}
								loading={pullFromGistPending}
								disabled={!githubBackupHasToken}
								onClick={onPullFromGist}
							>
								Pull
							</Button>
						</Group>
					</Stack>
				}
			/>
		</>
	);
}
