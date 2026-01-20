import {
	ActionIcon,
	Select,
	Switch,
	Text,
	Tooltip,
} from "@mantine/core";
import { Info, RotateCcw } from "lucide-react";
import type { ModelOption } from "../../../lib/modelOptions";
import type { AppSettings } from "../../../lib/tauri";
import { HintSelect } from "../../HintSelect";

const SELECT_DEFAULT = "default";

type RewriteSettingsSectionProps = {
	isDefaultScope: boolean;
	inheritTooltip: string;
	// Rewrite enabled
	defaultRewriteEnabled: boolean;
	localProfileRewriteEnabled: boolean;
	rewriteEnabledInheriting: boolean;
	onRewriteEnabledChange: (enabled: boolean) => void;
	onDisableRewriteEnabledOverride: () => void;
	isUpdatingRewriteEnabled: boolean;
	// Include clipboard context
	localProfileRewriteIncludeClipboardContext: boolean;
	rewriteIncludeClipboardContextInheriting: boolean;
	onRewriteIncludeClipboardContextChange: (enabled: boolean) => void;
	onDisableRewriteIncludeClipboardContextOverride: () => void;
	// LLM provider
	effectiveLlmProvider: string | null;
	llmProviderOptions: Array<{
		group: string;
		items: Array<{ value: string; label: string }>;
	}>;
	isLlmProviderDisabled: boolean;
	llmProviderInheriting: boolean;
	onLlmProviderChange: (value: string | null) => void;
	onDisableLlmProviderOverride: () => void;
	// LLM model
	llmModelOptions: ModelOption[];
	llmModelInheriting: boolean;
	localProfileLlmModel: string | null;
	llmPricingLabel: string | null;
	settings: AppSettings | undefined;
	onLlmModelChange: (value: string | null) => void;
	onDisableLlmModelOverride: () => void;
	// OpenAI thinking
	supportsOpenAiThinking: boolean;
	openAiReasoningEffortInheriting: boolean;
	localProfileOpenAiReasoningEffort: string;
	openAiThinkingOptions: Array<{ value: string; label: string }>;
	effectiveLlmModel: string | null;
	onOpenAiThinkingChange: (value: string | null) => void;
	onDisableOpenAiThinkingOverride: () => void;
	openAiDefaultReasoningEffortForModel: (model: string) => string;
	// Gemini thinking level
	supportsGeminiThinkingLevel: boolean;
	isGemini3Pro: boolean;
	geminiThinkingLevelInheriting: boolean;
	localProfileGeminiThinkingLevel: string;
	geminiThinkingLevelOptions: Array<{ value: string; label: string }>;
	onGeminiThinkingLevelChange: (value: string | null) => void;
	onDisableGeminiThinkingLevelOverride: () => void;
	// Gemini thinking budget
	supportsGeminiThinkingBudget: boolean;
	geminiThinkingBudgetInheriting: boolean;
	localProfileGeminiThinkingBudget: string;
	geminiThinkingBudgetOptions: Array<{ value: string; label: string }>;
	onGeminiThinkingBudgetChange: (value: string | null) => void;
	onDisableGeminiThinkingBudgetOverride: () => void;
	// Anthropic thinking
	supportsAnthropicThinkingBudget: boolean;
	anthropicThinkingBudgetInheriting: boolean;
	localProfileAnthropicThinkingBudget: string;
	anthropicThinkingLevelOptionsWithCustom: Array<{
		value: string;
		label: string;
	}>;
	onAnthropicThinkingBudgetChange: (value: string | null) => void;
	onDisableAnthropicThinkingBudgetOverride: () => void;
	formatThinkingBudgetShort: (budget: number) => string;
};

export function RewriteSettingsSection({
	isDefaultScope,
	inheritTooltip,
	// Rewrite enabled
	defaultRewriteEnabled,
	localProfileRewriteEnabled,
	rewriteEnabledInheriting,
	onRewriteEnabledChange,
	onDisableRewriteEnabledOverride,
	isUpdatingRewriteEnabled,
	// Include clipboard context
	localProfileRewriteIncludeClipboardContext,
	rewriteIncludeClipboardContextInheriting,
	onRewriteIncludeClipboardContextChange,
	onDisableRewriteIncludeClipboardContextOverride,
	// LLM provider
	effectiveLlmProvider,
	llmProviderOptions,
	isLlmProviderDisabled,
	llmProviderInheriting,
	onLlmProviderChange,
	onDisableLlmProviderOverride,
	// LLM model
	llmModelOptions,
	llmModelInheriting,
	localProfileLlmModel,
	llmPricingLabel,
	settings,
	onLlmModelChange,
	onDisableLlmModelOverride,
	// OpenAI thinking
	supportsOpenAiThinking,
	openAiReasoningEffortInheriting,
	localProfileOpenAiReasoningEffort,
	openAiThinkingOptions,
	effectiveLlmModel,
	onOpenAiThinkingChange,
	onDisableOpenAiThinkingOverride,
	openAiDefaultReasoningEffortForModel,
	// Gemini thinking level
	supportsGeminiThinkingLevel,
	isGemini3Pro,
	geminiThinkingLevelInheriting,
	localProfileGeminiThinkingLevel,
	geminiThinkingLevelOptions,
	onGeminiThinkingLevelChange,
	onDisableGeminiThinkingLevelOverride,
	// Gemini thinking budget
	supportsGeminiThinkingBudget,
	geminiThinkingBudgetInheriting,
	localProfileGeminiThinkingBudget,
	geminiThinkingBudgetOptions,
	onGeminiThinkingBudgetChange,
	onDisableGeminiThinkingBudgetOverride,
	// Anthropic thinking
	supportsAnthropicThinkingBudget,
	anthropicThinkingBudgetInheriting,
	localProfileAnthropicThinkingBudget,
	anthropicThinkingLevelOptionsWithCustom,
	onAnthropicThinkingBudgetChange,
	onDisableAnthropicThinkingBudgetOverride,
	formatThinkingBudgetShort,
}: RewriteSettingsSectionProps) {
	return (
		<>
			<div className="settings-mini-header">
				<span className="settings-mini-header__text">Rewrite</span>
			</div>

			{/* Rewrite Transcription toggle */}
			<div className="settings-row">
				<div>
					<p className="settings-label">Rewrite Transcription</p>
					<p className="settings-description">
						Enable or disable rewriting the transcription with an LLM
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && rewriteEnabledInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !rewriteEnabledInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableRewriteEnabledOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Switch
						checked={
							isDefaultScope ? defaultRewriteEnabled : localProfileRewriteEnabled
						}
						onChange={(e) => onRewriteEnabledChange(e.currentTarget.checked)}
						disabled={isUpdatingRewriteEnabled}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			{/* Include Clipboard Context toggle */}
			<div className="settings-row">
				<div>
					<p className="settings-label">Include Clipboard Context</p>
					<p className="settings-description">
						When enabled, Kolboo reads your clipboard text and includes it as
						optional context during the Rewrite step.
					</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && rewriteIncludeClipboardContextInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !rewriteIncludeClipboardContextInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableRewriteIncludeClipboardContextOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Switch
						checked={localProfileRewriteIncludeClipboardContext}
						onChange={(e) =>
							onRewriteIncludeClipboardContextChange(e.currentTarget.checked)
						}
						color="gray"
						size="md"
					/>
				</div>
			</div>

			{/* LLM Provider */}
			<div className="settings-row">
				<div>
					<p className="settings-label">Language Model Provider</p>
					<p className="settings-description">AI service for text formatting</p>
				</div>
				<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
					{!isDefaultScope && llmProviderInheriting && (
						<Tooltip label={inheritTooltip} withArrow>
							<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
						</Tooltip>
					)}
					{!isDefaultScope && !llmProviderInheriting && (
						<Tooltip label="Disable override (inherit from Default)" withArrow>
							<ActionIcon
								variant="subtle"
								color="gray"
								size="sm"
								onClick={onDisableLlmProviderOverride}
							>
								<RotateCcw size={14} style={{ opacity: 0.65 }} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						data={llmProviderOptions}
						value={effectiveLlmProvider}
						onChange={onLlmProviderChange}
						placeholder="Select provider"
						withCheckIcon={false}
						disabled={isLlmProviderDisabled}
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

			{/* LLM Model */}
			{llmModelOptions.length > 0 ? (
				<div className="settings-row">
					<div>
						<p className="settings-label">Rewrite LLM Model</p>
						<p className="settings-description">
							LLM Model used to rewrite the transcription.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && llmModelInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !llmModelInheriting && (
							<Tooltip label="Disable override (inherit from Default)" withArrow>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableLlmModelOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						{llmPricingLabel ? (
							<Text
								size="xs"
								c="dimmed"
								style={{ whiteSpace: "nowrap", lineHeight: 1 }}
							>
								{llmPricingLabel}
							</Text>
						) : null}
						<Select
							data={llmModelOptions}
							value={
								isDefaultScope
									? (settings?.llm_model ?? llmModelOptions[0]?.value ?? null)
									: localProfileLlmModel
							}
							onChange={onLlmModelChange}
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

			{/* OpenAI Thinking */}
			{supportsOpenAiThinking && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Set the reasoning effort for this model.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && openAiReasoningEffortInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !openAiReasoningEffortInheriting && (
							<Tooltip label="Disable override (inherit from Default)" withArrow>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableOpenAiThinkingOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={openAiThinkingOptions}
							value={
								isDefaultScope
									? (settings?.openai_reasoning_effort ?? SELECT_DEFAULT)
									: localProfileOpenAiReasoningEffort
							}
							onChange={onOpenAiThinkingChange}
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

								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? effectiveLlmModel
										? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
										: "medium"
									: (settings?.openai_reasoning_effort ??
											(effectiveLlmModel
												? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
												: "medium"));

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
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? effectiveLlmModel
										? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
										: "medium"
									: (settings?.openai_reasoning_effort ??
											(effectiveLlmModel
												? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
												: "medium"));

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

			{/* Gemini Thinking Level */}
			{supportsGeminiThinkingLevel && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Level</p>
						<p className="settings-description">
							{isGemini3Pro
								? "Gemini 3 Pro supports low/high (default high)."
								: "Gemini 3 Flash supports minimal/low/medium/high (default high)."}
						</p>
					</div>

					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && geminiThinkingLevelInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}

						{!isDefaultScope && !geminiThinkingLevelInheriting && (
							<Tooltip label="Disable override (inherit from Default)" withArrow>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableGeminiThinkingLevelOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}

						<HintSelect
							data={geminiThinkingLevelOptions}
							value={
								isDefaultScope
									? (settings?.gemini_thinking_level ?? SELECT_DEFAULT)
									: localProfileGeminiThinkingLevel
							}
							onChange={onGeminiThinkingLevelChange}
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
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.gemini_thinking_level ?? "high");

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
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const hint = isDefaultScope
									? "high"
									: (settings?.gemini_thinking_level ?? "high");

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

			{/* Gemini Thinking Budget */}
			{supportsGeminiThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking Budget</p>
						<p className="settings-description">
							Token budget for Gemini 2.5 thinking.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && geminiThinkingBudgetInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !geminiThinkingBudgetInheriting && (
							<Tooltip label="Disable override (inherit from Default)" withArrow>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableGeminiThinkingBudgetOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={geminiThinkingBudgetOptions}
							value={
								isDefaultScope
									? settings?.gemini_thinking_budget == null
										? SELECT_DEFAULT
										: String(settings.gemini_thinking_budget)
									: localProfileGeminiThinkingBudget
							}
							onChange={onGeminiThinkingBudgetChange}
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
								if (option.value !== SELECT_DEFAULT)
									return <Text size="sm">{option.label}</Text>;

								const inherited = settings?.gemini_thinking_budget;
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
								if (option.value !== SELECT_DEFAULT) {
									return <Text size="sm">{option.label}</Text>;
								}

								const inherited = settings?.gemini_thinking_budget;
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

			{/* Anthropic Thinking Budget */}
			{supportsAnthropicThinkingBudget && (
				<div className="settings-row">
					<div>
						<p className="settings-label">Thinking</p>
						<p className="settings-description">
							Extended thinking level for Claude models.
						</p>
					</div>
					<div style={{ display: "flex", alignItems: "center", gap: 8 }}>
						{!isDefaultScope && anthropicThinkingBudgetInheriting && (
							<Tooltip label={inheritTooltip} withArrow>
								<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
							</Tooltip>
						)}
						{!isDefaultScope && !anthropicThinkingBudgetInheriting && (
							<Tooltip label="Disable override (inherit from Default)" withArrow>
								<ActionIcon
									variant="subtle"
									color="gray"
									size="sm"
									onClick={onDisableAnthropicThinkingBudgetOverride}
								>
									<RotateCcw size={14} style={{ opacity: 0.65 }} />
								</ActionIcon>
							</Tooltip>
						)}
						<HintSelect
							data={anthropicThinkingLevelOptionsWithCustom}
							value={
								isDefaultScope
									? settings?.anthropic_thinking_budget == null
										? SELECT_DEFAULT
										: String(settings.anthropic_thinking_budget)
									: localProfileAnthropicThinkingBudget
							}
							onChange={onAnthropicThinkingBudgetChange}
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

								if (option.value === SELECT_DEFAULT) {
									const inheritedBudget = settings?.anthropic_thinking_budget;
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

								// Closed state: keep it simple (label only) unless it's a custom token budget.
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
								if (option.value === SELECT_DEFAULT) {
									const inheritedBudget = settings?.anthropic_thinking_budget;
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
		</>
	);
}
