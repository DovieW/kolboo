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
import { Info, RotateCcw } from "lucide-react";
import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import type {
	AppSettings,
	OpenAiReasoningEffort,
	RewriteProgramPromptProfile,
} from "../../../lib/tauri";
import { llmAPI, tauriAPI } from "../../../lib/tauri";
import { HintSelect } from "../../HintSelect";
import { PromptSectionEditor } from "../PromptSectionEditor";

type LlmOption = { value: string; label: string };
type LlmOptionGroup = { group: string; items: LlmOption[] };

type Mutation<Variables> = {
	mutate: (variables: Variables, options?: { onSuccess?: () => void }) => void;
	isPending: boolean;
};

type GeminiThinkingLevel = "minimal" | "low" | "medium" | "high";

export function QuickAskPanel({
	activeProfileId,
	activeProfile,
	isDefaultScope,
	inheritTooltip,
	defaultSystemPrompt,
	selectDefault,
	settings,
	effectiveQuickAskProvider,
	effectiveQuickAskModel,
	quickAskIncludeSelectedText,
	quickAskConversationHistoryEnabled,
	quickAskConversationHistoryCount,
	quickAskIncludeClipboardContextInheriting,
	quickAskProviderInheriting,
	quickAskModelInheriting,
	quickAskOpenAiReasoningEffortInheriting,
	quickAskGeminiThinkingLevelInheriting,
	quickAskGeminiThinkingBudgetInheriting,
	quickAskAnthropicThinkingBudgetInheriting,
	quickAskSystemPromptInheriting,
	defaultQuickAskIncludeClipboardContext,
	localProfileQuickAskIncludeClipboardContext,
	localProfileQuickAskOpenAiReasoningEffort,
	localProfileQuickAskGeminiThinkingLevel,
	localProfileQuickAskGeminiThinkingBudget,
	localProfileQuickAskAnthropicThinkingBudget,
	localQuickAskSystemPrompt,
	quickAskModelOptions,
	selectedQuickAskModelForUi,
	quickAskOpenAiThinkingOptions,
	quickAskGeminiThinkingLevelOptions,
	quickAskGeminiThinkingBudgetOptions,
	quickAskAnthropicThinkingLevelOptionsWithCustom,
	supportsQuickAskOpenAiThinking,
	supportsQuickAskGeminiThinkingLevel,
	supportsQuickAskGeminiThinkingBudget,
	supportsQuickAskAnthropicThinkingBudget,
	quickAskModelForThinking,
	llmProviderOptions,
	llmProviderDisabled,
	updateQuickAskIncludeSelectedText,
	updateQuickAskConversationHistoryEnabled,
	updateQuickAskConversationHistoryCount,
	updateQuickAskOpenAiReasoningEffort,
	updateQuickAskGeminiThinkingLevel,
	updateQuickAskGeminiThinkingBudget,
	updateQuickAskAnthropicThinkingBudget,
	updateQuickAskSystemPrompt,
	setQuickAskIncludeClipboardContextInheriting,
	setQuickAskProviderInheriting,
	setQuickAskModelInheriting,
	setQuickAskOpenAiReasoningEffortInheriting,
	setQuickAskGeminiThinkingLevelInheriting,
	setQuickAskGeminiThinkingBudgetInheriting,
	setQuickAskAnthropicThinkingBudgetInheriting,
	setQuickAskSystemPromptInheriting,
	setLocalProfileQuickAskIncludeClipboardContext,
	setLocalProfileQuickAskProvider,
	setLocalProfileQuickAskModel,
	setLocalProfileQuickAskOpenAiReasoningEffort,
	setLocalProfileQuickAskGeminiThinkingLevel,
	setLocalProfileQuickAskGeminiThinkingBudget,
	setLocalProfileQuickAskAnthropicThinkingBudget,
	setLocalQuickAskSystemPrompt,
	handleDefaultQuickAskProviderChange,
	handleDefaultQuickAskModelChange,
	openDisableOverrideDialog,
	saveProfileMetadata,
	getLlmModelOptionsForProvider,
	isOpenAiReasoningEffort,
	isGeminiThinkingLevel,
	openAiDefaultReasoningEffortForModel,
	formatThinkingBudgetShort,
	isSavingProfile,
	errorToMessage,
	quickAskTestInput,
	quickAskTestOutput,
	quickAskTestError,
	quickAskTestDurationMs,
	quickAskTestPending,
	quickAskTestStartRef,
	setQuickAskTestInput,
	setQuickAskTestOutput,
	setQuickAskTestError,
	setQuickAskTestDurationMs,
	setQuickAskTestPending,
}: {
	activeProfileId: string;
	activeProfile: RewriteProgramPromptProfile | null;
	isDefaultScope: boolean;
	inheritTooltip: string;
	defaultSystemPrompt: string;
	selectDefault: string;
	settings: AppSettings | undefined;
	effectiveQuickAskProvider: string | null;
	effectiveQuickAskModel: string | null;
	quickAskIncludeSelectedText: boolean;
	quickAskConversationHistoryEnabled: boolean;
	quickAskConversationHistoryCount: number;
	quickAskIncludeClipboardContextInheriting: boolean;
	quickAskProviderInheriting: boolean;
	quickAskModelInheriting: boolean;
	quickAskOpenAiReasoningEffortInheriting: boolean;
	quickAskGeminiThinkingLevelInheriting: boolean;
	quickAskGeminiThinkingBudgetInheriting: boolean;
	quickAskAnthropicThinkingBudgetInheriting: boolean;
	quickAskSystemPromptInheriting: boolean;
	defaultQuickAskIncludeClipboardContext: boolean;
	localProfileQuickAskIncludeClipboardContext: boolean;
	localProfileQuickAskOpenAiReasoningEffort: string;
	localProfileQuickAskGeminiThinkingLevel: string;
	localProfileQuickAskGeminiThinkingBudget: string;
	localProfileQuickAskAnthropicThinkingBudget: string;
	localQuickAskSystemPrompt: string;
	quickAskModelOptions: LlmOption[];
	selectedQuickAskModelForUi: string | null;
	quickAskOpenAiThinkingOptions: LlmOption[];
	quickAskGeminiThinkingLevelOptions: LlmOption[];
	quickAskGeminiThinkingBudgetOptions: LlmOption[];
	quickAskAnthropicThinkingLevelOptionsWithCustom: LlmOption[];
	supportsQuickAskOpenAiThinking: boolean;
	supportsQuickAskGeminiThinkingLevel: boolean;
	supportsQuickAskGeminiThinkingBudget: boolean;
	supportsQuickAskAnthropicThinkingBudget: boolean;
	quickAskModelForThinking: string | null;
	llmProviderOptions: LlmOptionGroup[];
	llmProviderDisabled: boolean;
	updateQuickAskIncludeSelectedText: Mutation<boolean>;
	updateQuickAskConversationHistoryEnabled: Mutation<boolean>;
	updateQuickAskConversationHistoryCount: Mutation<number>;
	updateQuickAskOpenAiReasoningEffort: Mutation<OpenAiReasoningEffort | null>;
	updateQuickAskGeminiThinkingLevel: Mutation<GeminiThinkingLevel | null>;
	updateQuickAskGeminiThinkingBudget: Mutation<number | null>;
	updateQuickAskAnthropicThinkingBudget: Mutation<number | null>;
	updateQuickAskSystemPrompt: Mutation<string | null>;
	setQuickAskIncludeClipboardContextInheriting: Dispatch<
		SetStateAction<boolean>
	>;
	setQuickAskProviderInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickAskModelInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickAskOpenAiReasoningEffortInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickAskGeminiThinkingLevelInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickAskGeminiThinkingBudgetInheriting: Dispatch<SetStateAction<boolean>>;
	setQuickAskAnthropicThinkingBudgetInheriting: Dispatch<
		SetStateAction<boolean>
	>;
	setQuickAskSystemPromptInheriting: Dispatch<SetStateAction<boolean>>;
	setLocalProfileQuickAskIncludeClipboardContext: Dispatch<
		SetStateAction<boolean>
	>;
	setLocalProfileQuickAskProvider: Dispatch<SetStateAction<string | null>>;
	setLocalProfileQuickAskModel: Dispatch<SetStateAction<string | null>>;
	setLocalProfileQuickAskOpenAiReasoningEffort: Dispatch<
		SetStateAction<string>
	>;
	setLocalProfileQuickAskGeminiThinkingLevel: Dispatch<SetStateAction<string>>;
	setLocalProfileQuickAskGeminiThinkingBudget: Dispatch<SetStateAction<string>>;
	setLocalProfileQuickAskAnthropicThinkingBudget: Dispatch<
		SetStateAction<string>
	>;
	setLocalQuickAskSystemPrompt: Dispatch<SetStateAction<string>>;
	handleDefaultQuickAskProviderChange: (value: string) => void;
	handleDefaultQuickAskModelChange: (value: string) => void;
	openDisableOverrideDialog: (args: {
		title: string;
		onConfirm: () => void;
	}) => void;
	saveProfileMetadata: (next: Partial<RewriteProgramPromptProfile>) => void;
	getLlmModelOptionsForProvider: (provider: string | null) => LlmOption[];
	isOpenAiReasoningEffort: (value: unknown) => value is OpenAiReasoningEffort;
	isGeminiThinkingLevel: (value: unknown) => value is GeminiThinkingLevel;
	openAiDefaultReasoningEffortForModel: (model: string) => string;
	formatThinkingBudgetShort: (budgetTokens: number) => string;
	isSavingProfile: boolean;
	errorToMessage: (err: unknown) => string;
	quickAskTestInput: string;
	quickAskTestOutput: string;
	quickAskTestError: string;
	quickAskTestDurationMs: number | null;
	quickAskTestPending: boolean;
	quickAskTestStartRef: MutableRefObject<number | null>;
	setQuickAskTestInput: Dispatch<SetStateAction<string>>;
	setQuickAskTestOutput: Dispatch<SetStateAction<string>>;
	setQuickAskTestError: Dispatch<SetStateAction<string>>;
	setQuickAskTestDurationMs: Dispatch<SetStateAction<number | null>>;
	setQuickAskTestPending: Dispatch<SetStateAction<boolean>>;
}) {
	return (
		<>
			<div className="settings-mini-header">
				<span className="settings-mini-header__text">Quick Ask</span>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Include Highlighted Text</p>
					<p className="settings-description">
						When enabled, Kolboo will try to copy your highlighted text and
						include it as optional context during Quick Ask.
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 10 }}>
					{!isDefaultScope && (
						<Tooltip label="Global setting (edit in Default profile)" withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}

					<Switch
						checked={quickAskIncludeSelectedText}
						onChange={(e) => {
							if (!isDefaultScope) return;
							const enabled = e.currentTarget.checked;
							updateQuickAskIncludeSelectedText.mutate(enabled, {
								onSuccess: () => {
									tauriAPI.emitSettingsChanged();
								},
							});
						}}
						color="gray"
						size="md"
						disabled={!isDefaultScope}
					/>
				</div>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Include Clipboard Context</p>
					<p className="settings-description">
						When enabled, Kolboo reads your clipboard text and includes it as
						optional context during Quick Ask.
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && quickAskIncludeClipboardContextInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !quickAskIncludeClipboardContextInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Quick Ask Clipboard Context override?",
										onConfirm: () => {
											setQuickAskIncludeClipboardContextInheriting(true);
											setLocalProfileQuickAskIncludeClipboardContext(
												defaultQuickAskIncludeClipboardContext,
											);
											saveProfileMetadata({
												quick_ask_include_clipboard_context: null,
											});
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Switch
						checked={localProfileQuickAskIncludeClipboardContext}
						onChange={(e) => {
							const enabled = e.currentTarget.checked;
							if (!isDefaultScope) {
								setQuickAskIncludeClipboardContextInheriting(false);
							}
							setLocalProfileQuickAskIncludeClipboardContext(enabled);
							saveProfileMetadata({
								quick_ask_include_clipboard_context: enabled,
							});
						}}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Conversation History</p>
					<p className="settings-description">
						When enabled, Quick Ask will include the last few Quick Ask
						questions and answers as additional context. This is kept in memory
						only (not saved to disk).
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 10 }}>
					{!isDefaultScope && (
						<Tooltip label="Global setting (edit in Default profile)" withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}

					{quickAskConversationHistoryEnabled && (
						<NumberInput
							value={quickAskConversationHistoryCount}
							onChange={(v) => {
								if (!isDefaultScope) return;
								const n = typeof v === "number" && Number.isFinite(v) ? v : 3;
								updateQuickAskConversationHistoryCount.mutate(n, {
									onSuccess: () => {
										tauriAPI.emitSettingsChanged();
									},
								});
							}}
							min={1}
							max={20}
							step={1}
							w={96}
							disabled={!isDefaultScope}
							styles={{
								input: {
									backgroundColor: "var(--bg-elevated)",
									borderColor: "var(--border-default)",
									color: "var(--text-primary)",
									textAlign: "center",
								},
							}}
						/>
					)}

					<Switch
						checked={quickAskConversationHistoryEnabled}
						onChange={(e) => {
							if (!isDefaultScope) return;
							const enabled = e.currentTarget.checked;
							updateQuickAskConversationHistoryEnabled.mutate(enabled, {
								onSuccess: () => {
									tauriAPI.emitSettingsChanged();
									// If enabling and count is missing/invalid, nudge it to default 3.
									if (enabled) {
										updateQuickAskConversationHistoryCount.mutate(
											quickAskConversationHistoryCount,
											{
												onSuccess: () => {
													tauriAPI.emitSettingsChanged();
												},
											},
										);
									}
								},
							});
						}}
						color="gray"
						size="md"
						disabled={!isDefaultScope}
					/>
				</div>
			</div>

			<div className="settings-row">
				<div>
					<p className="settings-label">Quick Ask Provider</p>
					<p className="settings-description">
						AI service used to answer Quick Ask questions
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && quickAskProviderInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !quickAskProviderInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={() =>
									openDisableOverrideDialog({
										title: "Disable Quick Ask Provider override?",
										onConfirm: () => {
											setQuickAskProviderInheriting(true);
											setQuickAskModelInheriting(true);
											setLocalProfileQuickAskProvider(
												settings?.quick_ask_provider ??
													settings?.llm_provider ??
													null,
											);
											setLocalProfileQuickAskModel(
												settings?.quick_ask_model ??
													settings?.llm_model ??
													null,
											);
											saveProfileMetadata({
												quick_ask_provider: null,
												quick_ask_model: null,
											});
										},
									})
								}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						data={llmProviderOptions}
						value={effectiveQuickAskProvider}
						onChange={(value) => {
							if (!value) return;
							if (isDefaultScope) {
								handleDefaultQuickAskProviderChange(value);
								return;
							}

							setQuickAskProviderInheriting(false);
							setQuickAskModelInheriting(false);
							setLocalProfileQuickAskProvider(value);
							const models = getLlmModelOptionsForProvider(value);
							const firstModel = models[0]?.value ?? null;
							setLocalProfileQuickAskModel(firstModel);
							saveProfileMetadata({
								quick_ask_provider: value,
								quick_ask_model: firstModel,
							});
						}}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={llmProviderDisabled}
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

			{quickAskModelOptions.length > 0 ? (
				<div className="settings-row">
					<div>
						<p className="settings-label">Quick Ask Model</p>
						<p className="settings-description">
							LLM model used to answer Quick Ask questions.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && quickAskModelInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !quickAskModelInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Quick Ask Model override?",
											onConfirm: () => {
												setQuickAskModelInheriting(true);
												setLocalProfileQuickAskModel(
													settings?.quick_ask_model ??
														settings?.llm_model ??
														null,
												);
												saveProfileMetadata({ quick_ask_model: null });
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<Select
							data={quickAskModelOptions}
							value={selectedQuickAskModelForUi}
							onChange={(value) => {
								if (!value) return;
								if (isDefaultScope) {
									handleDefaultQuickAskModelChange(value);
									return;
								}

								setQuickAskModelInheriting(false);
								setLocalProfileQuickAskModel(value);
								saveProfileMetadata({ quick_ask_model: value });
							}}
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
					</div>
				</div>
			) : null}

			{supportsQuickAskOpenAiThinking && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Set the reasoning effort for this model.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && quickAskOpenAiReasoningEffortInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !quickAskOpenAiReasoningEffortInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking override?",
											onConfirm: () => {
												setQuickAskOpenAiReasoningEffortInheriting(true);
												setLocalProfileQuickAskOpenAiReasoningEffort(
													selectDefault,
												);
												saveProfileMetadata({
													quick_ask_openai_reasoning_effort: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={quickAskOpenAiThinkingOptions}
							value={
								isDefaultScope
									? (settings?.quick_ask_openai_reasoning_effort ??
										selectDefault)
									: localProfileQuickAskOpenAiReasoningEffort
							}
							onChange={(value) => {
								if (isDefaultScope) {
									if (value == null || value === selectDefault) {
										updateQuickAskOpenAiReasoningEffort.mutate(null, {
											onSuccess: () => {
												tauriAPI.emitSettingsChanged();
											},
										});
										return;
									}

									const effort = isOpenAiReasoningEffort(value) ? value : null;
									if (!effort) return;
									updateQuickAskOpenAiReasoningEffort.mutate(effort, {
										onSuccess: () => {
											tauriAPI.emitSettingsChanged();
										},
									});
									return;
								}

								if (value == null || value === selectDefault) {
									setQuickAskOpenAiReasoningEffortInheriting(true);
									setLocalProfileQuickAskOpenAiReasoningEffort(selectDefault);
									saveProfileMetadata({
										quick_ask_openai_reasoning_effort: null,
									});
									return;
								}

								setQuickAskOpenAiReasoningEffortInheriting(false);
								setLocalProfileQuickAskOpenAiReasoningEffort(value);
								const effort = isOpenAiReasoningEffort(value) ? value : null;
								if (!effort) return;
								saveProfileMetadata({
									quick_ask_openai_reasoning_effort: effort,
								});
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}

								if (option.value !== selectDefault) {
									return <Text size="sm">{option.label}</Text>;
								}

								const modelHint = quickAskModelForThinking
									? openAiDefaultReasoningEffortForModel(
											quickAskModelForThinking,
										)
									: "medium";
								const hint = isDefaultScope
									? modelHint
									: (settings?.quick_ask_openai_reasoning_effort ?? modelHint);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== selectDefault) {
									return <Text size="sm">{option.label}</Text>;
								}

								const modelHint = quickAskModelForThinking
									? openAiDefaultReasoningEffortForModel(
											quickAskModelForThinking,
										)
									: "medium";
								const hint = isDefaultScope
									? modelHint
									: (settings?.quick_ask_openai_reasoning_effort ?? modelHint);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsQuickAskGeminiThinkingLevel && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Level</p>
						<p className="settings-description">
							{quickAskModelForThinking?.includes("gemini-3-pro")
								? "Gemini 3 Pro supports low/high (default high)."
								: "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
						</p>
					</div>

					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && quickAskGeminiThinkingLevelInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}

						{!isDefaultScope && !quickAskGeminiThinkingLevelInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking Level override?",
											onConfirm: () => {
												setQuickAskGeminiThinkingLevelInheriting(true);
												setLocalProfileQuickAskGeminiThinkingLevel(
													selectDefault,
												);
												saveProfileMetadata({
													quick_ask_gemini_thinking_level: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={quickAskGeminiThinkingLevelOptions}
							value={
								isDefaultScope
									? (settings?.quick_ask_gemini_thinking_level ?? selectDefault)
									: localProfileQuickAskGeminiThinkingLevel
							}
							onChange={(value) => {
								const v =
									value === "minimal" ||
									value === "low" ||
									value === "medium" ||
									value === "high"
										? value
										: null;

								if (isDefaultScope) {
									updateQuickAskGeminiThinkingLevel.mutate(v, {
										onSuccess: () => {
											tauriAPI.emitSettingsChanged();
										},
									});
									return;
								}

								if (value == null || value === selectDefault) {
									setQuickAskGeminiThinkingLevelInheriting(true);
									setLocalProfileQuickAskGeminiThinkingLevel(selectDefault);
									saveProfileMetadata({
										quick_ask_gemini_thinking_level: null,
									});
									return;
								}

								if (v == null) return;

								setQuickAskGeminiThinkingLevelInheriting(false);
								setLocalProfileQuickAskGeminiThinkingLevel(v);
								saveProfileMetadata({ quick_ask_gemini_thinking_level: v });
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}
								if (option.value !== selectDefault) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.quick_ask_gemini_thinking_level ?? "high");

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== selectDefault) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.quick_ask_gemini_thinking_level ?? "high");

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsQuickAskGeminiThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Budget</p>
						<p className="settings-description">
							Token budget for Gemini 2.5 thinking.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && quickAskGeminiThinkingBudgetInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !quickAskGeminiThinkingBudgetInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking Budget override?",
											onConfirm: () => {
												setQuickAskGeminiThinkingBudgetInheriting(true);
												setLocalProfileQuickAskGeminiThinkingBudget(
													selectDefault,
												);
												saveProfileMetadata({
													quick_ask_gemini_thinking_budget: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={quickAskGeminiThinkingBudgetOptions}
							value={
								isDefaultScope
									? settings?.quick_ask_gemini_thinking_budget == null
										? selectDefault
										: String(settings.quick_ask_gemini_thinking_budget)
									: localProfileQuickAskGeminiThinkingBudget
							}
							onChange={(value) => {
								if (isDefaultScope) {
									if (value == null || value === selectDefault) {
										updateQuickAskGeminiThinkingBudget.mutate(null, {
											onSuccess: () => {
												tauriAPI.emitSettingsChanged();
											},
										});
										return;
									}

									const parsed = Number(value);
									if (!Number.isFinite(parsed)) return;
									updateQuickAskGeminiThinkingBudget.mutate(
										Math.trunc(parsed),
										{
											onSuccess: () => {
												tauriAPI.emitSettingsChanged();
											},
										},
									);
									return;
								}

								if (value == null || value === selectDefault) {
									setQuickAskGeminiThinkingBudgetInheriting(true);
									setLocalProfileQuickAskGeminiThinkingBudget(selectDefault);
									saveProfileMetadata({
										quick_ask_gemini_thinking_budget: null,
									});
									return;
								}

								const parsed = Number(value);
								if (!Number.isFinite(parsed)) return;
								const asInt = Math.trunc(parsed);
								setQuickAskGeminiThinkingBudgetInheriting(false);
								setLocalProfileQuickAskGeminiThinkingBudget(String(asInt));
								saveProfileMetadata({
									quick_ask_gemini_thinking_budget: asInt,
								});
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}
								if (option.value !== selectDefault)
									return <Text size="sm">{option.label}</Text>;

								const inherited = settings?.quick_ask_gemini_thinking_budget;
								const hint = isDefaultScope
									? "dynamic"
									: inherited == null
										? "dynamic"
										: inherited === 0
											? "off"
											: inherited === -1
												? "dynamic"
												: String(inherited);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
							renderOption={({ option }) => {
								if (option.value !== selectDefault) {
									return <Text size="sm">{option.label}</Text>;
								}

								const inherited = settings?.quick_ask_gemini_thinking_budget;
								const hint = isDefaultScope
									? "dynamic"
									: inherited == null
										? "dynamic"
										: inherited === 0
											? "off"
											: inherited === -1
												? "dynamic"
												: String(inherited);

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<span style={{ fontSize: 14 }}>{option.label}</span>
										<span
											style={{
												fontSize: 11,
												color: "var(--text-muted)",
												opacity: 0.9,
												lineHeight: 1,
											}}
										>
											· {hint}
										</span>
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			{supportsQuickAskAnthropicThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Extended thinking level for Claude models.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && quickAskAnthropicThinkingBudgetInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !quickAskAnthropicThinkingBudgetInheriting && (
							<Tooltip
								label="Disable override (inherit from Default)"
								withArrow
							>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={() =>
										openDisableOverrideDialog({
											title: "Disable Thinking override?",
											onConfirm: () => {
												setQuickAskAnthropicThinkingBudgetInheriting(true);
												setLocalProfileQuickAskAnthropicThinkingBudget(
													selectDefault,
												);
												saveProfileMetadata({
													quick_ask_anthropic_thinking_budget: null,
												});
											},
										})
									}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={quickAskAnthropicThinkingLevelOptionsWithCustom}
							value={
								isDefaultScope
									? settings?.quick_ask_anthropic_thinking_budget == null
										? selectDefault
										: String(settings.quick_ask_anthropic_thinking_budget)
									: localProfileQuickAskAnthropicThinkingBudget
							}
							onChange={(value) => {
								if (isDefaultScope) {
									if (value == null || value === selectDefault) {
										updateQuickAskAnthropicThinkingBudget.mutate(null, {
											onSuccess: () => {
												tauriAPI.emitSettingsChanged();
											},
										});
										return;
									}

									const parsed = Number(value);
									if (!Number.isFinite(parsed)) return;
									updateQuickAskAnthropicThinkingBudget.mutate(
										Math.trunc(parsed),
										{
											onSuccess: () => {
												tauriAPI.emitSettingsChanged();
											},
										},
									);
									return;
								}

								if (value == null || value === selectDefault) {
									setQuickAskAnthropicThinkingBudgetInheriting(true);
									setLocalProfileQuickAskAnthropicThinkingBudget(selectDefault);
									saveProfileMetadata({
										quick_ask_anthropic_thinking_budget: null,
									});
									return;
								}

								const parsed = Number(value);
								if (!Number.isFinite(parsed)) return;
								const asInt = Math.trunc(parsed);
								setQuickAskAnthropicThinkingBudgetInheriting(false);
								setLocalProfileQuickAskAnthropicThinkingBudget(String(asInt));
								saveProfileMetadata({
									quick_ask_anthropic_thinking_budget: asInt,
								});
							}}
							placeholder="Default"
							inputStyle={{
								backgroundColor: "var(--bg-elevated)",
								borderColor: "var(--border-default)",
								color: "var(--text-primary)",
								minWidth: 200,
							}}
							renderSelected={({ option, placeholder }) => {
								if (!option) {
									return (
										<Text size="sm" c="dimmed">
											{placeholder}
										</Text>
									);
								}

								if (option.value === selectDefault) {
									const inheritedBudget =
										settings?.quick_ask_anthropic_thinking_budget;
									const hint = isDefaultScope
										? "off"
										: inheritedBudget == null
											? "off"
											: formatThinkingBudgetShort(inheritedBudget);

									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<span style={{ fontSize: 14 }}>{option.label}</span>
											<span
												style={{
													fontSize: 11,
													color: "var(--text-muted)",
													opacity: 0.9,
													lineHeight: 1,
												}}
											>
												· {hint}
											</span>
										</div>
									);
								}

								if (option.label.startsWith("Custom")) {
									const n = Number(option.value);
									const suffix = Number.isFinite(n)
										? formatThinkingBudgetShort(n)
										: null;
									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<Text size="sm">{option.label}</Text>
											{suffix && (
												<Text size="xs" c="dimmed" style={{ lineHeight: 1 }}>
													{suffix}
												</Text>
											)}
										</div>
									);
								}

								return <Text size="sm">{option.label}</Text>;
							}}
							renderOption={({ option }) => {
								if (option.value === selectDefault) {
									const inheritedBudget =
										settings?.quick_ask_anthropic_thinking_budget;
									const hint = isDefaultScope
										? "off"
										: inheritedBudget == null
											? "off"
											: formatThinkingBudgetShort(inheritedBudget);

									return (
										<div
											style={{
												display: "flex",
												alignItems: "baseline",
												gap: 8,
											}}
										>
											<span style={{ fontSize: 14 }}>{option.label}</span>
											<span
												style={{
													fontSize: 11,
													color: "var(--text-muted)",
													opacity: 0.9,
													lineHeight: 1,
												}}
											>
												· {hint}
											</span>
										</div>
									);
								}

								const n = Number(option.value);
								const suffix = Number.isFinite(n)
									? formatThinkingBudgetShort(n)
									: null;

								return (
									<div
										style={{
											display: "flex",
											alignItems: "baseline",
											gap: 8,
										}}
									>
										<Text size="sm">{option.label}</Text>
										{suffix && (
											<Text size="xs" c="dimmed" style={{ lineHeight: 1 }}>
												{suffix}
											</Text>
										)}
									</div>
								);
							}}
						/>
					</div>
				</div>
			)}

			<div
				className="settings-accordion-block"
				style={{ marginTop: 0, marginBottom: 16 }}
			>
				<Accordion variant="separated" radius="md">
					<PromptSectionEditor
						sectionKey={`${activeProfileId}-quick-ask-system-prompt`}
						title="System Prompt"
						description="Optional instructions that apply to all Quick Ask answers."
						enabled={true}
						hideToggle={true}
						placeholder="(leave empty to disable)"
						initialContent={localQuickAskSystemPrompt}
						defaultContent={
							isDefaultScope
								? defaultSystemPrompt
								: (settings?.quick_ask_system_prompt ?? "")
						}
						hasCustom={
							isDefaultScope
								? (() => {
										// While settings are loading, avoid flickering the reset button.
										if (settings?.quick_ask_system_prompt === undefined)
											return false;

										// `null` means explicitly disabled, which is a deviation from the default.
										if (settings?.quick_ask_system_prompt === null) return true;

										// Otherwise, enable reset only if the stored value differs from the default.
										return (
											settings?.quick_ask_system_prompt !== defaultSystemPrompt
										);
									})()
								: activeProfile?.quick_ask_system_prompt !== null &&
									activeProfile?.quick_ask_system_prompt !== undefined
						}
						inheritMode={
							isDefaultScope
								? null
								: quickAskSystemPromptInheriting
									? "inheriting"
									: "overriding"
						}
						inheritTooltip={inheritTooltip}
						disableOverrideTooltip="Disable override (inherit from Default)"
						onDisableOverride={
							isDefaultScope
								? undefined
								: () =>
										openDisableOverrideDialog({
											title: "Disable Quick Ask System Prompt override?",
											onConfirm: () => {
												setQuickAskSystemPromptInheriting(true);
												setLocalQuickAskSystemPrompt(
													settings?.quick_ask_system_prompt ?? "",
												);
												saveProfileMetadata({ quick_ask_system_prompt: null });
											},
										})
						}
						onToggle={() => {}}
						onSave={(content) => {
							if (isDefaultScope) {
								const normalized = content.trim();
								const toStore: string | null =
									normalized.length > 0 ? content : null;

								// Keep the outer state in sync so the editor doesn't snap back.
								setLocalQuickAskSystemPrompt(content);

								updateQuickAskSystemPrompt.mutate(toStore, {
									onSuccess: () => {
										tauriAPI.emitSettingsChanged();
									},
								});
								return;
							}

							const base = settings?.quick_ask_system_prompt ?? "";

							// If the user saves exactly the inherited value, treat it as inheriting.
							const toStore = content === base ? null : content;
							const nextLocal = toStore == null ? base : content;

							setLocalQuickAskSystemPrompt(nextLocal);
							setQuickAskSystemPromptInheriting(toStore == null);
							saveProfileMetadata({ quick_ask_system_prompt: toStore });
						}}
						onReset={() => {
							if (isDefaultScope) {
								setLocalQuickAskSystemPrompt(defaultSystemPrompt);
								updateQuickAskSystemPrompt.mutate(defaultSystemPrompt, {
									onSuccess: () => {
										tauriAPI.emitSettingsChanged();
									},
								});
								return;
							}

							const base = settings?.quick_ask_system_prompt ?? "";
							setLocalQuickAskSystemPrompt(base);
							setQuickAskSystemPromptInheriting(true);
							saveProfileMetadata({ quick_ask_system_prompt: null });
						}}
						isSaving={
							isDefaultScope
								? updateQuickAskSystemPrompt.isPending
								: isSavingProfile
						}
					/>

					<Accordion.Item value={`${activeProfileId}-quick-ask-test`}>
						<Accordion.Control>
							<div>
								<p className="settings-label">Test Quick Ask</p>
								<p className="settings-description">
									Ask a question and preview the answer using the Quick Ask
									settings above.
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
										{quickAskTestPending
											? "Duration: running…"
											: quickAskTestDurationMs === null
												? "Duration: —"
												: `Duration: ${(quickAskTestDurationMs / 1000).toFixed(
														2,
													)}s`}
									</Text>

									<Button
										color="gray"
										loading={quickAskTestPending}
										disabled={
											!effectiveQuickAskProvider || !quickAskTestInput.trim()
										}
										onClick={async () => {
											if (!effectiveQuickAskProvider) return;
											if (!quickAskTestInput.trim()) return;

											setQuickAskTestError("");
											setQuickAskTestOutput("");
											setQuickAskTestDurationMs(null);
											quickAskTestStartRef.current = performance.now();
											setQuickAskTestPending(true);

											const openAiReasoningEffort = isDefaultScope
												? (settings?.quick_ask_openai_reasoning_effort ?? null)
												: localProfileQuickAskOpenAiReasoningEffort ===
														selectDefault
													? null
													: isOpenAiReasoningEffort(
																localProfileQuickAskOpenAiReasoningEffort,
															)
														? localProfileQuickAskOpenAiReasoningEffort
														: null;

											const geminiThinkingLevel = isDefaultScope
												? (settings?.quick_ask_gemini_thinking_level ?? null)
												: localProfileQuickAskGeminiThinkingLevel ===
														selectDefault
													? null
													: isGeminiThinkingLevel(
																localProfileQuickAskGeminiThinkingLevel,
															)
														? localProfileQuickAskGeminiThinkingLevel
														: null;

											const geminiThinkingBudget = (() => {
												if (isDefaultScope) {
													return (
														settings?.quick_ask_gemini_thinking_budget ?? null
													);
												}
												if (
													localProfileQuickAskGeminiThinkingBudget ===
													selectDefault
												)
													return null;
												const n = Number(
													localProfileQuickAskGeminiThinkingBudget,
												);
												return Number.isFinite(n) ? Math.trunc(n) : null;
											})();

											const anthropicThinkingBudget = (() => {
												if (isDefaultScope) {
													return (
														settings?.quick_ask_anthropic_thinking_budget ??
														null
													);
												}
												if (
													localProfileQuickAskAnthropicThinkingBudget ===
													selectDefault
												)
													return null;
												const n = Number(
													localProfileQuickAskAnthropicThinkingBudget,
												);
												return Number.isFinite(n) ? Math.trunc(n) : null;
											})();

											try {
												const res = await llmAPI.complete({
													provider: effectiveQuickAskProvider,
													model:
														selectedQuickAskModelForUi ??
														effectiveQuickAskModel,
													systemPrompt: localQuickAskSystemPrompt,
													userPrompt: quickAskTestInput,
													openAiReasoningEffort,
													geminiThinkingBudget,
													geminiThinkingLevel,
													anthropicThinkingBudget,
												});

												setQuickAskTestOutput(
													`[${res.provider_used}/${res.model_used}]\n\n${res.output}`,
												);
											} catch (err) {
												setQuickAskTestError(errorToMessage(err));
											} finally {
												const startedAt = quickAskTestStartRef.current;
												quickAskTestStartRef.current = null;
												if (typeof startedAt === "number") {
													setQuickAskTestDurationMs(
														performance.now() - startedAt,
													);
												}
												setQuickAskTestPending(false);
											}
										}}
									>
										Test
									</Button>
								</div>

								<div style={{ width: "100%" }}>
									<Textarea
										value={quickAskTestInput}
										onChange={(e) =>
											setQuickAskTestInput(e.currentTarget.value)
										}
										placeholder="Ask a question…"
										autosize
										minRows={2}
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

								<div style={{ width: "100%" }}>
									{quickAskTestError ? (
										<Text size="sm" c="red" style={{ marginBottom: 8 }}>
											{quickAskTestError}
										</Text>
									) : null}

									<Textarea
										value={quickAskTestOutput}
										readOnly
										placeholder="Answer will appear here"
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
