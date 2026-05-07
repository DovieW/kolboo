import { Button, Group, Modal, PasswordInput, Text } from "@mantine/core";

type DataGithubTokenModalProps = {
	opened: boolean;
	saving: boolean;
	value: string;
	onChange: (value: string) => void;
	onClose: () => void;
	onSave: () => void;
	onOpenTokenCreationPage: () => void;
	onOpenDocs: () => void;
};

export function DataGithubTokenModal({
	opened,
	saving,
	value,
	onChange,
	onClose,
	onSave,
	onOpenTokenCreationPage,
	onOpenDocs,
}: DataGithubTokenModalProps) {
	return (
		<Modal
			opened={opened}
			onClose={onClose}
			title="GitHub token"
			centered
			size="sm"
		>
			<Text size="sm" mb="md">
				Create a GitHub personal access token with the <code>gist</code> scope.
				It will be stored securely in your OS credential manager.
			</Text>

			<Group gap="xs" mb="md" wrap="wrap">
				<Button variant="subtle" size="xs" onClick={onOpenTokenCreationPage}>
					Open token creation page
				</Button>

				<Button variant="subtle" size="xs" onClick={onOpenDocs}>
					Docs
				</Button>
			</Group>

			<PasswordInput
				label="Token"
				value={value}
				onChange={(event) => {
					onChange(event.currentTarget.value);
				}}
			/>

			<Group justify="flex-end" gap="sm" mt="md">
				<Button variant="default" disabled={saving} onClick={onClose}>
					Cancel
				</Button>
				<Button loading={saving} onClick={onSave}>
					Save token
				</Button>
			</Group>
		</Modal>
	);
}
