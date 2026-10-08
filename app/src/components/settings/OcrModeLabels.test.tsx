// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { QuickAskPanel } from "./prompt/QuickAskPanel";
import { RewriteSettingsSection } from "./prompt/RewriteSettingsSection";
import { TranscribeSettingsSection } from "./prompt/TranscribeSettingsSection";
import { QuickReplaceSettings } from "./QuickReplaceSettings";

const noop = () => {};
const mutation = { mutate: noop, isPending: false };
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: undefined }),
	useUpdateSTTLiveOutput: () => mutation,
	useUpdateSTTSimulatedStreaming: () => mutation,
}));
const transcribe: ComponentProps<typeof TranscribeSettingsSection> = {
	activeProfileId: "default",
	isDefaultScope: true,
	inheritTooltip: "Inherit default",
	sttProviderInheriting: false,
	sttModelInheriting: false,
	sttLanguageInheriting: false,
	sttTimeoutInheriting: false,
	effectiveSttProvider: "openai",
	sttProviderOptions: [],
	isSttProviderOptionsDisabled: false,
	sttProviderIsWhisperServer: false,
	sttModelOptions: [{ value: "current-model", label: "Current model" }],
	selectedSttModelForUi: null,
	sttPricingLabel: null,
	sttLanguageOptions: [],
	localProfileSttLanguage: "auto",
	whisperServerModelDraft: "",
	onWhisperServerModelDraftChange: noop,
	onWhisperServerModelBlur: noop,
	onSttProviderChange: noop,
	onSttModelChange: noop,
	onSttLanguageChange: noop,
	onDisableSttProviderOverride: noop,
	onDisableSttModelOverride: noop,
	onDisableSttLanguageOverride: noop,
	onDisableSttTimeoutOverride: noop,
	localProfileSttTimeout: 30,
	onSttTimeoutChange: noop,
	onSttTimeoutBlur: noop,
	sttPromptSupported: false,
	sttPromptDisabledReason: "",
	sttPromptMaxChars: 224,
	isPrompt224CharLimited: false,
	localSttTranscriptionPrompt: "",
	onSttPromptChange: noop,
	sttTestDurationMs: null,
	sttTestError: "",
	sttTestOutput: "",
	hasLastAudioForSttTest: false,
	isSttTestRunning: false,
	onRunSttTest: noop,
	hasStoredTranscriptionPrompt: false,
	managedAccessEnabled: false,
	managedModelCompatible: false,
	useManagedInference: false,
	ownKeyConfigured: true,
	onUseOwnKeyChange: noop,
};
const common = {
	activeProfileId: "default",
	activeProfile: null,
	isDefaultScope: true,
	inheritTooltip: "Inherit default",
	defaultSystemPrompt: "Test prompt",
	ocrProviderAvailable: true,
	ocrProviderUnavailableReason: null,
	llmProviderOptions: [],
	openDisableOverrideDialog: noop,
	saveProfileMetadata: noop,
	getLlmModelOptionsForProvider: () => [],
};
const rewrite: ComponentProps<typeof RewriteSettingsSection> = {
	...common,
	defaultRewriteEnabled: false,
	localProfileRewriteEnabled: false,
	rewriteEnabledInheriting: false,
	onRewriteEnabledChange: noop,
	onDisableRewriteEnabledOverride: noop,
	isUpdatingRewriteEnabled: false,
	localProfileRewriteIncludeClipboardContext: false,
	rewriteIncludeClipboardContextInheriting: false,
	onRewriteIncludeClipboardContextChange: noop,
	onDisableRewriteIncludeClipboardContextOverride: noop,
	localProfileRewriteActiveWindowOcrMode: "auto",
	rewriteActiveWindowOcrModeInheriting: false,
	onRewriteActiveWindowOcrModeChange: noop,
	onDisableRewriteActiveWindowOcrModeOverride: noop,
	effectiveLlmProvider: null,
	isLlmProviderDisabled: false,
	llmProviderInheriting: false,
	onLlmProviderChange: noop,
	onDisableLlmProviderOverride: noop,
	llmModelOptions: [],
	llmModelInheriting: false,
	localProfileLlmModel: null,
	llmPricingLabel: null,
	settings: undefined,
	onLlmModelChange: noop,
	onDisableLlmModelOverride: noop,
	supportsOpenAiThinking: false,
	openAiReasoningEffortInheriting: false,
	localProfileOpenAiReasoningEffort: "none",
	openAiThinkingOptions: [],
	effectiveLlmModel: null,
	onOpenAiThinkingChange: noop,
	onDisableOpenAiThinkingOverride: noop,
	openAiDefaultReasoningEffortForModel: () => "none",
	supportsGeminiThinkingLevel: false,
	isGemini3Pro: false,
	geminiThinkingLevelInheriting: false,
	localProfileGeminiThinkingLevel: "minimal",
	geminiThinkingLevelOptions: [],
	onGeminiThinkingLevelChange: noop,
	onDisableGeminiThinkingLevelOverride: noop,
	supportsGeminiThinkingBudget: false,
	geminiThinkingBudgetInheriting: false,
	localProfileGeminiThinkingBudget: "0",
	geminiThinkingBudgetOptions: [],
	onGeminiThinkingBudgetChange: noop,
	onDisableGeminiThinkingBudgetOverride: noop,
	supportsAnthropicThinkingBudget: false,
	anthropicThinkingBudgetInheriting: false,
	localProfileAnthropicThinkingBudget: "0",
	anthropicThinkingLevelOptionsWithCustom: [],
	onAnthropicThinkingBudgetChange: noop,
	onDisableAnthropicThinkingBudgetOverride: noop,
	formatThinkingBudgetShort: String,
	managedAccessEnabled: false,
	managedModelCompatible: false,
	usingOwnKey: true,
	ownKeyAvailable: true,
	ownKeyConfigured: true,
	onUseOwnKeyChange: noop,
};
const ask: ComponentProps<typeof QuickAskPanel> = {
	...common,
	selectDefault: "default",
	settings: undefined,
	effectiveQuickAskProvider: null,
	effectiveQuickAskModel: null,
	quickAskIncludeSelectedText: false,
	quickAskConversationHistoryEnabled: false,
	quickAskConversationHistoryCount: 0,
	localProfileQuickAskActiveWindowOcrMode: "auto",
	quickAskActiveWindowOcrModeInheriting: false,
	quickAskIncludeClipboardContextInheriting: false,
	quickAskProviderInheriting: false,
	quickAskModelInheriting: false,
	quickAskOpenAiReasoningEffortInheriting: false,
	quickAskGeminiThinkingLevelInheriting: false,
	quickAskGeminiThinkingBudgetInheriting: false,
	quickAskAnthropicThinkingBudgetInheriting: false,
	quickAskSystemPromptInheriting: false,
	quickAskDismissModeInheriting: false,
	defaultQuickAskIncludeClipboardContext: false,
	defaultQuickAskDismissMode: "auto",
	localProfileQuickAskIncludeClipboardContext: false,
	localProfileQuickAskDismissMode: "auto",
	localProfileQuickAskOpenAiReasoningEffort: "none",
	localProfileQuickAskGeminiThinkingLevel: "minimal",
	localProfileQuickAskGeminiThinkingBudget: "0",
	localProfileQuickAskAnthropicThinkingBudget: "0",
	localQuickAskSystemPrompt: "Test prompt",
	quickAskModelOptions: [],
	selectedQuickAskModelForUi: null,
	quickAskOpenAiThinkingOptions: [],
	quickAskGeminiThinkingLevelOptions: [],
	quickAskGeminiThinkingBudgetOptions: [],
	quickAskAnthropicThinkingLevelOptionsWithCustom: [],
	supportsQuickAskOpenAiThinking: false,
	supportsQuickAskGeminiThinkingLevel: false,
	supportsQuickAskGeminiThinkingBudget: false,
	supportsQuickAskAnthropicThinkingBudget: false,
	quickAskModelForThinking: null,
	llmProviderDisabled: false,
	updateQuickAskIncludeSelectedText: mutation,
	updateQuickAskConversationHistoryEnabled: mutation,
	updateQuickAskConversationHistoryCount: mutation,
	updateQuickAskOpenAiReasoningEffort: mutation,
	updateQuickAskGeminiThinkingLevel: mutation,
	updateQuickAskGeminiThinkingBudget: mutation,
	updateQuickAskAnthropicThinkingBudget: mutation,
	updateQuickAskSystemPrompt: mutation,
	updateQuickAskDismissMode: mutation,
	setQuickAskIncludeClipboardContextInheriting: noop,
	setQuickAskActiveWindowOcrModeInheriting: noop,
	setQuickAskProviderInheriting: noop,
	setQuickAskModelInheriting: noop,
	setQuickAskOpenAiReasoningEffortInheriting: noop,
	setQuickAskGeminiThinkingLevelInheriting: noop,
	setQuickAskGeminiThinkingBudgetInheriting: noop,
	setQuickAskAnthropicThinkingBudgetInheriting: noop,
	setQuickAskSystemPromptInheriting: noop,
	setQuickAskDismissModeInheriting: noop,
	setLocalProfileQuickAskIncludeClipboardContext: noop,
	setLocalProfileQuickAskActiveWindowOcrMode: noop,
	setLocalProfileQuickAskProvider: noop,
	setLocalProfileQuickAskModel: noop,
	setLocalProfileQuickAskDismissMode: noop,
	setLocalProfileQuickAskOpenAiReasoningEffort: noop,
	setLocalProfileQuickAskGeminiThinkingLevel: noop,
	setLocalProfileQuickAskGeminiThinkingBudget: noop,
	setLocalProfileQuickAskAnthropicThinkingBudget: noop,
	setLocalQuickAskSystemPrompt: noop,
	handleDefaultQuickAskProviderChange: noop,
	handleDefaultQuickAskModelChange: noop,
	isOpenAiReasoningEffort: (value): value is "none" => value === "none",
	isGeminiThinkingLevel: (value): value is "minimal" => value === "minimal",
	openAiDefaultReasoningEffortForModel: () => "none",
	formatThinkingBudgetShort: String,
	isSavingProfile: false,
	errorToMessage: String,
	quickAskTestInput: "",
	quickAskTestOutput: "",
	quickAskTestError: "",
	quickAskTestDurationMs: null,
	quickAskTestPending: false,
	quickAskTestStartRef: { current: null },
	setQuickAskTestInput: noop,
	setQuickAskTestOutput: noop,
	setQuickAskTestError: noop,
	setQuickAskTestDurationMs: noop,
	setQuickAskTestPending: noop,
};
const replace: ComponentProps<typeof QuickReplaceSettings> = {
	...common,
	defaultQuickReplaceEnabled: false,
	defaultQuickReplaceIncludeClipboardContext: false,
	localProfileQuickReplaceActiveWindowOcrMode: "auto",
	quickReplaceActiveWindowOcrModeInheriting: false,
	setQuickReplaceActiveWindowOcrModeInheriting: noop,
	setLocalProfileQuickReplaceActiveWindowOcrMode: noop,
	defaultQuickReplaceProvider: null,
	defaultQuickReplaceModel: null,
	defaultQuickReplaceSystemPrompt: "Test prompt",
	effectiveQuickReplaceProvider: null,
	llmProviderDisabled: false,
	quickReplaceModelOptions: [],
	selectedQuickReplaceModelForUi: null,
	localProfileQuickReplaceEnabled: false,
	localProfileQuickReplaceIncludeClipboardContext: false,
	localQuickReplaceSystemPrompt: "Test prompt",
	quickReplaceEnabledInheriting: false,
	quickReplaceIncludeClipboardContextInheriting: false,
	quickReplaceProviderInheriting: false,
	quickReplaceModelInheriting: false,
	quickReplaceSystemPromptInheriting: false,
	setQuickReplaceEnabledInheriting: noop,
	setQuickReplaceIncludeClipboardContextInheriting: noop,
	setQuickReplaceProviderInheriting: noop,
	setQuickReplaceModelInheriting: noop,
	setQuickReplaceSystemPromptInheriting: noop,
	setLocalProfileQuickReplaceEnabled: noop,
	setLocalProfileQuickReplaceIncludeClipboardContext: noop,
	setLocalProfileQuickReplaceProvider: noop,
	setLocalProfileQuickReplaceModel: noop,
	setLocalQuickReplaceSystemPrompt: noop,
	rewriteProvider: null,
	rewriteModel: null,
	isSaving: false,
};

it("labels automatic OCR as Always in all three flows without changing the persisted auto value", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const onRewriteChange = vi.fn();
	try {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<RewriteSettingsSection
						{...rewrite}
						onRewriteActiveWindowOcrModeChange={onRewriteChange}
					/>
					<QuickAskPanel {...ask} />
					<QuickReplaceSettings {...replace} />
				</MantineProvider>,
			),
		);
		const selectors = [
			...host.querySelectorAll<HTMLInputElement>("input"),
		].filter((input) => input.value === "Always");
		expect(selectors).toHaveLength(3);
		const autoValues = [
			...host.querySelectorAll<HTMLInputElement>('input[type="hidden"]'),
		].filter((input) => input.value === "auto");
		// Quick Ask's separate dismissal selector also has an auto value.
		expect(autoValues).toHaveLength(4);
		expect(onRewriteChange).not.toHaveBeenCalled();
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});

it("shows provider defaults rather than claiming the first model was selected", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const models = [{ value: "current-model", label: "Current model" }];
	const onModelChange = vi.fn();
	try {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<TranscribeSettingsSection
						{...transcribe}
						onSttModelChange={onModelChange}
					/>
					<RewriteSettingsSection
						{...rewrite}
						llmModelOptions={models}
						onLlmModelChange={onModelChange}
					/>
					<QuickAskPanel {...ask} quickAskModelOptions={models} />
					<QuickReplaceSettings
						{...replace}
						quickReplaceModelOptions={models}
					/>
				</MantineProvider>,
			),
		);
		const selectors = [
			...host.querySelectorAll<HTMLInputElement>(
				'input[placeholder="Provider default"]',
			),
		];
		expect(selectors).toHaveLength(4);
		expect(selectors.map((input) => input.value)).toEqual(["", "", "", ""]);
		expect(onModelChange).not.toHaveBeenCalled();
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<RewriteSettingsSection
						{...rewrite}
						isDefaultScope={false}
						llmModelOptions={models}
						localProfileLlmModel="current-model"
					/>
				</MantineProvider>,
			),
		);
		expect(
			host.querySelector<HTMLInputElement>(
				'input[placeholder="Provider default"]',
			)?.value,
		).toBe("Current model");
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});
