import {
	ActionIcon,
	Alert,
	Badge,
	Button,
	Group,
	Loader,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, FileAudio, FolderOpen, History, Upload, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import {
	audioFileName,
	audioFilesAPI,
	isSupportedAudioFile,
} from "../lib/tauri/audioFiles";
import { recordingControlsAPI } from "../lib/tauri/commands";
import type { RecordingPreferences } from "../lib/tauri/types";
import { FileModelPicker } from "./FileModelPicker";
import styles from "./FileTranscription.module.css";

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
	const [modelReady, setModelReady] = useState(false);
	const [draftPreferences, setPreferences] =
		useState<RecordingPreferences | null>(null);
	const initial = useQuery({
		queryKey: ["recording-preferences"],
		queryFn: recordingControlsAPI.getPreferences,
		enabled: active,
	});
	// Follow recorder defaults while pristine, then keep this file's explicit
	// options independent of changes made elsewhere (including while hidden).
	const preferences = useMemo(
		() =>
			draftPreferences ??
			(initial.data ? { ...initial.data, mode: "meeting" as const } : null),
		[draftPreferences, initial.data],
	);
	useEffect(() => {
		if (path && initial.data)
			setPreferences(
				(existing) => existing ?? { ...initial.data, mode: "meeting" },
			);
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
		(modelReady || !!recoveryId);
	return (
		<div className={`main-content ${styles.page}`}>
			<div
				className={`main-content-inner page-content-start ${styles.workspace}`}
			>
				<div className={hovered && !disabled ? styles.dropActive : styles.drop}>
					<Stack
						align="center"
						justify="center"
						gap="md"
						className={styles.dropTarget}
					>
						<div className={styles.fileIcon}>
							<FileAudio size={32} strokeWidth={1.5} aria-hidden="true" />
						</div>
						{path ? (
							<Group justify="center" wrap="nowrap" className={styles.fileName}>
								<Text fw={500} size="lg" lineClamp={2}>
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
						) : (
							<>
								<Title order={2} size="h3" fw={500}>
									Drop an audio file here
								</Title>
								<Text c="dimmed" size="sm">
									WAV, MP3, FLAC or AAC
								</Text>
							</>
						)}
						{complete && (
							<Badge variant="light" color="green">
								Transcribed
							</Badge>
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
					<Stack gap="lg" className={styles.options}>
						<FileModelPicker
							value={preferences?.meeting_model ?? null}
							disabled={!preferences || disabled || !!recoveryId || complete}
							onReadyChange={setModelReady}
							onChange={(meeting_model) => {
								setPreferences({ mode: "meeting", meeting_model });
							}}
						/>
						{initial.isError && !preferences && (
							<Alert color="red">
								Couldn’t load transcription options.{" "}
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
							<Alert color="red">
								Recording controls unavailable.{" "}
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
						{message && (
							<Alert color={complete ? "orange" : "red"}>{message}</Alert>
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
									<Text size="sm">
										{complete ? "Finishing cleanup…" : "Processing audio…"}
									</Text>
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
										? "Recorder is busy"
										: "Up to 4 hours · 2 GiB"}
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
				</div>
			</div>
		</div>
	);
}
