import { Alert, Button, Select, Tooltip } from "@mantine/core";
import { AlertCircle, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { formatErrorMessage } from "../../lib/formatError";
import {
	createHotkeyShortcutId,
	type HotkeyConfig,
	type HotkeyShortcutCard as HotkeyShortcutCardType,
	type HotkeyType,
} from "../../lib/hotkeys";
import {
	useCreateHotkeyShortcutCard,
	useDeleteHotkeyShortcutCard,
	useResetHotkeysToDefaults,
	useSettings,
	useUpdateHotkeyShortcutCard,
} from "../../lib/queries";
import { HotkeyInput } from "../HotkeyInput";
import { HotkeyShortcutCard } from "./HotkeyShortcutCard";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

type RecordingInput = string | null;

const HOTKEY_TYPE_OPTIONS: Array<{
	value: HotkeyType;
	label: string;
	description: string;
}> = [
	{
		value: "toggle",
		label: "Toggle Recording",
		description: "Press once to start recording, press again to stop",
	},
	{
		value: "hold",
		label: "Hold to Record",
		description: "Hold to record, release to stop",
	},
	{
		value: "paste_last",
		label: "Paste Last Transcription",
		description: "Paste your last result",
	},
	{
		value: "retry",
		label: "Retry Last Recording",
		description: "Re-run the most recent recording and paste the result",
	},
	{
		value: "quick_ask_hold",
		label: "Quick Ask Hold",
		description: "Record a question and show an answer overlay (no auto-paste)",
	},
	{
		value: "quick_ask_toggle",
		label: "Quick Ask Toggle",
		description:
			"Press once to start recording a question, press again to stop (shows answer overlay)",
	},
];

const HOTKEY_TYPE_META = HOTKEY_TYPE_OPTIONS.reduce(
	(acc, option) => {
		acc[option.value] = option;
		return acc;
	},
	{} as Record<HotkeyType, (typeof HOTKEY_TYPE_OPTIONS)[number]>,
);

export function HotkeySettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const isProfileScope = editingProfileId && editingProfileId !== "default";
	const { data: settings, isLoading } = useSettings();
	const createHotkeyShortcutCard = useCreateHotkeyShortcutCard();
	const updateHotkeyShortcutCard = useUpdateHotkeyShortcutCard();
	const deleteHotkeyShortcutCard = useDeleteHotkeyShortcutCard();
	const resetHotkeys = useResetHotkeysToDefaults();

	// Track which input is currently recording (only one at a time)
	const [recordingInput, setRecordingInput] = useState<RecordingInput>(null);
	const [selectedType, setSelectedType] = useState<HotkeyType | null>("toggle");
	const [pendingUpdateCardId, setPendingUpdateCardId] = useState<string | null>(
		null,
	);
	const [pendingDeleteCardId, setPendingDeleteCardId] = useState<string | null>(
		null,
	);

	// Track dismissed error to allow auto-dismiss
	const [dismissedError, setDismissedError] = useState<string | null>(null);

	// Collect any errors from mutations
	const rawError =
		createHotkeyShortcutCard.error ||
		updateHotkeyShortcutCard.error ||
		deleteHotkeyShortcutCard.error ||
		resetHotkeys.error;

	const errorMessage = rawError ? formatErrorMessage(rawError) : null;

	// Only show error if not dismissed
	const showError = errorMessage && errorMessage !== dismissedError;

	// Auto-dismiss error after 5 seconds
	useEffect(() => {
		if (!errorMessage || errorMessage === dismissedError) return;

		const timer = setTimeout(() => {
			setDismissedError(errorMessage);
		}, 5000);

		return () => clearTimeout(timer);
	}, [errorMessage, dismissedError]);

	// Reset dismissed error when a new error appears
	useEffect(() => {
		if (errorMessage && errorMessage !== dismissedError) {
			// New error appeared, clear previous dismissed state
			setDismissedError(null);
		}
	}, [errorMessage, dismissedError]);

	const handleAddShortcutCard = () => {
		if (!selectedType) return;
		const nextCard: HotkeyShortcutCardType = {
			id: createHotkeyShortcutId(),
			type: selectedType,
			hotkey: null,
		};
		createHotkeyShortcutCard.mutate(nextCard);
	};

	const handleHotkeyChange = (cardId: string, config: HotkeyConfig | null) => {
		setPendingUpdateCardId(cardId);
		updateHotkeyShortcutCard.mutate(
			{ cardId, hotkey: config },
			{
				onSettled: () => setPendingUpdateCardId(null),
			},
		);
	};

	const handleDeleteCard = (cardId: string) => {
		setPendingDeleteCardId(cardId);
		deleteHotkeyShortcutCard.mutate(cardId, {
			onSettled: () => setPendingDeleteCardId(null),
		});
	};

	const cards = settings?.hotkey_shortcuts ?? [];
	const visibleCards = pendingDeleteCardId
		? cards.filter((card) => card.id !== pendingDeleteCardId)
		: cards;

	const content = (
		<>
			{showError && (
				<Alert
					icon={<AlertCircle size={16} />}
					color="red"
					mb="md"
					title="Error"
					withCloseButton
					onClose={() => setDismissedError(errorMessage)}
				>
					{errorMessage}
				</Alert>
			)}
			<div className="hotkey-shortcut-controls">
				<Select
					data={HOTKEY_TYPE_OPTIONS.map((option) => ({
						value: option.value,
						label: option.label,
					}))}
					value={selectedType}
					onChange={(value) => setSelectedType(value as HotkeyType | null)}
					placeholder="Choose shortcut type"
					size="sm"
					disabled={isLoading || createHotkeyShortcutCard.isPending}
					className="hotkey-shortcut-controls__select"
				/>
				<Button
					variant="light"
					size="sm"
					onClick={handleAddShortcutCard}
					disabled={
						isLoading || createHotkeyShortcutCard.isPending || !selectedType
					}
					loading={createHotkeyShortcutCard.isPending}
				>
					Add shortcut
				</Button>
			</div>

			{visibleCards.length === 0 ? (
				<div className="empty-state">
					<div className="empty-state-title">No shortcuts yet</div>
					<div className="empty-state-text">
						Add a shortcut card to start configuring hotkeys.
					</div>
				</div>
			) : (
				<div className="hotkey-shortcut-list">
					{visibleCards.map((card) => {
						const meta = HOTKEY_TYPE_META[card.type];
						const isSaving =
							updateHotkeyShortcutCard.isPending &&
							pendingUpdateCardId === card.id;
						const isDeleting =
							deleteHotkeyShortcutCard.isPending &&
							pendingDeleteCardId === card.id;

						return (
							<HotkeyShortcutCard
								key={card.id}
								title={meta?.label ?? "Shortcut"}
								description={meta?.description}
								actions={
									<Button
										variant="subtle"
										size="xs"
										color="red"
										onClick={() => handleDeleteCard(card.id)}
										disabled={isLoading || isSaving || isDeleting}
										loading={isDeleting}
									>
										Delete
									</Button>
								}
							>
								<HotkeyInput
									label="Shortcut"
									value={card.hotkey}
									onChange={(config) => handleHotkeyChange(card.id, config)}
									disabled={isLoading || isSaving || isDeleting}
									isSaving={isSaving}
									isRecording={recordingInput === card.id}
									onStartRecording={() => setRecordingInput(card.id)}
									onStopRecording={() => setRecordingInput(null)}
								/>
							</HotkeyShortcutCard>
						);
					})}
				</div>
			)}

			<div
				style={{
					marginTop: 24,
					display: "flex",
					alignItems: "center",
					justifyContent: "flex-end",
				}}
			>
				<Button
					variant="light"
					color="gray"
					size="xs"
					leftSection={<RotateCcw size={14} />}
					onClick={() => resetHotkeys.mutate()}
					loading={resetHotkeys.isPending}
					disabled={isLoading}
				>
					Reset to Defaults
				</Button>
			</div>
		</>
	);

	if (isProfileScope) {
		return (
			<Tooltip label={GLOBAL_ONLY_TOOLTIP} withArrow position="top-start">
				<div style={{ opacity: 0.5, cursor: "not-allowed" }}>
					<div style={{ pointerEvents: "none" }}>{content}</div>
				</div>
			</Tooltip>
		);
	}

	return content;
}
