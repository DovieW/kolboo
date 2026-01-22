import {
	Accordion,
	Button,
	Group,
	NumberInput,
	Switch,
	Text,
	Tooltip,
} from "@mantine/core";
import { useEffect, useState } from "react";
import {
	useAudioSettingsTestStartRecording,
	useAudioSettingsTestStopRecording,
	useLastRecordingDiagnostics,
	useSettings,
	useUpdateAudioAgcEnabled,
	useUpdateAudioDownmixToMono,
	useUpdateAudioHighpassEnabled,
	useUpdateAudioNoiseSuppressionEnabled,
	useUpdateAudioResampleTo16khz,
	useUpdateHotMicEnabled,
	useUpdateHotMicPreRollMs,
	useUpdateMicAutoRecoverEnabled,
	useUpdateNoiseGateThresholdDbfs,
	useUpdateQuietAudioGateEnabled,
	useUpdateQuietAudioMinDurationSecs,
	useUpdateQuietAudioPeakDbfsThreshold,
	useUpdateQuietAudioRequireSpeech,
	useUpdateQuietAudioRmsDbfsThreshold,
} from "../../lib/queries";
import type { RewriteProgramPromptProfile } from "../../lib/tauri";
import { DeviceSelector } from "../DeviceSelector";
import { SettingsRow } from "./SettingsRow";

const GLOBAL_ONLY_TOOLTIP =
	"This setting can only be changed in the Default profile";

export function AudioSettings({
	editingProfileId,
}: {
	editingProfileId?: string;
}) {
	const { data: settings } = useSettings();
	const { data: lastRecordingDiagnostics } = useLastRecordingDiagnostics();

	const updateQuietAudioGateEnabled = useUpdateQuietAudioGateEnabled();
	const updateQuietAudioMinDurationSecs = useUpdateQuietAudioMinDurationSecs();
	const updateQuietAudioRmsDbfsThreshold =
		useUpdateQuietAudioRmsDbfsThreshold();
	const updateQuietAudioPeakDbfsThreshold =
		useUpdateQuietAudioPeakDbfsThreshold();
	const updateQuietAudioRequireSpeech = useUpdateQuietAudioRequireSpeech();

	const updateNoiseGateThresholdDbfs = useUpdateNoiseGateThresholdDbfs();
	const updateAudioDownmixToMono = useUpdateAudioDownmixToMono();
	const updateAudioResampleTo16khz = useUpdateAudioResampleTo16khz();
	const updateAudioHighpassEnabled = useUpdateAudioHighpassEnabled();
	const updateAudioAgcEnabled = useUpdateAudioAgcEnabled();
	const updateAudioNoiseSuppressionEnabled =
		useUpdateAudioNoiseSuppressionEnabled();

	const updateHotMicEnabled = useUpdateHotMicEnabled();
	const updateHotMicPreRollMs = useUpdateHotMicPreRollMs();
	const updateMicAutoRecoverEnabled = useUpdateMicAutoRecoverEnabled();

	const audioTestStart = useAudioSettingsTestStartRecording();
	const audioTestStop = useAudioSettingsTestStopRecording();

	const profiles = settings?.rewrite_program_prompt_profiles ?? [];
	const profile: RewriteProgramPromptProfile | null =
		editingProfileId && editingProfileId !== "default"
			? (profiles.find((p) => p.id === editingProfileId) ?? null)
			: null;

	const isProfileScope = profile !== null;

	const quietAudioGateEnabled = settings?.quiet_audio_gate_enabled ?? true;
	const quietAudioMinDurationSecs =
		settings?.quiet_audio_min_duration_secs ?? 0.15;
	const quietAudioRmsDbfsThreshold =
		settings?.quiet_audio_rms_dbfs_threshold ?? -60;
	const quietAudioPeakDbfsThreshold =
		settings?.quiet_audio_peak_dbfs_threshold ?? -50;
	const quietAudioRequireSpeech = settings?.quiet_audio_require_speech ?? false;

	const audioDownmixToMono = settings?.audio_downmix_to_mono ?? true;
	const audioResampleTo16khz = settings?.audio_resample_to_16khz ?? false;
	const audioHighpassEnabled = settings?.audio_highpass_enabled ?? true;
	const audioAgcEnabled = settings?.audio_agc_enabled ?? false;
	const audioNoiseSuppressionEnabled =
		settings?.audio_noise_suppression_enabled ?? false;

	const hotMicEnabled = settings?.hot_mic_enabled ?? false;
	const hotMicPreRollMs = settings?.hot_mic_pre_roll_ms ?? 1500;
	const micAutoRecoverEnabled = settings?.mic_auto_recover_enabled ?? false;

	const noiseGateThresholdDbfsFromSettings =
		settings?.noise_gate_threshold_dbfs ?? null;
	const [noiseGateThresholdDraft, setNoiseGateThresholdDraft] = useState<
		number | "" | null
	>(null);

	useEffect(() => {
		void noiseGateThresholdDbfsFromSettings;
		// If settings update elsewhere (or after save), drop any draft value.
		setNoiseGateThresholdDraft(null);
	}, [noiseGateThresholdDbfsFromSettings]);

	const noiseGateThresholdDbfs =
		noiseGateThresholdDraft ??
		(noiseGateThresholdDbfsFromSettings == null
			? ""
			: noiseGateThresholdDbfsFromSettings);

	const commitNoiseGateThresholdDbfs = () => {
		if (noiseGateThresholdDraft == null) return;
		const next =
			typeof noiseGateThresholdDraft === "number"
				? noiseGateThresholdDraft
				: null;
		updateNoiseGateThresholdDbfs.mutate(next);
		setNoiseGateThresholdDraft(null);
	};

	const lastStats = lastRecordingDiagnostics?.stats ?? null;
	const lastSpeechDetected = lastRecordingDiagnostics?.speech_detected ?? null;

	const ampToDbfs = (amp: number): number | null => {
		if (!Number.isFinite(amp) || amp <= 0) return null;
		return 20 * Math.log10(amp);
	};

	const formatDbfs = (dbfs: number | null): string => {
		if (dbfs == null) return "—";
		if (!Number.isFinite(dbfs)) return "-∞";
		return `${dbfs.toFixed(1)}`;
	};

	const lastRmsDbfs = lastStats ? ampToDbfs(lastStats.rms) : null;
	const lastPeakDbfs = lastStats ? ampToDbfs(lastStats.peak) : null;
	const lastRecordingSummary = lastStats
		? `Last recording: ${lastStats.duration_secs.toFixed(
				2,
			)}s · RMS ${formatDbfs(lastRmsDbfs)} dBFS · Peak ${formatDbfs(
				lastPeakDbfs,
			)} dBFS${
				lastSpeechDetected == null
					? ""
					: ` · Speech ${lastSpeechDetected ? "yes" : "no"}`
			}`
		: "Last recording: —";

	const peakSeemsAuto =
		Math.abs(quietAudioPeakDbfsThreshold - (quietAudioRmsDbfsThreshold + 10)) <
		0.75;

	const [testRawSrc, setTestRawSrc] = useState<string | null>(null);
	const [testProcessedSrc, setTestProcessedSrc] = useState<string | null>(null);
	const [isTestRecording, setIsTestRecording] = useState(false);

	const content = (
		<>
			<DeviceSelector />

			<SettingsRow
				label="Hot Mic Mode"
				description={
					"Keep the microphone open (no transcription) and buffer a short pre-roll so you don’t miss the first word"
				}
				right={
					<div style={{ display: "flex", alignItems: "center", gap: 10 }}>
					<Text size="xs" c="dimmed" style={{ whiteSpace: "nowrap" }}>
						Pre-roll: {hotMicPreRollMs} ms
					</Text>
					<NumberInput
						value={hotMicPreRollMs}
						onChange={(value) => {
							const next = typeof value === "number" ? value : 1500;
							updateHotMicPreRollMs.mutate(next);
						}}
						min={0}
						max={5000}
						step={100}
						disabled={isProfileScope || !hotMicEnabled}
						placeholder="Pre-roll (ms)"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								width: 140,
							},
						}}
					/>
					<Switch
						checked={hotMicEnabled}
						onChange={(event) =>
							updateHotMicEnabled.mutate(event.currentTarget.checked)
						}
						disabled={isProfileScope}
						color="gray"
						size="md"
					/>
					</div>
				}
			/>

			<SettingsRow
				label="Auto-recover microphone"
				description={
					"If the mic stream stops delivering audio (disconnects, sleep, device changes), attempt to restart automatically"
				}
				right={
					<Switch
					checked={micAutoRecoverEnabled}
					onChange={(event) =>
						updateMicAutoRecoverEnabled.mutate(event.currentTarget.checked)
					}
					disabled={isProfileScope}
					color="gray"
					size="md"
				/>
				}
			/>

			<SettingsRow
				noDivider
				left={
					<div>
						<p className="settings-label">Skip quiet recordings</p>
						<p className="settings-description">
							Skip transcription when the recording is basically quiet
							(Sensitivity sets the RMS threshold in dBFS)
						</p>
						<p className="settings-description">{lastRecordingSummary}</p>
					</div>
				}
				right={
					<div style={{ display: "flex", alignItems: "center", gap: 10 }}>
					<Switch
						checked={quietAudioGateEnabled}
						onChange={(event) =>
							updateQuietAudioGateEnabled.mutate(event.currentTarget.checked)
						}
						disabled={isProfileScope}
						color="gray"
						size="md"
					/>
					<NumberInput
						value={quietAudioRmsDbfsThreshold}
						onChange={(value) => {
							const next = typeof value === "number" ? value : -60;
							updateQuietAudioRmsDbfsThreshold.mutate(next);

							// If peak looks like it's in the "auto" relationship with RMS, keep it in sync.
							if (peakSeemsAuto) {
								updateQuietAudioPeakDbfsThreshold.mutate(
									Math.min(0, next + 10),
								);
							}
						}}
						min={-120}
						max={0}
						step={1}
						disabled={isProfileScope || !quietAudioGateEnabled}
						placeholder="Sensitivity (RMS dBFS)"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								width: 140,
							},
						}}
					/>
					</div>
				}
			/>

			<div style={{ marginTop: 0, marginBottom: 0 }}>
				<Accordion variant="separated" radius="md">
					<Accordion.Item value="audio-test">
						<Accordion.Control>
							<div>
								<p className="settings-label">Test audio settings</p>
								<p className="settings-description">
									Record once, then compare “before” (raw) vs “after” (with your
									current audio settings)
								</p>
							</div>
						</Accordion.Control>
						<Accordion.Panel>
							<div
								style={{ display: "flex", flexDirection: "column", gap: 12 }}
							>
								<div style={{ display: "flex", gap: 10, alignItems: "center" }}>
									<Button
										color="gray"
										disabled={
											isProfileScope ||
											audioTestStart.isPending ||
											audioTestStop.isPending
										}
										loading={
											audioTestStart.isPending || audioTestStop.isPending
										}
										onClick={() => {
											if (isTestRecording) {
												audioTestStop.mutate(undefined, {
													onSuccess: (res) => {
														setIsTestRecording(false);
														setTestRawSrc(
															`data:audio/wav;base64,${res.raw_wav_base64}`,
														);
														setTestProcessedSrc(
															`data:audio/wav;base64,${res.processed_wav_base64}`,
														);
													},
													onError: () => {
														setIsTestRecording(false);
													},
												});
												return;
											}

											setTestRawSrc(null);
											setTestProcessedSrc(null);
											audioTestStart.mutate(undefined, {
												onSuccess: () => setIsTestRecording(true),
											});
										}}
									>
										{isTestRecording ? "Stop" : "Record"}
									</Button>

									<Text size="sm" c="dimmed">
										{isTestRecording
											? "Recording… (this is raw capture; processing happens after you stop)"
											: "Tip: talk normally for 3–5 seconds"}
									</Text>
								</div>

								<div
									style={{ display: "flex", flexDirection: "column", gap: 10 }}
								>
									<div>
										<Text size="sm" fw={500}>
											Before
										</Text>
										<Text size="xs" c="dimmed" mb={6}>
											Raw capture (no filters applied)
										</Text>
										{/* biome-ignore lint/a11y/useMediaCaption: this is a dev/test audio playback widget; captions/tracks are not applicable for raw diagnostic audio */}
										<audio
											controls
											src={testRawSrc ?? undefined}
											style={{ width: "100%" }}
										/>
									</div>

									<div>
										<Text size="sm" fw={500}>
											After
										</Text>
										<Text size="xs" c="dimmed" mb={6}>
											Your current audio settings applied (noise gate, mono,
											HPF, AGC, etc.)
										</Text>
										{/* biome-ignore lint/a11y/useMediaCaption: this is a dev/test audio playback widget; captions/tracks are not applicable for processed diagnostic audio */}
										<audio
											controls
											src={testProcessedSrc ?? undefined}
											style={{ width: "100%" }}
										/>
									</div>
								</div>
							</div>
						</Accordion.Panel>
					</Accordion.Item>
				</Accordion>
			</div>

			<SettingsRow
				label="Skip quiet — Require speech"
				description="Extra skip: don't transcribe if VAD finds no speech"
				right={
					<Switch
					checked={quietAudioRequireSpeech}
					onChange={(event) =>
						updateQuietAudioRequireSpeech.mutate(event.currentTarget.checked)
					}
					disabled={isProfileScope || !quietAudioGateEnabled}
					color="gray"
					size="md"
				/>
				}
			/>

			<SettingsRow
				label="Skip quiet — Minimum duration"
				description="Treat very short recordings as quiet (seconds)"
				right={
					<Group gap={8} align="center">
					<NumberInput
						value={quietAudioMinDurationSecs}
						onChange={(value) => {
							const next = typeof value === "number" ? value : 0;
							updateQuietAudioMinDurationSecs.mutate(next);
						}}
						min={0}
						max={5}
						step={0.05}
						decimalScale={2}
						disabled={isProfileScope || !quietAudioGateEnabled}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								width: 140,
							},
						}}
					/>
					</Group>
				}
			/>

			<SettingsRow
				label="Skip quiet — Peak threshold"
				description="Peak level below this is considered quiet (dBFS)"
				right={
					<Group gap={8} align="center">
					<NumberInput
						value={quietAudioPeakDbfsThreshold}
						onChange={(value) => {
							const next = typeof value === "number" ? value : -40;
							updateQuietAudioPeakDbfsThreshold.mutate(next);
						}}
						min={-120}
						max={0}
						step={1}
						disabled={isProfileScope || !quietAudioGateEnabled}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								width: 140,
							},
						}}
					/>
					</Group>
				}
			/>

			<SettingsRow
				label="Noise gate threshold"
				description="Set how quiet audio must be to be suppressed. Blank = Off."
				right={
					<Group gap={8} align="center">
					<NumberInput
						value={noiseGateThresholdDbfs}
						onChange={(value) => {
							// Mantine NumberInput can emit string/number. Our draft state
							// allows only number, empty string (""), or null.
							if (value === "") {
								setNoiseGateThresholdDraft("");
								return;
							}
							if (value == null) {
								setNoiseGateThresholdDraft(null);
								return;
							}
							if (typeof value === "number") {
								setNoiseGateThresholdDraft(value);
								return;
							}
							// Any other string => treat as "blank" until commit.
							setNoiseGateThresholdDraft("");
						}}
						onBlur={commitNoiseGateThresholdDbfs}
						onKeyDown={(e) => {
							if (e.key === "Enter") commitNoiseGateThresholdDbfs();
						}}
						min={-75}
						max={-30}
						step={1}
						placeholder="Off"
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								width: 140,
							},
						}}
					/>
					</Group>
				}
			/>

			<SettingsRow
				label="Convert to mono"
				description="Downmix stereo/multi-channel inputs for more reliable VAD and STT"
				right={
					<Switch
						checked={audioDownmixToMono}
						onChange={(event) =>
							updateAudioDownmixToMono.mutate(event.currentTarget.checked)
						}
						disabled={isProfileScope}
						color="gray"
						size="md"
					/>
				}
			/>

			<SettingsRow
				label="High-pass filter"
				description="Removes DC/rumble to help voice clarity (very lightweight)"
				right={
					<Switch
					checked={audioHighpassEnabled}
					onChange={(event) =>
						updateAudioHighpassEnabled.mutate(event.currentTarget.checked)
					}
					disabled={isProfileScope}
					color="gray"
					size="md"
				/>
				}
			/>

			<SettingsRow
				label="Auto gain (AGC)"
				description="Normalizes volume so quiet voices are easier to transcribe"
				right={
					<Switch
					checked={audioAgcEnabled}
					onChange={(event) =>
						updateAudioAgcEnabled.mutate(event.currentTarget.checked)
					}
					disabled={isProfileScope}
					color="gray"
					size="md"
				/>
				}
			/>

			<SettingsRow
				label="Noise suppression (light)"
				description="Reduces steady background noise (best-effort, stop-time)"
				right={
					<Switch
					checked={audioNoiseSuppressionEnabled}
					onChange={(event) =>
						updateAudioNoiseSuppressionEnabled.mutate(
							event.currentTarget.checked,
						)
					}
					disabled={isProfileScope}
					color="gray"
					size="md"
				/>
				}
			/>

			<SettingsRow
				label="Resample to 16 kHz"
				description="Helpful for some STT engines; costs a bit of CPU at stop time"
				right={
					<Switch
					checked={audioResampleTo16khz}
					onChange={(event) =>
						updateAudioResampleTo16khz.mutate(event.currentTarget.checked)
					}
					disabled={isProfileScope}
					color="gray"
					size="md"
				/>
				}
			/>
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
