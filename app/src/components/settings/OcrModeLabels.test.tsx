// @vitest-environment happy-dom
import { Accordion, MantineProvider } from "@mantine/core";
import { act, type ComponentProps } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { QuickAskPanel } from "./prompt/QuickAskPanel";
import { RewriteSettingsSection } from "./prompt/RewriteSettingsSection";
import { TranscribeSettingsSection } from "./prompt/TranscribeSettingsSection";
import { QuickReplaceSettings } from "./QuickReplaceSettings";
import { PromptIntentRouterSection } from "./prompt/PromptIntentRouterSection";
import { llmAPI, type AppSettings } from "../../lib/tauri";
import type { ModelChoice } from "../../lib/modelSelection";

const noop = () => {};
const availability = vi.hoisted(() => ({
	settings: undefined as Partial<AppSettings> | undefined,
	insertion: true,
	choices: [] as ModelChoice[],
}));
const mutation = { mutate: vi.fn(), isPending: false };
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({ data: availability.settings }),
	useUpdateSTTLiveOutput: () => mutation,
	useUpdateSTTSimulatedStreaming: () => mutation,
	useUpdateSTTInputMode: () => mutation,
	useAvailableProviders: () => ({
		data: { live_text_insertion_supported: availability.insertion },
	}),
}));
vi.mock("../../lib/queries/settings", () => ({
	useSettings: () => ({ data: undefined }),
	useUpdateModelSelection: () => mutation,
}));
vi.mock("../../lib/queries/modelChoices", () => ({
	useModelChoices: () => ({
		choices: availability.choices,
		loading: false,
		error: false,
	}),
}));
const transcribe: ComponentProps<typeof TranscribeSettingsSection> = {
	activeProfileId: "default",
	isDefaultScope: true,
	inheritTooltip: "Inherit default",
	sttLanguageInheriting: false,
	sttTimeoutInheriting: false,
	effectiveSttProvider: "openai",
	selectedSttModelForUi: null,
	sttLanguageOptions: [],
	localProfileSttLanguage: "auto",
	onSttLanguageChange: noop,
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
	settings: undefined,
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
	setQuickAskOpenAiReasoningEffortInheriting: noop,
	setQuickAskGeminiThinkingLevelInheriting: noop,
	setQuickAskGeminiThinkingBudgetInheriting: noop,
	setQuickAskAnthropicThinkingBudgetInheriting: noop,
	setQuickAskSystemPromptInheriting: noop,
	setQuickAskDismissModeInheriting: noop,
	setLocalProfileQuickAskIncludeClipboardContext: noop,
	setLocalProfileQuickAskActiveWindowOcrMode: noop,
	setLocalProfileQuickAskDismissMode: noop,
	setLocalProfileQuickAskOpenAiReasoningEffort: noop,
	setLocalProfileQuickAskGeminiThinkingLevel: noop,
	setLocalProfileQuickAskGeminiThinkingBudget: noop,
	setLocalProfileQuickAskAnthropicThinkingBudget: noop,
	setLocalQuickAskSystemPrompt: noop,
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
	defaultQuickReplaceSystemPrompt: "Test prompt",
	localProfileQuickReplaceEnabled: false,
	localProfileQuickReplaceIncludeClipboardContext: false,
	localQuickReplaceSystemPrompt: "Test prompt",
	quickReplaceEnabledInheriting: false,
	quickReplaceIncludeClipboardContextInheriting: false,
	quickReplaceSystemPromptInheriting: false,
	setQuickReplaceEnabledInheriting: noop,
	setQuickReplaceIncludeClipboardContextInheriting: noop,
	setQuickReplaceSystemPromptInheriting: noop,
	setLocalProfileQuickReplaceEnabled: noop,
	setLocalProfileQuickReplaceIncludeClipboardContext: noop,
	setLocalQuickReplaceSystemPrompt: noop,
	isSaving: false,
};

it("tests Quick Ask with its scoped atomic route rather than the rewrite route", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: { addEventListener: vi.fn(), removeEventListener: vi.fn() },
	});
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const selection = {
		source: "byok",
		provider_id: "ollama",
		model_id: "ask-id",
	} as const;
	const completion = vi.spyOn(llmAPI, "complete").mockResolvedValue({
		output: "Synthetic answer",
		provider_used: "ollama",
		model_used: "ask-id",
	});
	try {
		for (const isDefaultScope of [true, false]) {
			await act(async () =>
				root.render(
					<MantineProvider env="test">
						<QuickAskPanel
							{...ask}
							isDefaultScope={isDefaultScope}
							activeProfileId={isDefaultScope ? "default" : "p"}
							activeProfile={
								{
									id: "p",
									model_selections: { quick_ask: selection },
								} as ComponentProps<typeof QuickAskPanel>["activeProfile"]
							}
							settings={
								{ model_selections: { quick_ask: selection } } as AppSettings
							}
							effectiveQuickAskProvider="ollama"
							effectiveQuickAskModel="ask-id"
							quickAskTestInput="Synthetic question"
						/>
					</MantineProvider>,
				),
			);
			const test = [...host.querySelectorAll<HTMLButtonElement>("button")].find(
				(button) => button.textContent === "Test",
			);
			expect(test).toBeDefined();
			await act(async () => test?.click());
			expect(completion).toHaveBeenLastCalledWith(
				expect.objectContaining({
					action: "quick_ask",
					profileId: isDefaultScope ? "default" : "p",
					modelSelection: selection,
				}),
			);
		}
	} finally {
		await act(async () => root.unmount());
		host.remove();
		completion.mockRestore();
	}
});

it("changes router strategy without inventing a model and displays scoped routing metadata", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const save = vi.fn();
	const props: ComponentProps<typeof PromptIntentRouterSection> = {
		activeProfileId: "p",
		presets: [{ id: "q", name: "Preset" }] as ComponentProps<
			typeof PromptIntentRouterSection
		>["presets"],
		settings: undefined,
		profileRouter: null,
		effectiveRouter: null,
		routerStrategyValue: "llm",
		embeddingProviderValue: "openai",
		embeddingModels: [],
		embeddingModelValue: null,
		isCachingRouterEmbeddings: false,
		selectDefaultValue: "default",
		anthropicThinkingBudgets: [],
		getEmbeddingModelsForProvider: () => [],
		saveRouter: save,
		onCacheRouterEmbeddings: noop,
	};
	try {
		for (const settings of [
			undefined,
			{ rewrite_program_prompt_profiles: [] },
			{
				llm_provider: "openai",
				llm_model: "gpt-5",
				rewrite_program_prompt_profiles: [
					{ id: "other" },
					{ id: "p", router: { llm_provider: "openai", llm_model: "gpt-5" } },
				],
			},
		] as (AppSettings | undefined)[]) {
			await act(async () =>
				root.render(
					<MantineProvider env="test">
						<Accordion value="p-intent-router">
							<PromptIntentRouterSection {...props} settings={settings} />
						</Accordion>
					</MantineProvider>,
				),
			);
			expect(host.querySelector('[aria-label="Routing model"]')).not.toBeNull();
		}
		for (const effectiveRouter of [
			null,
			{ llm_provider: "ollama", llm_model: "existing-id" },
		] as ComponentProps<
			typeof PromptIntentRouterSection
		>["effectiveRouter"][]) {
			await act(async () =>
				root.render(
					<MantineProvider env="test">
						<Accordion value="p-intent-router">
							<PromptIntentRouterSection
								{...props}
								effectiveRouter={effectiveRouter}
								routerStrategyValue="off"
							/>
						</Accordion>
					</MantineProvider>,
				),
			);
			await act(async () =>
				host.querySelector<HTMLInputElement>('input[role="combobox"]')?.click(),
			);
			const option = [
				...document.querySelectorAll<HTMLElement>('[role="option"]'),
			].find((option) => option.textContent === "LLM");
			expect(option).toBeDefined();
			await act(async () => option?.click());
			expect(save).toHaveBeenLastCalledWith(
				expect.objectContaining({
					strategy: "llm",
					llm_provider: effectiveRouter?.llm_provider ?? null,
					llm_model: effectiveRouter?.llm_model ?? null,
				}),
			);
		}
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});

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

it("does not select any first model when the shared pickers are unset", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	try {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<TranscribeSettingsSection {...transcribe} />
					<RewriteSettingsSection {...rewrite} />
					<QuickAskPanel {...ask} />
					<QuickReplaceSettings {...replace} />
				</MantineProvider>,
			),
		);
		const selectors = [
			...host.querySelectorAll<HTMLButtonElement>("button"),
		].filter((button) => button.textContent?.includes("Choose a model"));
		expect(selectors).toHaveLength(4);
		expect(selectors.map((input) => input.value)).toEqual(["", "", "", ""]);
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<RewriteSettingsSection {...rewrite} isDefaultScope={false} />
				</MantineProvider>,
			),
		);
		expect(host.textContent).toContain("Choose a model");
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});

it("saves input mode at its own scope and preserves inheritance", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const change = vi.fn();
	const render = async (
		props: Partial<ComponentProps<typeof TranscribeSettingsSection>>,
	) => {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<TranscribeSettingsSection {...transcribe} {...props} />
				</MantineProvider>,
			),
		);
	};
	const choose = async (label: string) => {
		const input = host.querySelector<HTMLInputElement>(
			'input[value="Auto"], input[value="After recording"], input[value^="Inherit ("]',
		);
		expect(input).not.toBeNull();
		await act(async () => input?.click());
		const option = [
			...document.querySelectorAll<HTMLElement>('[role="option"]'),
		].find((e) => e.textContent === label);
		expect(option).toBeDefined();
		await act(async () => option?.click());
	};
	try {
		await render({});
		await choose("After recording");
		expect(mutation.mutate).toHaveBeenLastCalledWith("after_recording");
		await render({
			isDefaultScope: false,
			activeProfileId: "p",
			onSttInputModeChange: change,
		});
		await choose("After recording");
		expect(change).toHaveBeenLastCalledWith("after_recording");
		await render({
			isDefaultScope: false,
			activeProfileId: "p",
			sttInputMode: "after_recording",
			onSttInputModeChange: change,
			effectiveSttProvider: null,
			selectedSttModelForUi: null,
		});
		await choose("Inherit (Auto)");
		expect(change).toHaveBeenLastCalledWith(null);
		await render({
			isDefaultScope: false,
			activeProfileId: "p",
			sttInputMode: "after_recording",
		});
		await choose("Auto");
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});

it("only offers live insertion when rewriting and routing are off and the desktop supports it", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const render = async (scope = true) => {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<TranscribeSettingsSection
						{...transcribe}
						isDefaultScope={scope}
						activeProfileId={scope ? "default" : "p"}
						selectedSttModelForUi="gpt-live-transcribe"
					/>
				</MantineProvider>,
			),
		);
	};
	const control = () =>
		host.querySelector<HTMLInputElement>('input[type="checkbox"]');
	try {
		mutation.mutate.mockClear();
		availability.settings = {
			rewrite_llm_enabled: false,
			quick_replace_enabled: false,
			stt_input_mode: "auto",
		};
		await render();
		expect(control()?.disabled).toBe(false);
		await act(async () => control()?.click());
		expect(mutation.mutate).toHaveBeenLastCalledWith(true);
		for (const settings of [
			undefined,
			{ rewrite_llm_enabled: true },
			{ rewrite_llm_enabled: false, quick_replace_enabled: true },
		]) {
			availability.settings = settings;
			await render();
			expect(control()?.disabled).toBe(true);
			expect(host.textContent).toContain("requires rewriting");
		}
		availability.settings = {
			rewrite_llm_enabled: false,
			stt_live_output: true,
		};
		availability.insertion = false;
		await render();
		expect(control()?.disabled).toBe(true);
		expect(control()?.checked).toBe(true);
		availability.insertion = true;
		for (const profile of [
			undefined,
			{ id: "p", rewrite_llm_enabled: false, quick_replace_enabled: false },
			{ id: "p", rewrite_llm_enabled: false, router: { enabled: true } },
		]) {
			availability.settings = {
				rewrite_llm_enabled: false,
				rewrite_program_prompt_profiles: profile
					? [profile as AppSettings["rewrite_program_prompt_profiles"][number]]
					: [],
			};
			await render(false);
			expect(control()?.disabled).toBe(
				profile?.router?.enabled === true || !profile,
			);
		}
		availability.settings = {
			model_selections: {
				transcription: {
					source: "managed",
					provider_id: "groq",
					model_id: "whisper-large-v3-turbo",
				},
			},
		};
		await render();
		expect(host.textContent).toContain(
			"Kolboo Managed uploads after recording",
		);
		expect(control()).toBeNull();
		const managed: ModelChoice = {
			selection: {
				source: "managed",
				provider_id: "openai",
				model_id: "gpt-live-transcribe",
			},
			displayName: "Synthetic model",
			providerName: "Synthetic provider",
			ready: true,
			capabilities: [],
		};
		availability.settings = {
			rewrite_llm_enabled: false,
			stt_use_managed_inference: true,
		};
		for (const choice of [
			{ ...managed, ready: false },
			{
				...managed,
				selection: { ...managed.selection, source: "byok" as const },
			},
			{ ...managed, selection: { ...managed.selection, provider_id: "other" } },
			{ ...managed, selection: { ...managed.selection, model_id: "other" } },
		]) {
			availability.choices = [choice];
			await render();
			expect(control()?.disabled).toBe(false);
			expect(host.textContent).not.toContain(
				"Kolboo Managed uploads after recording",
			);
		}
		availability.choices = [managed];
		await render();
		expect(control()).toBeNull();
		expect(host.textContent).toContain(
			"Kolboo Managed uploads after recording",
		);
		availability.settings.model_selections = {
			transcription: { ...managed.selection, source: "byok" },
		};
		await render();
		expect(control()?.disabled).toBe(false);
		availability.settings.rewrite_program_prompt_profiles = [
			{
				id: "p",
				rewrite_llm_enabled: false,
				model_selections: { transcription: managed.selection },
			} as AppSettings["rewrite_program_prompt_profiles"][number],
		];
		await render(false);
		expect(control()).toBeNull();
	} finally {
		await act(async () => root.unmount());
		host.remove();
		availability.settings = undefined;
		availability.insertion = true;
		availability.choices = [];
	}
});
