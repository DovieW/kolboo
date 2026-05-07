import type { IntentRouterSettings } from "../types";
import {
	normalizeAnthropicThinkingBudget,
	normalizeGeminiThinkingBudget,
	normalizeGeminiThinkingLevel,
	normalizeOpenAiReasoningEffort,
} from "./reasoning";
import { isRecord } from "./shared";

export function normalizeIntentRouterStrategy(
	value: unknown,
): IntentRouterSettings["strategy"] {
	if (value === "off" || value === "embeddings" || value === "llm") {
		return value;
	}
	return "off";
}

export function normalizeIntentRouterSettings(
	value: unknown,
): IntentRouterSettings {
	const v = isRecord(value) ? value : {};
	const enabled = typeof v.enabled === "boolean" ? v.enabled : false;
	const strategy = normalizeIntentRouterStrategy(v.strategy);

	const embedding_provider =
		v.embedding_provider === "openai" ||
		v.embedding_provider === "cohere" ||
		v.embedding_provider === "fireworks"
			? (v.embedding_provider as "openai" | "cohere" | "fireworks")
			: null;
	const embedding_model =
		typeof v.embedding_model === "string" ? v.embedding_model : null;

	const pick_highest_score =
		typeof v.pick_highest_score === "boolean" ? v.pick_highest_score : null;

	const similarity_threshold =
		typeof v.similarity_threshold === "number" &&
		Number.isFinite(v.similarity_threshold)
			? v.similarity_threshold
			: null;
	const similarity_margin =
		typeof v.similarity_margin === "number" &&
		Number.isFinite(v.similarity_margin)
			? v.similarity_margin
			: null;

	const llm_provider =
		typeof v.llm_provider === "string" ? v.llm_provider : null;
	const llm_model = typeof v.llm_model === "string" ? v.llm_model : null;

	const openai_reasoning_effort = normalizeOpenAiReasoningEffort(
		v.openai_reasoning_effort,
	);
	const gemini_thinking_budget = normalizeGeminiThinkingBudget(
		v.gemini_thinking_budget,
	);
	const gemini_thinking_level = normalizeGeminiThinkingLevel(
		v.gemini_thinking_level,
	);
	const anthropic_thinking_budget = normalizeAnthropicThinkingBudget(
		v.anthropic_thinking_budget,
	);

	const llm_system_prompt =
		typeof v.llm_system_prompt === "string" ? v.llm_system_prompt : null;

	return {
		enabled,
		strategy,
		embedding_provider,
		embedding_model,
		pick_highest_score,
		similarity_threshold,
		similarity_margin,
		llm_provider,
		llm_model,
		openai_reasoning_effort,
		gemini_thinking_budget,
		gemini_thinking_level,
		anthropic_thinking_budget,
		llm_system_prompt,
	};
}
