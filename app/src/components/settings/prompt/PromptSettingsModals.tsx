import { Button, Group, Modal, Select, Text } from "@mantine/core";
import type { RewritePreset } from "../../../lib/tauri";

export interface LinkableProfileOption {
	id: string;
	label: string;
	presets: RewritePreset[];
}

interface PromptSettingsModalsProps {
	linkPresetModalOpen: boolean;
	onCloseLinkPresetModal: () => void;
	linkableProfiles: LinkableProfileOption[];
	linkSourceProfileId: string | null;
	onLinkSourceProfileChange: (value: string) => void;
	linkSourcePresetId: string | null;
	onLinkSourcePresetChange: (value: string) => void;
	linkSourceProfile: LinkableProfileOption | null;
	canConfirmLinkPreset: boolean;
	onConfirmLinkPreset: () => void;
	deletePresetDialog: null | {
		presetId: string;
		presetName: string;
		isShared: boolean;
	};
	onCloseDeletePresetDialog: () => void;
	onConfirmDeletePreset: () => void;
	resetDialog: null | {
		title: string;
		onConfirm: () => void;
	};
	onCloseResetDialog: () => void;
	onConfirmResetDialog: () => void;
}

export function PromptSettingsModals({
	linkPresetModalOpen,
	onCloseLinkPresetModal,
	linkableProfiles,
	linkSourceProfileId,
	onLinkSourceProfileChange,
	linkSourcePresetId,
	onLinkSourcePresetChange,
	linkSourceProfile,
	canConfirmLinkPreset,
	onConfirmLinkPreset,
	deletePresetDialog,
	onCloseDeletePresetDialog,
	onConfirmDeletePreset,
	resetDialog,
	onCloseResetDialog,
	onConfirmResetDialog,
}: PromptSettingsModalsProps) {
	return (
		<>
			<Modal
				opened={linkPresetModalOpen}
				onClose={onCloseLinkPresetModal}
				title="Add preset from another profile"
				centered
				zIndex={1200}
			>
				<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
					<Select
						label="Source profile"
						data={linkableProfiles.map((p) => ({
							value: p.id,
							label: p.label,
						}))}
						value={linkSourceProfileId}
						onChange={(value) => {
							if (!value) return;
							onLinkSourceProfileChange(value);
						}}
						placeholder={
							linkableProfiles.length === 0
								? "No other profiles"
								: "Select profile"
						}
						withCheckIcon={false}
					/>

					<Select
						label="Preset"
						data={(linkSourceProfile?.presets ?? []).map((p) => ({
							value: p.id,
							label: p.name?.trim() || p.id,
						}))}
						value={linkSourcePresetId}
						onChange={(value) => {
							if (!value) return;
							onLinkSourcePresetChange(value);
						}}
						disabled={!linkSourceProfile}
						placeholder={
							!linkSourceProfile ? "Select a profile first" : "Select preset"
						}
						withCheckIcon={false}
					/>
				</div>

				<Group justify="flex-end" mt="md" gap="sm">
					<Button variant="default" onClick={onCloseLinkPresetModal}>
						Cancel
					</Button>
					<Button
						color="gray"
						onClick={onConfirmLinkPreset}
						disabled={!canConfirmLinkPreset}
					>
						Add Preset
					</Button>
				</Group>
			</Modal>

			<Modal
				opened={deletePresetDialog !== null}
				onClose={onCloseDeletePresetDialog}
				title="Delete preset?"
				centered
				zIndex={1300}
			>
				<Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
					{deletePresetDialog?.isShared
						? "This preset is shared. Deleting it here only removes it from this profile; other profiles will keep it."
						: "This will remove the preset from this profile."}
				</Text>

				<Text size="sm" mt="xs" style={{ lineHeight: 1.4 }}>
					{deletePresetDialog?.presetName ?? ""}
				</Text>

				<Group justify="flex-end" mt="md" gap="sm">
					<Button variant="default" onClick={onCloseDeletePresetDialog}>
						Cancel
					</Button>
					<Button color="red" onClick={onConfirmDeletePreset}>
						Delete
					</Button>
				</Group>
			</Modal>

			<Modal
				opened={resetDialog !== null}
				onClose={onCloseResetDialog}
				title={resetDialog?.title ?? ""}
				centered
			>
				<Text size="sm" c="dimmed" style={{ lineHeight: 1.4 }}>
					This setting is currently overriding the Default profile. Disable the
					override to inherit from Default.
				</Text>
				<Group justify="flex-end" mt="md" gap="sm">
					<Button variant="default" onClick={onCloseResetDialog}>
						Keep override
					</Button>
					<Button color="gray" onClick={onConfirmResetDialog}>
						Disable override
					</Button>
				</Group>
			</Modal>
		</>
	);
}
