import type { CostTimeframe } from "./tauri/types";

export type CostKind = "all" | "stt" | "llm";

export type CostQueryParams = {
	timeframe: CostTimeframe;
	kind?: CostKind;
	sttModelKeys?: string[];
	llmModelKeys?: string[];
	excludeFreeTier?: boolean;
};

export type CostInvokeParams = {
	timeframe: CostTimeframe;
	kind?: Exclude<CostKind, "all">;
	sttModelKeys?: string[];
	llmModelKeys?: string[];
	excludeFreeTier?: boolean;
};

export function normalizeCostKind(
	kind?: CostKind,
): Exclude<CostKind, "all"> | undefined {
	return kind === "all" ? undefined : kind;
}

/**
 * Normalize cost query params into the shape expected by the backend invoke calls.
 *
 * Today this mainly means mapping `kind: "all"` => `kind: undefined`.
 */
export function buildCostInvokeParams(
	params: CostQueryParams,
): CostInvokeParams {
	return {
		timeframe: params.timeframe,
		kind: normalizeCostKind(params.kind),
		sttModelKeys: params.sttModelKeys,
		llmModelKeys: params.llmModelKeys,
		excludeFreeTier: params.excludeFreeTier,
	};
}
