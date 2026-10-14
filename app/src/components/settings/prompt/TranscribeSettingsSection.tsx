import {
	Accordion,
	ActionIcon,
	Button,
	NumberInput,
	Select,
	Switch,
	Text,
	Textarea,
	Tooltip,
} from "@mantine/core";
import { ActionModelPicker } from "../../ActionModelPicker";
import { Info, RotateCcw } from "lucide-react";
import {
	isRealtimeSttModel,
	isStreamingOnlySttModel,
} from "../../../lib/modelOptions";
import type { SttInputMode } from "../../../lib/tauri";
import { TranscriptionInputModeControl } from "./TranscriptionInputModeControl";
import { useModelChoices } from "../../../lib/queries/modelChoices";
import {
	useSettings,
	useUpdateSTTLiveOutput,
	useUpdateSTTSimulatedStreaming,
	useUpdateSTTInputMode,
	useAvailableProviders,
} from "../../../lib/queries";

interface TranscribeSettingsSectionProps {
	activeProfileId: string;
	sttInputMode?: SttInputMode | null;
	onSttInputModeChange?: (mode: SttInputMode | null) => void;
	isDefaultScope: boolean;
	inheritTooltip: string;
	sttLanguageInheriting: boolean;
	sttTimeoutInheriting: boolean;
	effectiveSttProvider: string | null;
	selectedSttModelForUi: string | null;
	sttLanguageOptions: Array<{ value: string; label: string }>;
	localProfileSttLanguage: string;
	onSttLanguageChange: (value: string | null) => void;
	onDisableSttLanguageOverride: () => void;
	onDisableSttTimeoutOverride: () => void;
	localProfileSttTimeout: number | string;
	onSttTimeoutChange: (value: number | string) => void;
	onSttTimeoutBlur: () => void;
	sttPromptSupported: boolean;
	sttPromptDisabledReason: string;
	sttPromptMaxChars: number;
	isPrompt224CharLimited: boolean;
	localSttTranscriptionPrompt: string;
	onSttPromptChange: (value: string) => void;
	sttTestDurationMs: number | null;
	sttTestError: string;
	sttTestOutput: string;
	hasLastAudioForSttTest: boolean;
	isSttTestRunning: boolean;
	onRunSttTest: () => void;
	hasStoredTranscriptionPrompt: boolean;
}

export function TranscribeSettingsSection({
	activeProfileId,
	sttInputMode = null,
	onSttInputModeChange,
	isDefaultScope,
	inheritTooltip,
	sttLanguageInheriting,
	sttTimeoutInheriting,
	effectiveSttProvider,
	selectedSttModelForUi,
	sttLanguageOptions,
	localProfileSttLanguage,
	onSttLanguageChange,
	onDisableSttLanguageOverride,
	onDisableSttTimeoutOverride,
	localProfileSttTimeout,
	onSttTimeoutChange,
	onSttTimeoutBlur,
	sttPromptSupported,
	sttPromptDisabledReason,
	sttPromptMaxChars,
	isPrompt224CharLimited,
	localSttTranscriptionPrompt,
	onSttPromptChange,
	sttTestDurationMs,
	sttTestError,
	sttTestOutput,
	hasLastAudioForSttTest,
	isSttTestRunning,
	onRunSttTest,
	hasStoredTranscriptionPrompt,
}: TranscribeSettingsSectionProps) {
	const { data: globalSettings } = useSettings();
	const { data: providers } = useAvailableProviders();
	const { choices } = useModelChoices("dictation");
	const updateSTTLiveOutput = useUpdateSTTLiveOutput();
	const updateSTTSimulatedStreaming = useUpdateSTTSimulatedStreaming();
	const updateSTTInputMode = useUpdateSTTInputMode();
	const globalInputMode = globalSettings?.stt_input_mode ?? "auto";
	const effectiveInputMode = isDefaultScope
		? globalInputMode
		: (sttInputMode ?? globalInputMode);

	const isNativeRealtime = isRealtimeSttModel(
		effectiveSttProvider,
		selectedSttModelForUi,
	);
	const profile = globalSettings?.rewrite_program_prompt_profiles?.find(
		(profile) => profile.id === activeProfileId,
	);
	const source = (
		profile?.model_selections?.transcription ??
		globalSettings?.model_selections?.transcription
	)?.source;
	const legacyManaged =
		globalSettings?.stt_use_managed_inference === true &&
		choices.some(
			(choice) =>
				choice.ready &&
				choice.selection.source === "managed" &&
				choice.selection.provider_id === effectiveSttProvider &&
				choice.selection.model_id === selectedSttModelForUi,
		);
	const byokInput = source ? source === "byok" : !legacyManaged;

	const showSimulatedStreaming =
		byokInput && !isNativeRealtime && effectiveInputMode === "auto";
	const simulatedStreamingEnabled =
		globalSettings?.stt_simulated_streaming ?? false;
	const showLiveOutput =
		byokInput &&
		effectiveInputMode === "auto" &&
		(isNativeRealtime || simulatedStreamingEnabled);
	const rewritingEnabled =
		profile?.rewrite_llm_enabled ??
		(isDefaultScope ? globalSettings?.rewrite_llm_enabled !== false : true);
	const replacementEnabled =
		profile?.quick_replace_enabled ??
		globalSettings?.quick_replace_enabled ??
		false;
	const routingEnabled = profile?.router?.enabled === true;
	const liveOutputAvailable =
		providers?.live_text_insertion_supported === true &&
		!rewritingEnabled &&
		!replacementEnabled &&
		!routingEnabled;

	return (
		<>
			<div className="settings-mini-header settings-mini-header--first">
				<span className="settings-mini-header__text">Transcribe</span>
			</div>

			<div className="settings-row">
				<ActionModelPicker
					action="transcription"
					label="Transcription model"
					profileId={activeProfileId}
				/>
			</div>
				<TranscriptionInputModeControl
					value={isDefaultScope ? globalInputMode : sttInputMode}
					inheritedMode={globalInputMode}
					canInherit={!isDefaultScope}
					realtimeOnly={isStreamingOnlySttModel(
						effectiveSttProvider ?? "",
						selectedSttModelForUi ?? "",
					)}
					disabled={updateSTTInputMode.isPending}
					onChange={(mode) => {
						if (isDefaultScope && mode) updateSTTInputMode.mutate(mode);
						else onSttInputModeChange?.(mode);
					}}
				/>

			{/* Simulated Streaming — only for non-realtime STT models */}
			{!byokInput && (
				<Text size="xs" c="dimmed">
					Kolboo Managed uploads after recording. Realtime and simulated input
					are available only for Your models in this phase.
				</Text>
			)}
			{showSimulatedStreaming && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Simulated Streaming (experimental)</p>
						<p className="settings-description">
							Periodically transcribe audio chunks during recording for
							progressive results. This is not realtime input and can send
							additional billable requests. Off by default.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						<Switch
							checked={simulatedStreamingEnabled}
							onChange={(e) =>
								updateSTTSimulatedStreaming.mutate(e.currentTarget.checked)
							}
							color="gray"
							size="md"
						/>
					</div>
				</div>
			)}

			{/* Live Output — for realtime or simulated streaming models */}
			{showLiveOutput && (
				<div className="settings-row">
					<div>
						<p className="settings-label">
							Live Output{" "}
							<span style={{ fontWeight: 400, opacity: 0.6 }}>
								(experimental)
							</span>
						</p>
						<p className="settings-description">
							Paste committed chunks as they arrive. Advanced and opt-in; stops
							if the focused target changes and never replaces text already
							inserted.
						</p>
						{!liveOutputAvailable && (
							<p className="settings-description">
								Live output requires rewriting, Quick Replace and routing to be
								off, and a desktop session that supports safe insertion.
								Otherwise Kolboo previews the transcript and outputs once after
								stopping.
							</p>
						)}
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						<Switch
							checked={globalSettings?.stt_live_output ?? false}
							disabled={!liveOutputAvailable || updateSTTLiveOutput.isPending}
							onChange={(e) =>
								updateSTTLiveOutput.mutate(e.currentTarget.checked)
							}
							color="gray"
							size="md"
						/>
					</div>
				</div>
			)}

			<div className="settings-row">
				<div>
					<p className="settings-label">STT Language</p>
					<p className="settings-description">
						Language hint for transcription (Auto-detect available)
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && sttLanguageInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !sttLanguageInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableSttLanguageOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						data={sttLanguageOptions}
						value={localProfileSttLanguage}
						onChange={onSttLanguageChange}
						placeholder="Auto-detect"
						withCheckIcon={false}
						styles={{
							input: {
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							},
						}}
					/>
				</div>
			</div>

			<div className="settings-row no-divider">
				<div>
					<p className="settings-label">STT Timeout</p>
					<p className="settings-description">
						Increase if nothing is getting transcribed (seconds)
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && sttTimeoutInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !sttTimeoutInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableSttTimeoutOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}

					<NumberInput
						value={localProfileSttTimeout}
						onChange={onSttTimeoutChange}
						onBlur={onSttTimeoutBlur}
						min={5}
						max={120}
						step={1}
						clampBehavior="blur"
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
			</div>

			<div style={{ marginTop: 0, marginBottom: 16 }}>
				<Accordion variant="separated" radius="md">
					<Accordion.Item value={`${activeProfileId}-stt-prompt`}>
						<Accordion.Control>
							<Tooltip
								label={sttPromptDisabledReason}
								withArrow
								disabled={sttPromptSupported}
							>
								<div style={{ opacity: sttPromptSupported ? 1 : 0.5 }}>
									<p className="settings-label">Transcription prompt</p>
									<p className="settings-description">
										Optional context used during transcription.
									</p>
								</div>
							</Tooltip>
						</Accordion.Control>
						<Accordion.Panel>
							<div
								style={{ display: "flex", flexDirection: "column", gap: 10 }}
							>
								<div style={{ width: "100%" }}>
									<div
										style={{
											display: "flex",
											alignItems: "center",
											justifyContent: "space-between",
											gap: 12,
											marginBottom: 6,
										}}
									>
										{isPrompt224CharLimited ? (
											<Text size="xs" c="dimmed">
												{localSttTranscriptionPrompt.length}/{sttPromptMaxChars}{" "}
												chars
											</Text>
										) : null}
									</div>

									<Textarea
										value={localSttTranscriptionPrompt}
										onChange={(e) => {
											const next = e.currentTarget.value;

											if (
												isPrompt224CharLimited &&
												next.length > sttPromptMaxChars
											) {
												onSttPromptChange(next.slice(0, sttPromptMaxChars));
												return;
											}

											onSttPromptChange(next);
										}}
										disabled={!sttPromptSupported}
										placeholder={"Prompt"}
										autosize
										minRows={2}
										maxLength={
											isPrompt224CharLimited ? sttPromptMaxChars : undefined
										}
										styles={{
											input: {
												backgroundColor: "var(--bg-elevated)",
												borderColor: "var(--border-default)",
												color: "var(--text-primary)",
												fontFamily: "monospace",
												fontSize: "13px",
											},
										}}
									/>
								</div>
							</div>
						</Accordion.Panel>
					</Accordion.Item>

					<Accordion.Item value={`${activeProfileId}-stt-test`}>
						<Accordion.Control>
							<div>
								<p className="settings-label">Test transcription</p>
								<p className="settings-description">
									Run STT on the last recorded audio to validate provider/model
									settings.
								</p>
							</div>
						</Accordion.Control>
						<Accordion.Panel>
							<div
								style={{ display: "flex", flexDirection: "column", gap: 10 }}
							>
								<div
									style={{
										display: "flex",
										alignItems: "center",
										justifyContent: "space-between",
										gap: 12,
									}}
								>
									<Text size="sm" c="dimmed">
										{isSttTestRunning
											? "Duration: running…"
											: sttTestDurationMs === null
												? "Duration: —"
												: `Duration: ${(sttTestDurationMs / 1000).toFixed(2)}s`}
									</Text>

									<div
										style={{
											display: "flex",
											alignItems: "center",
											gap: 12,
											marginLeft: "auto",
										}}
									>
										<Text size="sm" c="dimmed">
											Test with last created audio (and test audio settings)
										</Text>
										<Tooltip
											label={
												hasLastAudioForSttTest
													? undefined
													: "No previous audio found. Record once (toggle/hold hotkey), then come back and click Test."
											}
											withArrow
											disabled={hasLastAudioForSttTest}
										>
											<span>
												<Button
													color="gray"
													loading={isSttTestRunning}
													disabled={!hasLastAudioForSttTest}
													onClick={onRunSttTest}
												>
													Test
												</Button>
											</span>
										</Tooltip>
									</div>
								</div>

								<div style={{ width: "100%" }}>
									{sttTestError ? (
										<Text size="sm" c="red" style={{ marginBottom: 8 }}>
											{sttTestError}
										</Text>
									) : null}

									{sttPromptSupported && hasStoredTranscriptionPrompt ? (
										<Text size="xs" c="dimmed" style={{ marginBottom: 8 }}>
											Test transcription will include your global transcription
											prompt.
										</Text>
									) : null}

									<Textarea
										value={sttTestOutput}
										readOnly
										placeholder="Transcript will appear here"
										autosize
										minRows={3}
										styles={{
											input: {
												backgroundColor: "var(--bg-elevated)",
												borderColor: "var(--border-default)",
												color: "var(--text-primary)",
												fontFamily: "monospace",
												fontSize: "13px",
											},
										}}
									/>
								</div>
							</div>
						</Accordion.Panel>
					</Accordion.Item>
				</Accordion>
			</div>
		</>
	);
}
