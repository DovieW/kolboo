import {
	ActionIcon,
	Alert,
	Button,
	Group,
	Loader,
	Modal,
	Paper,
	Popover,
	SegmentedControl,
	Stack,
	Switch,
	Text,
	Tooltip,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
	Archive,
	CircleAlert,
	Ellipsis,
	Mic,
	Pause,
	Play,
	Square,
	X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import { recordingControlsAPI } from "../lib/tauri/commands";
import { listenTyped } from "../lib/tauri/events";
import type {
	FileImportResult,
	RecordingPreferences,
} from "../lib/tauri/types";
import { MeetingModelDialog } from "./MeetingModelDialog";

/** Uses the backend pipeline as owner, including recordings started with F3. */
export function RecordingBar() {
	const client = useQueryClient();
	const [computerAudio, setComputerAudio] = useState(false);
	const [optionsOpen, setOptionsOpen] = useState(false);
	const [recoveryOpen, setRecoveryOpen] = useState(false);
	const [discardId, setDiscardId] = useState<string | null>(null);
	const [recoveryResult, setRecoveryResult] = useState<FileImportResult | null>(
		null,
	);
	const [modelOpen, setModelOpen] = useState(false);
	const preferences = useQuery({
		queryKey: ["recording-preferences"],
		queryFn: recordingControlsAPI.getPreferences,
	});
	const recordingPreferences = preferences.data ?? {
		mode: "dictation" as const,
		meeting_model: null,
	};
	const preferencesReady = preferences.isSuccess && !!preferences.data;
	const savePreferences = useMutation({
		mutationFn: recordingControlsAPI.setPreferences,
		onSuccess: (_, value) => {
			client.setQueryData(["recording-preferences"], value);
			setModelOpen(false);
		},
	});
	const updatePreferences = (patch: Partial<RecordingPreferences>) => {
		if (!preferencesReady || savePreferences.isPending) return;
		savePreferences.mutate({ ...recordingPreferences, ...patch });
	};
	const capability = useQuery({
		queryKey: ["computer-audio-capability"],
		queryFn: recordingControlsAPI.computerAudioAvailable,
		staleTime: 60_000,
	});
	const state = useQuery({
		queryKey: ["home-recording-state"],
		queryFn: recordingControlsAPI.getState,
		refetchInterval: 500,
	});
	const action = useMutation({
		mutationFn: async (operation: "start" | "stop" | "cancel") => {
			if (operation === "start") {
				if (!preferencesReady)
					throw new Error("Load recording preferences before starting");
				await recordingControlsAPI.start(
					recordingPreferences.mode === "meeting" && computerAudio,
				);
			} else await recordingControlsAPI[operation]();
		},
		onSettled: async () => {
			await client.invalidateQueries({ queryKey: ["home-recording-state"] });
		},
	});
	const recording = state.data === "recording";
	const progress = useQuery({
		queryKey: ["recording-seconds"],
		queryFn: recordingControlsAPI.getSeconds,
		refetchInterval: 1000,
		retry: false,
	});
	const canPause = useQuery({
		queryKey: ["recording-can-pause"],
		queryFn: recordingControlsAPI.canPause,
		refetchInterval: 500,
	});
	const recovery = useQuery({
		queryKey: ["recording-recovery"],
		queryFn: recordingControlsAPI.listRecovery,
		refetchInterval: 3000,
	});
	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void listenTyped("pipeline-cancelled", () => {
			if (disposed) return;
			// Escape may stop a History-only recording in another window. Ask
			// the backend what was retained; cancellation alone is not proof.
			for (const queryKey of [
				"recording-recovery",
				"home-recording-state",
				"recording-can-pause",
			])
				void client.invalidateQueries({ queryKey: [queryKey] });
		})
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => {
				// Existing polling remains the fallback if event setup is unavailable.
			});
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [client]);
	const recover = useMutation({
		mutationFn: recordingControlsAPI.recover,
		onSuccess: (result) => {
			setRecoveryResult(result);
			if (result.message) {
				setOptionsOpen(false);
				setRecoveryOpen(true);
			}
		},
		onSettled: async () => {
			await Promise.all(
				[
					"recording-recovery",
					"home-recording-state",
					"historyPage",
					"historyAll",
				].map((queryKey) => client.invalidateQueries({ queryKey: [queryKey] })),
			);
		},
	});
	const discard = useMutation({
		mutationFn: recordingControlsAPI.discardRecovery,
		onSuccess: (_, id) => {
			setDiscardId(null);
			if (recoveryResult?.recovery_id === id) setRecoveryResult(null);
		},
		onSettled: () =>
			client.invalidateQueries({ queryKey: ["recording-recovery"] }),
	});
	const paused = useQuery({
		queryKey: ["home-recording-paused"],
		queryFn: recordingControlsAPI.getPaused,
		refetchInterval: 500,
	});
	const pause = useMutation({
		mutationFn: recordingControlsAPI.setPaused,
		onSettled: () =>
			client.invalidateQueries({ queryKey: ["home-recording-paused"] }),
	});
	const cancel = useMutation({
		mutationFn: recordingControlsAPI.cancel,
		onSettled: async () => {
			await Promise.all([
				client.invalidateQueries({ queryKey: ["home-recording-state"] }),
				client.invalidateQueries({ queryKey: ["recording-recovery"] }),
				client.invalidateQueries({ queryKey: ["recording-can-pause"] }),
			]);
		},
	});
	const idle = state.data === "idle" || state.data === "error";
	const error =
		preferences.error ??
		(modelOpen ? null : savePreferences.error) ??
		progress.error ??
		recover.error ??
		discard.error ??
		recovery.error ??
		pause.error ??
		cancel.error ??
		action.error ??
		state.error;

	const errorMessage = error ? formatErrorMessage(error) : null;
	useEffect(() => {
		if (errorMessage) {
			setOptionsOpen(false);
			setRecoveryOpen(true);
		}
	}, [errorMessage]);
	const recoveryMessage = recoveryResult?.message;
	const recoveryIds = [...(recovery.data ?? [])];
	// Cleanup can fail after the PCM was already removed. Retain the returned
	// recovery handle even if the disk listing only contains remaining audio.
	if (
		recoveryResult?.recovery_id &&
		!recoveryIds.includes(recoveryResult.recovery_id)
	)
		recoveryIds.push(recoveryResult.recovery_id);
	const savedCount = recoveryIds.length;
	const pending = action.isPending || recover.isPending || cancel.isPending;
	const busy = !idle && !recording;
	const canSaveForLater =
		recording && canPause.data === true && !canPause.isError;
	const cancelLabel = canSaveForLater ? "Stop & save for later" : "Cancel";
	const optionsLabel = errorMessage
		? "Recording options: error"
		: recoveryMessage
			? "Recording options: recovery needs attention"
			: savedCount
				? `Recording options: ${savedCount} saved ${savedCount === 1 ? "recording" : "recordings"}`
				: "Recording options";

	return (
		<Paper
			withBorder
			shadow="md"
			radius="xl"
			p={8}
			role="region"
			aria-label="Recorder"
			style={{
				position: "fixed",
				bottom: 24,
				right: 24,
				zIndex: 100,
				maxWidth: "calc(100vw - 112px)",
			}}
		>
			<Group gap={6} wrap="nowrap">
				{recording ? (
					<>
						<Text
							size="sm"
							aria-label={
								paused.data ? "Recording paused" : "Recording duration"
							}
							style={{
								minWidth: 52,
								textAlign: "center",
								fontVariantNumeric: "tabular-nums",
							}}
						>
							{Math.floor((progress.data ?? 0) / 60)}:
							{String(Math.floor((progress.data ?? 0) % 60)).padStart(2, "0")}
						</Text>
						{canPause.data ? (
							<Tooltip label={paused.data ? "Resume" : "Pause"}>
								<ActionIcon
									size={34}
									variant="subtle"
									aria-label={paused.data ? "Resume" : "Pause"}
									disabled={pause.isPending || pending}
									onClick={() => pause.mutate(!paused.data)}
								>
									{paused.data ? <Play size={17} /> : <Pause size={17} />}
								</ActionIcon>
							</Tooltip>
						) : null}
						<Tooltip label="Stop & transcribe">
							<ActionIcon
								size={34}
								radius="xl"
								variant="filled"
								aria-label="Stop & transcribe"
								disabled={pending}
								onClick={() => action.mutate("stop")}
							>
								<Square size={15} fill="currentColor" />
							</ActionIcon>
						</Tooltip>
					</>
				) : busy || pending ? (
					<Tooltip label={state.data ? "Processing" : "Connecting"}>
						<span
							role="status"
							aria-label={state.data ? "Processing" : "Connecting"}
						>
							<Loader size={16} mx={9} />
						</span>
					</Tooltip>
				) : (
					<ActionIcon
						size={34}
						variant="filled"
						radius="xl"
						aria-label="Record"
						disabled={
							state.isError ||
							!idle ||
							discard.isPending ||
							!preferencesReady ||
							savePreferences.isPending
						}
						onClick={() => action.mutate("start")}
					>
						<Mic size={17} />
					</ActionIcon>
				)}
				{(!idle && state.data) || pending ? (
					<Tooltip label={cancelLabel}>
						<ActionIcon
							size={34}
							variant="subtle"
							color="gray"
							aria-label={cancelLabel}
							disabled={cancel.isPending}
							onClick={() => cancel.mutate()}
						>
							{canSaveForLater ? <Archive size={17} /> : <X size={17} />}
						</ActionIcon>
					</Tooltip>
				) : null}
				<Popover
					opened={optionsOpen}
					onChange={setOptionsOpen}
					position="top-end"
					withArrow
					shadow="md"
					width={260}
					withinPortal
				>
					<Popover.Target>
						<ActionIcon
							size={34}
							variant="subtle"
							radius="xl"
							color={
								errorMessage
									? "red"
									: savedCount || recoveryMessage
										? "orange"
										: "gray"
							}
							aria-label={optionsLabel}
							onClick={() => setOptionsOpen((open) => !open)}
						>
							{errorMessage ? (
								<CircleAlert size={19} />
							) : (
								<Ellipsis size={19} />
							)}
						</ActionIcon>
					</Popover.Target>
					<Popover.Dropdown
						style={{
							maxWidth: "calc(100vw - 32px)",
						}}
					>
						<Stack gap="sm">
							<SegmentedControl
								aria-label="Recording mode"
								size="xs"
								fullWidth
								data={[
									{ value: "dictation", label: "Dictation" },
									{ value: "meeting", label: "Meeting" },
								]}
								value={recordingPreferences.mode}
								disabled={
									!idle ||
									pending ||
									!preferencesReady ||
									savePreferences.isPending
								}
								onChange={(mode) =>
									updatePreferences({
										mode: mode === "meeting" ? "meeting" : "dictation",
									})
								}
							/>
							{recordingPreferences.mode === "meeting" && (
								<>
									<Button
										size="compact-xs"
										variant="subtle"
										onClick={() => {
											savePreferences.reset();
											setOptionsOpen(false);
											setModelOpen(true);
										}}
										disabled={
											!idle ||
											pending ||
											!preferencesReady ||
											savePreferences.isPending
										}
									>
										Meeting model
									</Button>
									<Switch
										label="Computer audio"
										checked={computerAudio}
										onChange={(event) =>
											setComputerAudio(event.currentTarget.checked)
										}
										disabled={
											!idle || !capability.data || pending || !preferencesReady
										}
										description={
											capability.data ? undefined : "Unavailable on this device"
										}
									/>
								</>
							)}
							{savedCount > 0 && (
								<Button
									variant="subtle"
									size="compact-xs"
									onClick={() => {
										setOptionsOpen(false);
										setRecoveryOpen(true);
									}}
								>
									Saved recordings ({savedCount})
								</Button>
							)}
						</Stack>
					</Popover.Dropdown>
				</Popover>
			</Group>
			{modelOpen && (
				<MeetingModelDialog
					preferences={recordingPreferences}
					onClose={() => {
						if (savePreferences.isPending) return;
						savePreferences.reset();
						setModelOpen(false);
					}}
					onSave={(value) => {
						if (!preferencesReady || savePreferences.isPending) return;
						savePreferences.mutate(value);
					}}
					saving={savePreferences.isPending}
					error={
						savePreferences.error
							? formatErrorMessage(savePreferences.error)
							: null
					}
				/>
			)}
			<Modal
				opened={recoveryOpen}
				onClose={() => {
					setRecoveryOpen(false);
					setDiscardId(null);
				}}
				title="Saved recordings"
				centered
			>
				<Stack gap="md">
					{errorMessage && <Alert color="red">{errorMessage}</Alert>}
					{preferences.isError && (
						<Button
							variant="default"
							loading={preferences.isFetching}
							onClick={() => void preferences.refetch()}
						>
							Retry recording preferences
						</Button>
					)}
					{recoveryMessage && (
						<Alert
							color={recoveryResult?.transcription_complete ? "orange" : "red"}
							title={
								recoveryResult?.transcription_complete
									? "Transcript saved; cleanup needs attention"
									: "Recovery needs attention"
							}
						>
							{recoveryMessage}
						</Alert>
					)}
					<Text size="sm" c="dimmed">
						Transcribe saved audio, finish a cleanup, or discard a recording.
					</Text>
					{recovery.isSuccess && !savedCount && (
						<Text size="sm">No saved recordings found.</Text>
					)}
					{recoveryIds.map((id, index) => (
						<Stack key={id} gap={4}>
							<Text size="xs">Saved recording {index + 1}</Text>
							{discardId === id ? (
								<Stack gap="xs">
									<Text size="sm">
										Remove this recording from recovery? This cannot be undone.
									</Text>
									<Group gap={6}>
										<Button
											size="compact-xs"
											variant="default"
											disabled={discard.isPending}
											onClick={() => setDiscardId(null)}
										>
											Keep recording
										</Button>
										<Button
											size="compact-xs"
											color="red"
											disabled={!idle || pending}
											loading={discard.isPending}
											onClick={() => discard.mutate(id)}
										>
											Discard recording
										</Button>
									</Group>
								</Stack>
							) : (
								<Group gap={6}>
									<Button
										size="compact-xs"
										disabled={!idle || pending || discard.isPending}
										onClick={() => recover.mutate(id)}
									>
										{recoveryResult?.recovery_id === id &&
										recoveryResult.transcription_complete
											? "Finish cleanup"
											: "Transcribe"}
									</Button>
									<Button
										size="compact-xs"
										color="red"
										variant="subtle"
										disabled={!idle || pending || discard.isPending}
										onClick={() => setDiscardId(id)}
									>
										Discard
									</Button>
								</Group>
							)}
						</Stack>
					))}
				</Stack>
			</Modal>
		</Paper>
	);
}
