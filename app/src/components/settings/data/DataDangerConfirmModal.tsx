import { Button, Group, Modal, Text, TextInput } from "@mantine/core";

export type DataDangerDialogState = {
	title: string;
	message: string;
	confirmLabel: string;
	typedConfirm?: {
		requiredText: string;
		label?: string;
		placeholder?: string;
	};
	action: () => Promise<void>;
};

type DataDangerConfirmModalProps = {
	dialog: DataDangerDialogState | null;
	running: boolean;
	typedDraft: string;
	onTypedDraftChange: (value: string) => void;
	onClose: () => void;
	onConfirm: () => void;
};

export function DataDangerConfirmModal({
	dialog,
	running,
	typedDraft,
	onTypedDraftChange,
	onClose,
	onConfirm,
}: DataDangerConfirmModalProps) {
	const typedConfirm = dialog?.typedConfirm;
	const confirmDisabled =
		running ||
		(typedConfirm ? typedDraft.trim() !== typedConfirm.requiredText : false);

	return (
		<Modal
			opened={dialog !== null}
			onClose={onClose}
			title={dialog?.title ?? ""}
			centered
			size="sm"
		>
			<Text size="sm" mb="md">
				{dialog?.message ?? ""}
			</Text>

			{typedConfirm ? (
				<TextInput
					label={
						typedConfirm.label ?? `Type ${typedConfirm.requiredText} to confirm`
					}
					placeholder={typedConfirm.placeholder ?? typedConfirm.requiredText}
					value={typedDraft}
					onChange={(event) => {
						onTypedDraftChange(event.currentTarget.value);
					}}
					mb="md"
				/>
			) : null}

			<Text size="xs" c="dimmed" mb="md">
				Tip: if you only want to free up disk space, delete recordings — it's
				the least destructive option.
			</Text>

			<Group justify="flex-end" gap="sm">
				<Button variant="default" disabled={running} onClick={onClose}>
					Cancel
				</Button>
				<Button
					color="red"
					loading={running}
					disabled={confirmDisabled}
					onClick={onConfirm}
				>
					{dialog?.confirmLabel ?? "Confirm"}
				</Button>
			</Group>
		</Modal>
	);
}
