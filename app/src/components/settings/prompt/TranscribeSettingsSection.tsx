import {
	Accordion,
	ActionIcon,
	Button,
	NumberInput,
	Select,
	Switch,
	Text,
	Textarea,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { Info, RotateCcw } from "lucide-react";
import { isRealtimeSttModel } from "../../../lib/modelOptions";
import {
	useSettings,
	useUpdateSTTLiveOutput,
	useUpdateSTTSimulatedStreaming,
} from "../../../lib/queries";

interface TranscribeSettingsSectionProps {
	activeProfileId: string;
	isDefaultScope: boolean;
	inheritTooltip: string;
	sttProviderInheriting: boolean;
	sttModelInheriting: boolean;
	sttLanguageInheriting: boolean;
	sttTimeoutInheriting: boolean;
	effectiveSttProvider: string | null;
	sttProviderOptions: Array<{
		group: string;
		items: Array<{ value: string; label: string }>;
	}>;
	isSttProviderOptionsDisabled: boolean;
	sttProviderIsWhisperServer: boolean;
	sttModelOptions: Array<{ value: string; label: string }>;
	selectedSttModelForUi: string | null;
	sttPricingLabel: string | null;
	sttLanguageOptions: Array<{ value: string; label: string }>;
	localProfileSttLanguage: string;
	whisperServerModelDraft: string;
	onWhisperServerModelDraftChange: (value: string) => void;
	onWhisperServerModelBlur: () => void;
	onSttProviderChange: (value: string | null) => void;
	onSttModelChange: (value: string | null) => void;
	onSttLanguageChange: (value: string | null) => void;
	onDisableSttProviderOverride: () => void;
	onDisableSttModelOverride: () => void;
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
	managedAccessEnabled: boolean;
	managedModelCompatible: boolean;
	useManagedInference: boolean;
	ownKeyConfigured: boolean;
	onUseOwnKeyChange: (useOwnKey: boolean) => void;
}

export function TranscribeSettingsSection({
	activeProfileId,
	isDefaultScope,
	inheritTooltip,
	sttProviderInheriting,
	sttModelInheriting,
	sttLanguageInheriting,
	sttTimeoutInheriting,
	effectiveSttProvider,
	sttProviderOptions,
	isSttProviderOptionsDisabled,
	sttProviderIsWhisperServer,
	sttModelOptions,
	selectedSttModelForUi,
	sttPricingLabel,
	sttLanguageOptions,
	localProfileSttLanguage,
	whisperServerModelDraft,
	onWhisperServerModelDraftChange,
	onWhisperServerModelBlur,
	onSttProviderChange,
	onSttModelChange,
	onSttLanguageChange,
	onDisableSttProviderOverride,
	onDisableSttModelOverride,
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
	managedAccessEnabled,
	managedModelCompatible,
	useManagedInference,
	ownKeyConfigured,
	onUseOwnKeyChange,
}: TranscribeSettingsSectionProps) {
	const { data: globalSettings } = useSettings();
	const updateSTTLiveOutput = useUpdateSTTLiveOutput();
	const updateSTTSimulatedStreaming = useUpdateSTTSimulatedStreaming();

	const isNativeRealtime = isRealtimeSttModel(
		effectiveSttProvider,
		selectedSttModelForUi,
	);

	const showSimulatedStreaming = !isNativeRealtime;
	const simulatedStreamingEnabled =
		globalSettings?.stt_simulated_streaming ?? false;
	const showLiveOutput = isNativeRealtime || simulatedStreamingEnabled;

	return (
		<>
			<div className="settings-mini-header settings-mini-header--first">
				<span className="settings-mini-header__text">Transcribe</span>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Speech-to-Text Provider</p>
					<p className="settings-description">Service for transcribing audio</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && sttProviderInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !sttProviderInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableSttProviderOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						data={sttProviderOptions}
						value={effectiveSttProvider}
						onChange={onSttProviderChange}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={isSttProviderOptionsDisabled}
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

			{sttProviderIsWhisperServer || sttModelOptions.length > 0 ? (
				<div className="settings-row">
					<div>
						<p className="settings-label">STT Model</p>
						<p className="settings-description">
							Model to use for transcription
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && sttModelInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !sttModelInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableSttModelOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						{sttPricingLabel ? (
							<Text
								size="xs"
								c="dimmed"
								style={{ whiteSpace: "nowrap", lineHeight: 1 }}
							>
								{sttPricingLabel}
							</Text>
						) : null}
						{sttProviderIsWhisperServer ? (
							<TextInput
								value={whisperServerModelDraft}
								onChange={(e) =>
									onWhisperServerModelDraftChange(e.currentTarget.value)
								}
								onBlur={onWhisperServerModelBlur}
								placeholder="whisper-1"
								styles={{
									input: {
										backgroundColor: "var(--bg-elevated)",
										borderColor: "var(--border-default)",
										color: "var(--text-primary)",
										minWidth: 200,
									},
								}}
							/>
						) : (
							<div
								style={{
									display: "flex",
									flexDirection: "column",
									alignItems: "flex-end",
									gap: 6,
								}}
							>
								<Select
									data={sttModelOptions}
									value={selectedSttModelForUi}
									onChange={onSttModelChange}
									placeholder="Select model"
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
								{managedAccessEnabled && managedModelCompatible ? (
									<>
										<Switch
											label="Use your own API key"
											checked={!useManagedInference}
											onChange={(event) =>
												onUseOwnKeyChange(event.currentTarget.checked)
											}
											color="gray"
											size="sm"
										/>
										{!useManagedInference && !ownKeyConfigured ? (
											<Text size="xs" c="yellow">
												Add this provider's API key in Providers before use.
											</Text>
										) : null}
									</>
								) : managedAccessEnabled && selectedSttModelForUi ? (
									<Text size="xs" c="yellow">
										This model is not available through Kolboo Managed. It
										requires your own API key.
									</Text>
								) : null}
							</div>
						)}
					</div>
				</div>
			) : null}

			{/* Simulated Streaming — only for non-realtime STT models */}
			{showSimulatedStreaming && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Simulated Streaming (experimental)</p>
						<p className="settings-description">
							Periodically transcribe audio chunks during recording for
							progressive results
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
							Paste each chunk as it arrives instead of waiting until the end
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						<Switch
							checked={globalSettings?.stt_live_output ?? false}
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
