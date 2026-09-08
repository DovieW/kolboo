import {
	ActionIcon,
	Alert,
	Badge,
	Button,
	Group,
	Loader,
	Paper,
	SegmentedControl,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
	Check,
	FileAudio,
	FolderOpen,
	History,
	ShieldCheck,
	Upload,
	X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import {
	audioFileName,
	audioFilesAPI,
	isSupportedAudioFile,
} from "../lib/tauri/audioFiles";
import { recordingControlsAPI } from "../lib/tauri/commands";
import type { RecordingPreferences } from "../lib/tauri/types";
import styles from "./FileTranscription.module.css";
import { MeetingModelDialog } from "./MeetingModelDialog";

/** Kept mounted by App so a draft/job survives navigation, never persisted as a path. */
export function FileTranscription({
	active = true,
	onOpenHistory,
}: {
	active?: boolean;
	onOpenHistory?: () => void;
}) {
	const client = useQueryClient();
	const [path, setPath] = useState<string | null>(null);
	const [hovered, setHovered] = useState(false);
	const [message, setMessage] = useState<string | null>(null);
	const [complete, setComplete] = useState(false);
	const [recoveryId, setRecoveryId] = useState<string | null>(null);
	const [modelOpen, setModelOpen] = useState(false);
	const [draftPreferences, setPreferences] =
		useState<RecordingPreferences | null>(null);
	const initial = useQuery({
		queryKey: ["recording-preferences"],
		queryFn: recordingControlsAPI.getPreferences,
		enabled: active,
	});
	// Follow recorder defaults while pristine, then keep this file's explicit
	// options independent of changes made elsewhere (including while hidden).
	const preferences = draftPreferences ?? initial.data ?? null;
	useEffect(() => {
		if (path && initial.data)
			setPreferences((existing) => existing ?? initial.data);
	}, [path, initial.data]);
	const state = useQuery({
		queryKey: ["home-recording-state"],
		queryFn: recordingControlsAPI.getState,
		enabled: active,
		refetchInterval: 1000,
	});
	const job = useMutation({
		mutationFn: async () => {
			if (recoveryId) return recordingControlsAPI.recover(recoveryId);
			if (!path || !preferences)
				throw new Error("Choose audio and transcription options first");
			return recordingControlsAPI.importFile(path, preferences);
		},
		onSuccess: (result) => {
			setRecoveryId(result.recovery_id);
			setMessage(result.message);
			setComplete(result.transcription_complete);
		},
		onError: (error) => setMessage(formatErrorMessage(error)),
		onSettled: () => {
			for (const key of [
				"historyPage",
				"historyAll",
				"requestLogs",
				"recording-recovery",
				"home-recording-state",
			])
				void client.invalidateQueries({ queryKey: [key] });
		},
	});
	const cancel = useMutation({
		mutationFn: recordingControlsAPI.cancel,
		onError: (error) => setMessage(formatErrorMessage(error)),
	});
	const choosing = useRef(false);
	const pending = job.isPending;
	const busy = state.data !== "idle" && state.data !== "error";
	const disabled = pending || cancel.isPending || busy || state.isError;
	const select = (paths: string[]) => {
		if (disabled) return;
		const selected = paths[0];
		if (paths.length !== 1 || !selected || !isSupportedAudioFile(selected)) {
			setMessage("Choose one supported audio file at a time.");
			return;
		}
		setPath(selected);
		setPreferences(preferences);
		setComplete(false);
		setMessage(null);
		setRecoveryId(null);
	};
	const selectRef = useRef(select);
	selectRef.current = select;
	useEffect(() => {
		if (!active) return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void audioFilesAPI
			.listenForDrop(
				(paths) => {
					if (!disposed) selectRef.current(paths);
				},
				(value) => {
					if (!disposed) setHovered(value);
				},
			)
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => {
				// File selection still works when native drag-and-drop isn't available.
			});
		return () => {
			disposed = true;
			unlisten?.();
			setHovered(false);
		};
	}, [active]);
	const choose = async () => {
		if (disabled || choosing.current) return;
		choosing.current = true;
		try {
			const selected = await audioFilesAPI.choose();
			if (selected) selectRef.current([selected]);
		} catch {
			setMessage(
				"Could not open the file picker. Try dragging an audio file here.",
			);
		} finally {
			choosing.current = false;
		}
	};
	const canTranscribe =
		!!path &&
		!!preferences &&
		!disabled &&
		!complete &&
		(preferences.mode === "dictation" ||
			!!preferences.meeting_model ||
			!!recoveryId);
	return (
		<div className={`main-content ${styles.page}`}>
			<header className="tv-page-header">
				<Title order={1}>Transcribe file</Title>
				<Text c="dimmed">
					Turn saved audio into a searchable, editable transcript.
				</Text>
			</header>
			<div className={`main-content-inner ${styles.workspace}`}>
				<Paper
					withBorder
					radius="lg"
					p="xl"
					className={hovered && !disabled ? styles.dropActive : styles.drop}
				>
					<Stack align="center" gap="md">
						<div className={styles.fileIcon}>
							<FileAudio size={30} aria-hidden="true" />
						</div>
						{path ? (
							<>
								<Group
									justify="center"
									wrap="nowrap"
									className={styles.fileName}
								>
									<Text fw={600} lineClamp={2}>
										{audioFileName(path)}
									</Text>
									<ActionIcon
										variant="subtle"
										color="gray"
										aria-label="Remove selected file"
										disabled={disabled}
										onClick={() => {
											setPath(null);
											setMessage(null);
											setRecoveryId(null);
											setComplete(false);
										}}
									>
										<X size={17} />
									</ActionIcon>
								</Group>
								<Badge variant="light" color={complete ? "green" : "gray"}>
									{complete ? "Transcribed" : "Selected locally"}
								</Badge>
							</>
						) : (
							<>
								<Title order={2} size="h3">
									Drop an audio file here
								</Title>
								<Text c="dimmed" size="sm" ta="center">
									WAV, MP3, FLAC or AAC
									<br />
									Up to 4 hours · 2 GiB source file
								</Text>
							</>
						)}
						<Button
							variant="default"
							leftSection={<FolderOpen size={16} />}
							disabled={disabled}
							onClick={() => void choose()}
						>
							{path ? "Choose another file" : "Choose file"}
						</Button>
					</Stack>
				</Paper>
				<Paper withBorder radius="lg" p="lg">
					<Stack gap="lg">
						<Group justify="space-between">
							<Text fw={600}>Transcription</Text>
							<SegmentedControl
								aria-label="File transcription mode"
								value={preferences?.mode ?? "dictation"}
								disabled={!preferences || disabled || !!recoveryId || complete}
								data={[
									{ value: "dictation", label: "Dictation" },
									{ value: "meeting", label: "Meeting" },
								]}
								onChange={(mode) => {
									if (preferences)
										setPreferences({
											...preferences,
											mode: mode === "meeting" ? "meeting" : "dictation",
										});
								}}
							/>
						</Group>
						{!preferences ? null : preferences.mode === "meeting" ? (
							<Stack gap="xs">
								<Group justify="space-between" wrap="nowrap">
									<div className={styles.model}>
										<Text size="sm" fw={500}>
											{preferences.meeting_model?.model ??
												"Choose a meeting model"}
										</Text>
										{preferences.meeting_model && (
											<Text size="xs" c="dimmed">
												{preferences.meeting_model.provider} ·{" "}
												{preferences.meeting_model.use_managed
													? "Managed"
													: preferences.meeting_model.provider ===
															"local-whisper"
														? "On this device"
														: "Your key"}
											</Text>
										)}
									</div>
									<Button
										variant="default"
										size="xs"
										disabled={disabled || !!recoveryId || complete}
										onClick={() => setModelOpen(true)}
									>
										{preferences.meeting_model
											? "Change model"
											: "Choose model"}
									</Button>
								</Group>
								<Text size="sm" c="dimmed">
									No rewriting. Choose a speaker-label model if you need
									diarization.
								</Text>
							</Stack>
						) : (
							<Text size="sm" c="dimmed">
								Uses your dictation model and optional rewriting settings. The
								result stays in History; nothing is pasted into another app.
							</Text>
						)}
						{initial.isError && !preferences && (
							<Alert color="red">
								Recording preferences could not be loaded.{" "}
								<Button
									size="compact-xs"
									variant="subtle"
									onClick={() => void initial.refetch()}
								>
									Retry
								</Button>
							</Alert>
						)}
						{state.isError && (
							<Alert color="red" title="Recording controls unavailable">
								Couldn’t check whether the recorder is busy. Reconnect before
								starting another transcription.
								<Button
									size="compact-xs"
									variant="subtle"
									onClick={() => void state.refetch()}
								>
									Reconnect
								</Button>
							</Alert>
						)}
						{(state.isPending || (!preferences && initial.isPending)) && (
							<Group gap="xs" role="status">
								<Loader size="xs" />
								<Text size="sm" c="dimmed">
									{state.isPending
										? "Checking recorder…"
										: "Loading transcription options…"}
								</Text>
							</Group>
						)}
						<Group align="flex-start" gap="xs" wrap="nowrap">
							<ShieldCheck
								size={17}
								className={styles.noteIcon}
								aria-hidden="true"
							/>
							<Text size="xs" c="dimmed">
								No audio is sent until you choose Transcribe. Local
								transcription keeps audio on this device; cloud models send it
								to your selected provider. Optional rewriting follows your
								existing settings. Your original file is unchanged.
							</Text>
						</Group>
						{message && (
							<Alert
								color={complete ? "orange" : "red"}
								title={
									complete
										? "Transcript saved; cleanup needs attention"
										: recoveryId
											? "Audio saved for retry"
											: "Couldn't finish"
								}
							>
								{message}
								{recoveryId && !complete && (
									<Text size="sm" mt="xs">
										Retry uses your saved audio and completed-upload progress.
										You can also find it in Home → Recording options → Saved
										recordings.
									</Text>
								)}
							</Alert>
						)}
						{complete && !pending ? (
							<Group justify="space-between">
								<Group gap="xs" role="status">
									<Check size={18} />
									<Text size="sm">Saved to History</Text>
								</Group>
								{recoveryId && (
									<Button
										variant="default"
										disabled={disabled}
										onClick={() => {
											setMessage(null);
											job.mutate();
										}}
									>
										Finish cleanup
									</Button>
								)}
								{onOpenHistory && (
									<Button
										variant="light"
										leftSection={<History size={16} />}
										onClick={onOpenHistory}
									>
										Open History
									</Button>
								)}
							</Group>
						) : pending ? (
							<Group justify="space-between">
								<Group gap="sm" role="status">
									<Loader size="sm" />
									<div>
										<Text size="sm" fw={500}>
											{complete ? "Finishing cleanup…" : "Processing audio…"}
										</Text>
										<Text size="xs" c="dimmed">
											You can leave this page. Results appear in History.
										</Text>
									</div>
								</Group>
								<Button
									variant="default"
									onClick={() => cancel.mutate()}
									loading={cancel.isPending}
								>
									Cancel
								</Button>
							</Group>
						) : (
							<Group justify="space-between">
								<Text size="xs" c="dimmed">
									{busy && state.data
										? "Finish the current recording or transcription first."
										: "Full recording · Result goes to History"}
								</Text>
								<Button
									leftSection={<Upload size={16} />}
									disabled={!canTranscribe}
									onClick={() => {
										setMessage(null);
										job.mutate();
									}}
								>
									{recoveryId ? "Retry saved audio" : "Transcribe"}
								</Button>
							</Group>
						)}
					</Stack>
				</Paper>
			</div>
			{active && modelOpen && preferences && (
				<MeetingModelDialog
					preferences={preferences}
					saving={false}
					onClose={() => setModelOpen(false)}
					onSave={(value) => {
						setPreferences(value);
						setModelOpen(false);
					}}
				/>
			)}
		</div>
	);
}
