import { Select, Switch, Text } from "@mantine/core";
import type { ModelOption } from "../../../lib/modelOptions";
import type { AppSettings } from "../../../lib/tauri";
import { HintSelect } from "../../HintSelect";
import { HintSelectWithDefaultHint } from "../../HintSelectWithDefaultHint";
import { SettingsHintSelectRow } from "../SettingsHintSelectRow";
import { SettingsInheritanceIndicator } from "../SettingsInheritance";
import { SettingsRow } from "../SettingsRow";

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
			<SettingsRow
				label="Rewrite Transcription"
				description="Enable or disable rewriting the transcription with an LLM"
				right={
					<>
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={rewriteEnabledInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={onDisableRewriteEnabledOverride}
							disabled={isUpdatingRewriteEnabled}
						/>
						<Switch
							checked={
								isDefaultScope
									? defaultRewriteEnabled
									: localProfileRewriteEnabled
							}
							onChange={(e) =>
								onRewriteEnabledChange(e.currentTarget.checked)
							}
							disabled={isUpdatingRewriteEnabled}
							color="gray"
							size="md"
						/>
					</>
				}
			/>

			{/* Include Clipboard Context toggle */}
			<SettingsRow
				label="Include Clipboard Context"
				description={
					<>
						When enabled, Kolboo reads your clipboard text and includes it as
						optional context during the Rewrite step.
					</>
				}
				right={
					<>
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={rewriteIncludeClipboardContextInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={
								onDisableRewriteIncludeClipboardContextOverride
							}
						/>
						<Switch
							checked={localProfileRewriteIncludeClipboardContext}
							onChange={(e) =>
								onRewriteIncludeClipboardContextChange(e.currentTarget.checked)
							}
							color="gray"
							size="md"
						/>
					</>
				}
			/>

			{/* LLM Provider */}
			<SettingsRow
				label="Language Model Provider"
				description="AI service for text formatting"
				right={
					<>
						<SettingsInheritanceIndicator
							isDefaultScope={isDefaultScope}
							inheriting={llmProviderInheriting}
							inheritTooltip={inheritTooltip}
							onDisableOverride={onDisableLlmProviderOverride}
						/>
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
					</>
				}
			/>

			{/* LLM Model */}
			{llmModelOptions.length > 0 ? (
				<SettingsRow
					label="Rewrite LLM Model"
					description="LLM Model used to rewrite the transcription."
					right={
						<>
							<SettingsInheritanceIndicator
								isDefaultScope={isDefaultScope}
								inheriting={llmModelInheriting}
								inheritTooltip={inheritTooltip}
								onDisableOverride={onDisableLlmModelOverride}
							/>
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
										? (settings?.llm_model ??
												llmModelOptions[0]?.value ??
												null)
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
						</>
					}
				/>
			) : null}

			{/* OpenAI Thinking */}
			{supportsOpenAiThinking && (
				<SettingsHintSelectRow
					label="Thinking"
					description="Set the reasoning effort for this model."
					isDefaultScope={isDefaultScope}
					inheriting={openAiReasoningEffortInheriting}
					inheritTooltip={inheritTooltip}
					onDisableOverride={onDisableOpenAiThinkingOverride}
				>
					<HintSelectWithDefaultHint
						data={openAiThinkingOptions}
						value={
							isDefaultScope
								? (settings?.openai_reasoning_effort ?? SELECT_DEFAULT)
								: localProfileOpenAiReasoningEffort
						}
						onChange={onOpenAiThinkingChange}
						placeholder="Default"
						defaultValue={SELECT_DEFAULT}
						defaultHint={
							isDefaultScope
								? effectiveLlmModel
									? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
									: "medium"
								: (settings?.openai_reasoning_effort ??
									(effectiveLlmModel
										? openAiDefaultReasoningEffortForModel(effectiveLlmModel)
										: "medium"))
						}
						inputStyle={{
							backgroundColor: "var(--bg-elevated)",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
							minWidth: 200,
						}}
					/>
				</SettingsHintSelectRow>
			)}

			{/* Gemini Thinking Level */}
			{supportsGeminiThinkingLevel && (
				<SettingsHintSelectRow
					label="Thinking Level"
					description={
						isGemini3Pro
							? "Gemini 3 Pro supports low/high (default high)."
							: "Gemini 3 Flash supports minimal/low/medium/high (default high)."
					}
					isDefaultScope={isDefaultScope}
					inheriting={geminiThinkingLevelInheriting}
					inheritTooltip={inheritTooltip}
					onDisableOverride={onDisableGeminiThinkingLevelOverride}
				>
					<HintSelectWithDefaultHint
						data={geminiThinkingLevelOptions}
						value={
							isDefaultScope
								? (settings?.gemini_thinking_level ?? SELECT_DEFAULT)
								: localProfileGeminiThinkingLevel
						}
						onChange={onGeminiThinkingLevelChange}
						placeholder="Default"
						defaultValue={SELECT_DEFAULT}
						defaultHint={
							isDefaultScope ? "high" : (settings?.gemini_thinking_level ?? "high")
						}
						inputStyle={{
							backgroundColor: "var(--bg-elevated)",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
							minWidth: 200,
						}}
					/>
				</SettingsHintSelectRow>
			)}

			{/* Gemini Thinking Budget */}
			{supportsGeminiThinkingBudget && (
				<SettingsHintSelectRow
					label="Thinking Budget"
					description="Token budget for Gemini 2.5 thinking."
					isDefaultScope={isDefaultScope}
					inheriting={geminiThinkingBudgetInheriting}
					inheritTooltip={inheritTooltip}
					onDisableOverride={onDisableGeminiThinkingBudgetOverride}
				>
					<HintSelectWithDefaultHint
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
						defaultValue={SELECT_DEFAULT}
						defaultHint={(() => {
							const inherited = settings?.gemini_thinking_budget;
							if (isDefaultScope) return "dynamic";

							if (inherited == null) return "dynamic";
							if (inherited === 0) return "off";
							if (inherited === -1) return "dynamic";
							return String(inherited);
						})()}
						inputStyle={{
							backgroundColor: "var(--bg-elevated)",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
							minWidth: 200,
						}}
					/>
				</SettingsHintSelectRow>
			)}

			{/* Anthropic Thinking Budget */}
			{supportsAnthropicThinkingBudget && (
				<SettingsHintSelectRow
					label="Thinking"
					description="Extended thinking level for Claude models."
					isDefaultScope={isDefaultScope}
					inheriting={anthropicThinkingBudgetInheriting}
					inheritTooltip={inheritTooltip}
					onDisableOverride={onDisableAnthropicThinkingBudgetOverride}
				>
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
				</SettingsHintSelectRow>
			)}
		</>
	);
}
