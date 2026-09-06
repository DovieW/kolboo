import {
	Alert,
	Button,
	Group,
	Paper,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import { recordingControlsAPI } from "../lib/tauri/commands";

/** Uses the backend pipeline as owner, including recordings started with F3. */
export function RecordingBar() {
	const client = useQueryClient();
	const [computerAudio, setComputerAudio] = useState(false);
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
			if (operation === "start")
				await recordingControlsAPI.start(computerAudio);
			else await recordingControlsAPI[operation]();
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
	const recover = useMutation({
		mutationFn: recordingControlsAPI.recover,
		onSettled: () =>
			client.invalidateQueries({ queryKey: ["recording-recovery"] }),
	});
	const discard = useMutation({
		mutationFn: recordingControlsAPI.discardRecovery,
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
		onSettled: () =>
			client.invalidateQueries({ queryKey: ["home-recording-state"] }),
	});
	const idle = state.data === "idle" || state.data === "error";
	const error =
		progress.error ??
		recover.error ??
		discard.error ??
		recovery.error ??
		pause.error ??
		cancel.error ??
		action.error ??
		state.error;

	return (
		<Paper
			withBorder
			shadow="md"
			radius="lg"
			p="md"
			style={{
				position: "fixed",
				bottom: 24,
				right: 24,
				zIndex: 100,
				width: 380,
				maxWidth: "calc(100vw - 112px)",
				maxHeight: "60vh",
				overflowY: "auto",
			}}
		>
			<Stack gap="sm">
				<Text fw={600}>
					{recording
						? "Recording audio"
						: idle
							? "Record a transcription"
							: "Recording pipeline busy"}
				</Text>
				{recording ? (
					<Text size="sm" aria-live="off">
						{paused.data ? "Paused · " : ""}
						{Math.floor((progress.data ?? 0) / 60)}:
						{String(Math.floor((progress.data ?? 0) % 60)).padStart(2, "0")}
					</Text>
				) : null}
				<Switch
					label="Computer audio"
					checked={computerAudio}
					onChange={(event) => setComputerAudio(event.currentTarget.checked)}
					disabled={!idle || !capability.data || action.isPending}
					description={
						capability.data
							? "Include system output and the default microphone"
							: "Computer audio is unavailable on this installation"
					}
				/>
				<Text size="xs" c="dimmed">
					Audio is saved locally for recovery. Stop sends it to your selected
					transcription provider in 30-second sections.
				</Text>
				{error ? (
					<Alert color="red" role="alert">
						{formatErrorMessage(error)}
					</Alert>
				) : null}
				<Group justify="flex-end">
					{recording && canPause.data ? (
						<Button
							variant="default"
							disabled={pause.isPending || action.isPending}
							onClick={() => pause.mutate(!paused.data)}
						>
							{paused.data ? "Resume" : "Pause"}
						</Button>
					) : null}
					{!idle && state.data ? (
						<Button
							variant="subtle"
							disabled={cancel.isPending}
							onClick={() => cancel.mutate()}
						>
							Cancel
						</Button>
					) : null}
					<Button
						disabled={
							state.isError || (!idle && !recording) || action.isPending
						}
						loading={action.isPending}
						onClick={() => action.mutate(recording ? "stop" : "start")}
					>
						{recording ? "Stop & transcribe" : "Record"}
					</Button>
				</Group>
				{recovery.data?.map((id, index) => (
					<Group key={id}>
						<Text size="xs">Saved audio {index + 1}</Text>
						<Button
							size="xs"
							disabled={!idle || recover.isPending || discard.isPending}
							onClick={() => recover.mutate(id)}
						>
							Recover & transcribe
						</Button>
						<Button
							size="xs"
							color="red"
							variant="subtle"
							disabled={!idle || recover.isPending || discard.isPending}
							onClick={() => discard.mutate(id)}
						>
							Discard audio
						</Button>
					</Group>
				))}
			</Stack>
		</Paper>
	);
}
