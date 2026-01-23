import { useMemo } from "react";

/** Select option for default/inherit value. */
export const SELECT_DEFAULT = "default";

/** Anthropic thinking level budget values. */
export const ANTHROPIC_THINKING_LEVEL_BUDGETS = [
	2000, 4000, 8000, 32000,
] as const;

/** Option type for Mantine Select components. */
export interface SelectOption {
	value: string;
	label: string;
}

const GEMINI3_FLASH_THINKING_LEVEL_OPTIONS: SelectOption[] = [
	{ value: SELECT_DEFAULT, label: "Default" },
	{ value: "minimal", label: "Minimal" },
	{ value: "low", label: "Low" },
	{ value: "medium", label: "Medium" },
	{ value: "high", label: "High" },
];

const GEMINI3_PRO_THINKING_LEVEL_OPTIONS: SelectOption[] = [
	{ value: SELECT_DEFAULT, label: "Default" },
	{ value: "low", label: "Low" },
	{ value: "high", label: "High" },
];

function gemini3ThinkingLevelOptions(isFlash: boolean): SelectOption[] {
	return isFlash
		? GEMINI3_FLASH_THINKING_LEVEL_OPTIONS
		: GEMINI3_PRO_THINKING_LEVEL_OPTIONS;
}

function withAnthropicCustomThinkingBudgetOption(
	options: SelectOption[],
	vRaw: unknown,
): SelectOption[] {
	const v =
		typeof vRaw === "number" && Number.isFinite(vRaw) ? Math.trunc(vRaw) : null;
	if (v == null) return options;

	const asString = String(v);
	const exists = options.some((o) => o.value === asString);
	if (exists) return options;

	return [...options, { value: asString, label: `Custom (${v})` }];
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure utility functions (exported for use by other components)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Get available OpenAI reasoning efforts for a model.
 * OpenAI docs (2025-12):
 * - gpt-5.1 supports: none, low, medium, high
 * - models before gpt-5.1 do not support `none`
 * - gpt-5-pro defaults to and only supports `high`
 */
export function openAiThinkingEffortsForModel(model: string): string[] {
	if (model.startsWith("gpt-5-pro")) {
		return ["high"];
	}
	if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1")) {
		return ["none", "low", "medium", "high"];
	}
	if (model.startsWith("gpt-5")) {
		return ["low", "medium", "high"];
	}
	if (model.startsWith("o")) {
		return ["low", "medium", "high"];
	}
	return [];
}

/**
 * Get default OpenAI reasoning effort for a model.
 * OpenAI docs (2025-12):
 * - gpt-5.1 defaults to `none`
 * - models before gpt-5.1 default to `medium`
 * - gpt-5-pro defaults to `high`
 */
export function openAiDefaultReasoningEffortForModel(model: string): string {
	if (model.startsWith("gpt-5-pro")) return "high";
	if (model.startsWith("gpt-5.2") || model.startsWith("gpt-5.1")) return "none";
	return "medium";
}

/** Format a thinking budget as a short label (e.g., 4000 -> "4k"). */
export function formatThinkingBudgetShort(budgetTokens: number): string {
	if (!Number.isFinite(budgetTokens) || budgetTokens <= 0)
		return String(budgetTokens);
	if (budgetTokens >= 1000) {
		const k = budgetTokens / 1000;
		const pretty = Number.isInteger(k)
			? String(k)
			: k.toFixed(1).replace(/\.0$/, "");
		return `${pretty}k`;
	}
	return String(budgetTokens);
}

// ─────────────────────────────────────────────────────────────────────────────
// Hook parameters and result
// ─────────────────────────────────────────────────────────────────────────────

export interface UseThinkingOptionsParams {
	// LLM (rewrite) context
	effectiveLlmProvider: string | null;
	effectiveLlmModel: string | null;

	// Quick Ask context
	effectiveQuickAskProvider: string | null;
	effectiveQuickAskModel: string | null;
	selectedQuickAskModelForUi: string | null;

	// For anthropic custom budget detection
	isDefaultScope: boolean;
	defaultAnthropicThinkingBudget: number | null | undefined;
	localProfileAnthropicThinkingBudget: string;
	defaultQuickAskAnthropicThinkingBudget: number | null | undefined;
	localProfileQuickAskAnthropicThinkingBudget: string;
}

export interface UseThinkingOptionsResult {
	// LLM supports flags
	supportsOpenAiThinking: boolean;
	supportsGeminiThinkingLevel: boolean;
	supportsGeminiThinkingBudget: boolean;
	supportsAnthropicThinkingBudget: boolean;

	// Quick Ask supports flags
	supportsQuickAskOpenAiThinking: boolean;
	supportsQuickAskGeminiThinkingLevel: boolean;
	supportsQuickAskGeminiThinkingBudget: boolean;
	supportsQuickAskAnthropicThinkingBudget: boolean;

	// Quick Ask model for thinking (resolved)
	quickAskModelForThinking: string | null;

	// LLM OpenAI options
	openAiThinkingOptions: SelectOption[];

	// Quick Ask OpenAI options
	quickAskOpenAiThinkingOptions: SelectOption[];

	// LLM Gemini options
	geminiThinkingLevelOptions: SelectOption[];
	geminiThinkingBudgetOptions: SelectOption[];
	isGemini3Flash: boolean;
	isGemini3Pro: boolean;
	gemini25MaxBudget: number;
	gemini25MinBudget: number;

	// Quick Ask Gemini options
	quickAskGeminiThinkingLevelOptions: SelectOption[];
	quickAskGeminiThinkingBudgetOptions: SelectOption[];
	isQuickAskGemini3Flash: boolean;

	// LLM Anthropic options
	anthropicThinkingLevelOptions: SelectOption[];
	anthropicThinkingLevelOptionsWithCustom: SelectOption[];

	// Quick Ask Anthropic options
	quickAskAnthropicThinkingLevelOptionsWithCustom: SelectOption[];
}

// ─────────────────────────────────────────────────────────────────────────────
// Hook implementation
// ─────────────────────────────────────────────────────────────────────────────

export function useThinkingOptions({
	effectiveLlmProvider,
	effectiveLlmModel,
	effectiveQuickAskProvider,
	effectiveQuickAskModel,
	selectedQuickAskModelForUi,
	isDefaultScope,
	defaultAnthropicThinkingBudget,
	localProfileAnthropicThinkingBudget,
	defaultQuickAskAnthropicThinkingBudget,
	localProfileQuickAskAnthropicThinkingBudget,
}: UseThinkingOptionsParams): UseThinkingOptionsResult {
	// ─────────────────────────────────────────────────────────────────────────
	// LLM (rewrite) supports flags
	// ─────────────────────────────────────────────────────────────────────────

	const supportsOpenAiThinking =
		effectiveLlmProvider === "openai" &&
		!!effectiveLlmModel &&
		(effectiveLlmModel.startsWith("gpt-5") ||
			effectiveLlmModel.startsWith("o"));

	const supportsGeminiThinkingLevel =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-3");

	const supportsGeminiThinkingBudget =
		effectiveLlmProvider === "gemini" &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5") &&
		!effectiveLlmModel.includes("flash-lite");

	const supportsAnthropicThinkingBudget =
		effectiveLlmProvider === "anthropic" &&
		!!effectiveLlmModel &&
		// Extended thinking is supported by newer Claude families. Keep conservative.
		(effectiveLlmModel.includes("claude-3-7") ||
			effectiveLlmModel.includes("claude-4") ||
			effectiveLlmModel.includes("-4-"));

	// ─────────────────────────────────────────────────────────────────────────
	// Quick Ask supports flags
	// ─────────────────────────────────────────────────────────────────────────

	const quickAskModelForThinking =
		selectedQuickAskModelForUi ?? effectiveQuickAskModel;

	const supportsQuickAskOpenAiThinking =
		effectiveQuickAskProvider === "openai" &&
		!!quickAskModelForThinking &&
		(quickAskModelForThinking.startsWith("gpt-5") ||
			quickAskModelForThinking.startsWith("o"));

	const supportsQuickAskGeminiThinkingLevel =
		effectiveQuickAskProvider === "gemini" &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-3");

	const supportsQuickAskGeminiThinkingBudget =
		effectiveQuickAskProvider === "gemini" &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5") &&
		!quickAskModelForThinking.includes("flash-lite");

	const supportsQuickAskAnthropicThinkingBudget =
		effectiveQuickAskProvider === "anthropic" &&
		!!quickAskModelForThinking &&
		(quickAskModelForThinking.includes("claude-3-7") ||
			quickAskModelForThinking.includes("claude-4") ||
			quickAskModelForThinking.includes("-4-"));

	// ─────────────────────────────────────────────────────────────────────────
	// OpenAI thinking options
	// ─────────────────────────────────────────────────────────────────────────

	const openAiThinkingOptions = useMemo<SelectOption[]>(() => {
		if (!supportsOpenAiThinking || !effectiveLlmModel) return [];
		return [
			{ value: SELECT_DEFAULT, label: "Default" },
			...openAiThinkingEffortsForModel(effectiveLlmModel).map((v) => ({
				value: v,
				label: v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
			})),
		];
	}, [supportsOpenAiThinking, effectiveLlmModel]);

	const quickAskOpenAiThinkingOptions = useMemo<SelectOption[]>(() => {
		if (!supportsQuickAskOpenAiThinking || !quickAskModelForThinking) return [];
		return [
			{ value: SELECT_DEFAULT, label: "Default" },
			...openAiThinkingEffortsForModel(quickAskModelForThinking).map((v) => ({
				value: v,
				label: v === "none" ? "None" : v.charAt(0).toUpperCase() + v.slice(1),
			})),
		];
	}, [supportsQuickAskOpenAiThinking, quickAskModelForThinking]);

	// ─────────────────────────────────────────────────────────────────────────
	// Gemini thinking level options (gemini-3)
	// ─────────────────────────────────────────────────────────────────────────

	const isGemini3Flash =
		supportsGeminiThinkingLevel &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-3-flash");

	const isGemini3Pro =
		supportsGeminiThinkingLevel &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-3-pro");

	const isQuickAskGemini3Flash =
		supportsQuickAskGeminiThinkingLevel &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-3-flash");

	const geminiThinkingLevelOptions = useMemo<SelectOption[]>(() => {
		return gemini3ThinkingLevelOptions(isGemini3Flash);
	}, [isGemini3Flash]);

	const quickAskGeminiThinkingLevelOptions = useMemo<SelectOption[]>(() => {
		return gemini3ThinkingLevelOptions(isQuickAskGemini3Flash);
	}, [isQuickAskGemini3Flash]);

	// ─────────────────────────────────────────────────────────────────────────
	// Gemini thinking budget options (gemini-2.5)
	// ─────────────────────────────────────────────────────────────────────────

	const canDisableGemini25Thinking =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-flash") &&
		!effectiveLlmModel.includes("gemini-2.5-pro");

	const isGemini25Pro =
		supportsGeminiThinkingBudget &&
		!!effectiveLlmModel &&
		effectiveLlmModel.includes("gemini-2.5-pro");

	const gemini25MaxBudget = isGemini25Pro ? 32768 : 24576;
	const gemini25MinBudget = isGemini25Pro ? 128 : 0;

	const geminiThinkingBudgetOptions = useMemo<SelectOption[]>(() => {
		return [
			{ value: SELECT_DEFAULT, label: "Default" },
			{ value: "-1", label: "Dynamic (-1)" },
			...(canDisableGemini25Thinking ? [{ value: "0", label: "Off (0)" }] : []),
			...(isGemini25Pro
				? [{ value: String(gemini25MinBudget), label: "Minimal (128)" }]
				: []),
			{ value: "1024", label: "Light (1024)" },
			{ value: "4096", label: "Medium (4096)" },
			{ value: "16384", label: "High (16384)" },
			...(gemini25MaxBudget > 16384
				? [
						{
							value: String(gemini25MaxBudget),
							label: `Max (${gemini25MaxBudget})`,
						},
					]
				: []),
		];
	}, [
		canDisableGemini25Thinking,
		isGemini25Pro,
		gemini25MinBudget,
		gemini25MaxBudget,
	]);

	// Quick Ask Gemini 2.5 budget options
	const canDisableQuickAskGemini25Thinking =
		supportsQuickAskGeminiThinkingBudget &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5-flash") &&
		!quickAskModelForThinking.includes("gemini-2.5-pro");

	const isQuickAskGemini25Pro =
		supportsQuickAskGeminiThinkingBudget &&
		!!quickAskModelForThinking &&
		quickAskModelForThinking.includes("gemini-2.5-pro");

	const quickAskGemini25MaxBudget = isQuickAskGemini25Pro ? 32768 : 24576;
	const quickAskGemini25MinBudget = isQuickAskGemini25Pro ? 128 : 0;

	const quickAskGeminiThinkingBudgetOptions = useMemo<SelectOption[]>(() => {
		return [
			{ value: SELECT_DEFAULT, label: "Default" },
			{ value: "-1", label: "Dynamic (-1)" },
			...(canDisableQuickAskGemini25Thinking
				? [{ value: "0", label: "Off (0)" }]
				: []),
			...(isQuickAskGemini25Pro
				? [
						{
							value: String(quickAskGemini25MinBudget),
							label: "Minimal (128)",
						},
					]
				: []),
			{ value: "1024", label: "Light (1024)" },
			{ value: "4096", label: "Medium (4096)" },
			{ value: "16384", label: "High (16384)" },
			...(quickAskGemini25MaxBudget > 16384
				? [
						{
							value: String(quickAskGemini25MaxBudget),
							label: `Max (${quickAskGemini25MaxBudget})`,
						},
					]
				: []),
		];
	}, [
		canDisableQuickAskGemini25Thinking,
		isQuickAskGemini25Pro,
		quickAskGemini25MinBudget,
		quickAskGemini25MaxBudget,
	]);

	// ─────────────────────────────────────────────────────────────────────────
	// Anthropic thinking budget options
	// ─────────────────────────────────────────────────────────────────────────

	const anthropicThinkingLevelOptions = useMemo<SelectOption[]>(() => {
		return [
			{ value: SELECT_DEFAULT, label: "Default" },
			// Allow profiles to explicitly turn off thinking even if Default enables it.
			...(!isDefaultScope ? [{ value: "0", label: "Off" }] : []),
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[0]), label: "Low" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[1]), label: "Medium" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[2]), label: "High" },
			{ value: String(ANTHROPIC_THINKING_LEVEL_BUDGETS[3]), label: "Max" },
		];
	}, [isDefaultScope]);

	const anthropicThinkingLevelOptionsWithCustom = useMemo<
		SelectOption[]
	>(() => {
		const vRaw = isDefaultScope
			? defaultAnthropicThinkingBudget
			: localProfileAnthropicThinkingBudget === SELECT_DEFAULT
				? null
				: Number(localProfileAnthropicThinkingBudget);
		return withAnthropicCustomThinkingBudgetOption(
			anthropicThinkingLevelOptions,
			vRaw,
		);
	}, [
		isDefaultScope,
		defaultAnthropicThinkingBudget,
		localProfileAnthropicThinkingBudget,
		anthropicThinkingLevelOptions,
	]);

	const quickAskAnthropicThinkingLevelOptionsWithCustom = useMemo<
		SelectOption[]
	>(() => {
		const vRaw = isDefaultScope
			? defaultQuickAskAnthropicThinkingBudget
			: localProfileQuickAskAnthropicThinkingBudget === SELECT_DEFAULT
				? null
				: Number(localProfileQuickAskAnthropicThinkingBudget);
		return withAnthropicCustomThinkingBudgetOption(
			anthropicThinkingLevelOptions,
			vRaw,
		);
	}, [
		isDefaultScope,
		defaultQuickAskAnthropicThinkingBudget,
		localProfileQuickAskAnthropicThinkingBudget,
		anthropicThinkingLevelOptions,
	]);

	return {
		// LLM supports flags
		supportsOpenAiThinking,
		supportsGeminiThinkingLevel,
		supportsGeminiThinkingBudget,
		supportsAnthropicThinkingBudget,

		// Quick Ask supports flags
		supportsQuickAskOpenAiThinking,
		supportsQuickAskGeminiThinkingLevel,
		supportsQuickAskGeminiThinkingBudget,
		supportsQuickAskAnthropicThinkingBudget,

		// Quick Ask model
		quickAskModelForThinking,

		// LLM OpenAI
		openAiThinkingOptions,

		// Quick Ask OpenAI
		quickAskOpenAiThinkingOptions,

		// LLM Gemini
		geminiThinkingLevelOptions,
		geminiThinkingBudgetOptions,
		isGemini3Flash,
		isGemini3Pro,
		gemini25MaxBudget,
		gemini25MinBudget,

		// Quick Ask Gemini
		quickAskGeminiThinkingLevelOptions,
		quickAskGeminiThinkingBudgetOptions,
		isQuickAskGemini3Flash,

		// LLM Anthropic
		anthropicThinkingLevelOptions,
		anthropicThinkingLevelOptionsWithCustom,

		// Quick Ask Anthropic
		quickAskAnthropicThinkingLevelOptionsWithCustom,
	};
}
