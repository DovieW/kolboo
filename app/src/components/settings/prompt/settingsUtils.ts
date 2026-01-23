import type {
	IntentRouterSettings,
	OpenAiReasoningEffort,
} from "../../../lib/tauri/types";

// ─────────────────────────────────────────────────────────────────────────────
// Type guards
// ─────────────────────────────────────────────────────────────────────────────

/** Check if a value is a valid OpenAI reasoning effort level. */
export const isOpenAiReasoningEffort = (
	value: unknown,
): value is OpenAiReasoningEffort => {
	return (
		value === "none" ||
		value === "minimal" ||
		value === "low" ||
		value === "medium" ||
		value === "high" ||
		value === "xhigh"
	);
};

/** Check if a value is a valid Gemini thinking level. */
export const isGeminiThinkingLevel = (
	value: unknown,
): value is "minimal" | "low" | "medium" | "high" => {
	return (
		value === "minimal" ||
		value === "low" ||
		value === "medium" ||
		value === "high"
	);
};

// ─────────────────────────────────────────────────────────────────────────────
// Formatting utilities
// ─────────────────────────────────────────────────────────────────────────────

/** Format a USD rate from micros (1/1,000,000 of a dollar) to a display string. */
export function formatUsdRateFromMicros(micros: number): string {
	const safeMicros =
		typeof micros === "number" && Number.isFinite(micros) ? micros : 0;
	const dollars = safeMicros / 1_000_000;

	if (dollars > 0 && dollars < 0.01) {
		return `$${dollars.toFixed(3).replace(/0+$/, "").replace(/\.$/, "")}`;
	}

	return `$${dollars.toFixed(2).replace(/\.00$/, "")}`;
}

// ─────────────────────────────────────────────────────────────────────────────
// Router normalization
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Normalize an intent router settings object, ensuring all fields have valid
 * values and replacing invalid/missing values with sensible defaults.
 */
export function normalizeRouter(
	router: IntentRouterSettings | null | undefined,
): IntentRouterSettings {
	const r: Partial<IntentRouterSettings> = router ?? {};
	const openai_reasoning_effort = isOpenAiReasoningEffort(
		r.openai_reasoning_effort,
	)
		? r.openai_reasoning_effort
		: null;
	const gemini_thinking_level = isGeminiThinkingLevel(r.gemini_thinking_level)
		? r.gemini_thinking_level
		: null;

	return {
		enabled: Boolean(r.enabled),
		strategy:
			r.strategy === "embeddings" || r.strategy === "llm" ? r.strategy : "off",
		embedding_provider:
			r.embedding_provider === "openai" ||
			r.embedding_provider === "cohere" ||
			r.embedding_provider === "fireworks"
				? r.embedding_provider
				: null,
		embedding_model:
			typeof r.embedding_model === "string" ? r.embedding_model : null,
		pick_highest_score:
			typeof r.pick_highest_score === "boolean" ? r.pick_highest_score : null,
		similarity_threshold:
			typeof r.similarity_threshold === "number" &&
			Number.isFinite(r.similarity_threshold)
				? r.similarity_threshold
				: null,
		similarity_margin:
			typeof r.similarity_margin === "number" &&
			Number.isFinite(r.similarity_margin)
				? r.similarity_margin
				: null,

		llm_provider: typeof r.llm_provider === "string" ? r.llm_provider : null,
		llm_model: typeof r.llm_model === "string" ? r.llm_model : null,
		openai_reasoning_effort,
		gemini_thinking_budget:
			typeof r.gemini_thinking_budget === "number" &&
			Number.isFinite(r.gemini_thinking_budget)
				? r.gemini_thinking_budget
				: null,
		gemini_thinking_level,
		anthropic_thinking_budget:
			typeof r.anthropic_thinking_budget === "number" &&
			Number.isFinite(r.anthropic_thinking_budget)
				? r.anthropic_thinking_budget
				: null,
		llm_system_prompt:
			typeof r.llm_system_prompt === "string" ? r.llm_system_prompt : null,
	};
}
