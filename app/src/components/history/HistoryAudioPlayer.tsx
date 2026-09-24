import {
	ActionIcon,
	Button,
	Group,
	Select,
	Stack,
	Text,
	Tooltip,
} from "@mantine/core";
import { Expand, Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import WaveSurfer from "wavesurfer.js";
import type { RecordingPlayerControls } from "../../lib/useRecordingPlayer";

export function audioTime(seconds: number): string {
	const total = Math.max(0, Math.floor(seconds || 0));
	const hours = Math.floor(total / 3600);
	return `${hours ? `${hours}:` : ""}${hours ? String(Math.floor(total / 60) % 60).padStart(2, "0") : Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

export function HistoryAudioPlayer({
	player,
	recordingId,
	onOpenFullView,
}: {
	player: RecordingPlayerControls;
	recordingId: string;
	onOpenFullView?: () => void;
}) {
	const container = useRef<HTMLDivElement>(null);
	const [waveformError, setWaveformError] = useState(false);
	const [renderedMedia, setRenderedMedia] = useState<HTMLAudioElement | null>(
		null,
	);
	const active = player.id === recordingId;
	const waveform = active ? player.waveform : null;
	const media = player.media;
	const ready = Boolean(active && player.ready && !player.error && media);
	const waveformRevealed = ready && renderedMedia === media && !waveformError;
	useEffect(() => {
		setWaveformError(false);
		void player.prepare(recordingId);
	}, [player.prepare, recordingId]);
	useEffect(() => {
		if (!container.current || !media || !waveform) return;
		setWaveformError(false);
		let wave: WaveSurfer | undefined;
		let disposed = false;
		try {
			wave = WaveSurfer.create({
				container: container.current,
				media,
				peaks: [waveform.peaks],
				duration: waveform.duration_seconds,
				height: 64,
				waveColor: "#61756b",
				progressColor: "#19bf65",
				cursorColor: "#19bf65",
				barWidth: 2,
				barGap: 2,
				barRadius: 2,
				normalize: false,
				interact: true,
				dragToSeek: true,
			});
			wave.on("error", () => setWaveformError(true));
			wave.on("redrawcomplete", () => {
				if (!disposed) setRenderedMedia(media);
			});
		} catch {
			setWaveformError(true);
		}
		return () => {
			disposed = true;
			wave?.destroy();
		};
	}, [media, waveform]);
	return (
		<Stack
			gap="xs"
			className={`history-audio-player${ready ? " history-audio-player--ready" : " history-audio-player--loading"}`}
			aria-busy={!ready && !player.error}
		>
			<div
				className={`history-waveform-stage${waveformRevealed ? " history-waveform-stage--revealed" : ""}`}
			>
				<div className="history-waveform-placeholder" aria-hidden="true" />
				<div
					ref={container}
					className="history-waveform"
					role="slider"
					tabIndex={ready ? 0 : -1}
					aria-disabled={!ready}
					aria-label="Playback position"
					aria-valuemin={0}
					aria-valuemax={Math.max(1, Math.round(player.duration))}
					aria-valuenow={Math.round(player.position)}
					aria-valuetext={`${audioTime(player.position)} of ${audioTime(player.duration)}`}
					onKeyDown={(event) => {
						if (!ready) return;
						if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
							event.preventDefault();
							player.seek(
								player.position + (event.key === "ArrowLeft" ? -5 : 5),
							);
						} else if (event.key === "Home" || event.key === "End") {
							event.preventDefault();
							player.seek(event.key === "Home" ? 0 : player.duration);
						}
					}}
				/>
			</div>
			{waveformError && (
				<Text size="xs" c="dimmed">
					Waveform unavailable.
				</Text>
			)}
			{player.error && (
				<Group justify="space-between" gap="xs" wrap="nowrap">
					<Text size="sm" c="dimmed">
						{player.error}
					</Text>
					<Button
						variant="subtle"
						size="xs"
						onClick={() => void player.prepare(recordingId)}
					>
						Retry audio
					</Button>
				</Group>
			)}
			<div className="history-audio-controls">
				<Text
					size="xs"
					c="dimmed"
					className="history-audio-time"
					style={{ fontVariantNumeric: "tabular-nums" }}
				>
					{audioTime(player.position)} / {audioTime(player.duration)}
				</Text>
				<Group gap="xs" className="history-audio-buttons">
					<ActionIcon
						variant="subtle"
						aria-label="Back 10 seconds"
						disabled={!ready}
						onClick={() => player.seek(player.position - 10)}
					>
						<RotateCcw size={17} />
					</ActionIcon>
					<ActionIcon
						variant="light"
						radius="xl"
						size="lg"
						aria-label={player.playing ? "Pause audio" : "Play audio"}
						disabled={!ready}
						onClick={() => void player.toggle(recordingId)}
					>
						{player.playing ? <Pause size={18} /> : <Play size={18} />}
					</ActionIcon>
					<ActionIcon
						variant="subtle"
						aria-label="Forward 10 seconds"
						disabled={!ready}
						onClick={() => player.seek(player.position + 10)}
					>
						<RotateCw size={17} />
					</ActionIcon>
				</Group>
				<Group gap="xs" className="history-audio-trailing" wrap="nowrap">
					<Select
						className="history-audio-speed"
						aria-label="Playback speed"
						size="xs"
						w={85}
						value={String(player.rate)}
						disabled={!ready}
						onChange={(v) => player.setRate(Number(v))}
						data={[0.5, 0.75, 1, 1.25, 1.5, 2].map((r) => ({
							value: String(r),
							label: `${r}×`,
						}))}
						allowDeselect={false}
					/>
					{onOpenFullView ? (
						<Tooltip label="Open full view" withArrow>
							<ActionIcon
								variant="subtle"
								aria-label="Open full view"
								onClick={onOpenFullView}
							>
								<Expand size={17} />
							</ActionIcon>
						</Tooltip>
					) : null}
				</Group>
			</div>
		</Stack>
	);
}
