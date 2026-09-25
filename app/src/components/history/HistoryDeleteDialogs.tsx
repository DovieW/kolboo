import { Button, Group, Modal, Text } from "@mantine/core";
import type { HistoryDeleteOneContext } from "../../lib/history/orchestration";

export function HistoryDeleteDialogs({
	confirmOpened,
	onCloseConfirm,
	onDeleteAll,
	isDeleteAllPending,
	deleteOneOpened,
	onCloseDeleteOne,
	deleteOneContext,
	disableDeleteOneActions,
	deleteOnlyThisTranscriptLoading,
	deleteAllUsingRecordingLoading,
	onDeleteOnlyThisTranscript,
	onDeleteAllUsingRecording,
}: {
	confirmOpened: boolean;
	onCloseConfirm: () => void;
	onDeleteAll: () => void;
	isDeleteAllPending: boolean;
	deleteOneOpened: boolean;
	onCloseDeleteOne: () => void;
	deleteOneContext: HistoryDeleteOneContext | null;
	disableDeleteOneActions: boolean;
	deleteOnlyThisTranscriptLoading: boolean;
	deleteAllUsingRecordingLoading: boolean;
	onDeleteOnlyThisTranscript: () => void;
	onDeleteAllUsingRecording: () => void;
}) {
	return (
		<>
			<Modal
				opened={confirmOpened}
				onClose={onCloseConfirm}
				closeOnClickOutside={!isDeleteAllPending}
				closeOnEscape={!isDeleteAllPending}
				withCloseButton={!isDeleteAllPending}
				title="Delete transcripts and recordings"
				centered
				size="sm"
			>
				<Text size="sm" mb="lg">
					Delete all transcripts and recordings? This cannot be undone.
				</Text>
				<Group justify="flex-end">
					<Button
						variant="default"
						onClick={onCloseConfirm}
						disabled={isDeleteAllPending}
					>
						Cancel
					</Button>
					<Button
						color="red"
						onClick={onDeleteAll}
						loading={isDeleteAllPending}
					>
						Delete all
					</Button>
				</Group>
			</Modal>

			<Modal
				opened={deleteOneOpened}
				onClose={onCloseDeleteOne}
				title="Delete"
				centered
				size="sm"
			>
				{deleteOneContext ? (
					<>
						<Text size="sm" mb="sm">
							This transcript shares its recording with{" "}
							{Math.max(0, deleteOneContext.refCount - 1)} other history item
							{Math.max(0, deleteOneContext.refCount - 1) === 1 ? "" : "s"}.
						</Text>

						<Group justify="flex-end" gap="sm" wrap="wrap">
							<Button
								variant="subtle"
								color="gray"
								onClick={onDeleteOnlyThisTranscript}
								loading={deleteOnlyThisTranscriptLoading}
								disabled={disableDeleteOneActions}
							>
								Delete only this transcript
							</Button>

							<Button
								color="red"
								onClick={onDeleteAllUsingRecording}
								loading={deleteAllUsingRecordingLoading}
								disabled={disableDeleteOneActions}
							>
								Delete all using this recording
							</Button>
						</Group>
					</>
				) : null}
			</Modal>
		</>
	);
}
