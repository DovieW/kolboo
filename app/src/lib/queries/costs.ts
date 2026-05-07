import { useQuery } from "@tanstack/react-query";

import type { CostTimeframe, ModelPricingKind } from "../tauri";
import {
	createCostByProviderQueryFn,
	createCostSummaryQueryFn,
	createModelPricingQueryFn,
} from "./queryFns";
import { queryFnDeps } from "./shared";

// Cost hooks stay together because they share normalized filter handling and
// the same stats-derived query freshness expectations.
export function useModelPricing(
	provider: string | null,
	kind: ModelPricingKind,
	model: string | null,
) {
	const { normalized, queryFn } = createModelPricingQueryFn(queryFnDeps, {
		provider,
		kind,
		model,
	});

	return useQuery({
		queryKey: [
			"modelPricing",
			normalized.provider,
			normalized.kind,
			normalized.model,
		],
		enabled: Boolean(provider) && Boolean(model),
		queryFn,
		staleTime: Number.POSITIVE_INFINITY,
	});
}

export function useCostSummary(
	timeframe: CostTimeframe,
	filters?: {
		kind?: "all" | "stt" | "llm";
		sttModelKeys?: string[];
		llmModelKeys?: string[];
		excludeFreeTier?: boolean;
	},
) {
	const { normalized, queryFn } = createCostSummaryQueryFn(
		queryFnDeps,
		timeframe,
		filters,
	);

	return useQuery({
		queryKey: [
			"costSummary",
			timeframe,
			normalized.kind ?? "all",
			normalized.excludeFreeTier ? "exclude_free" : "include_free",
			normalized.sttModelKeys,
			normalized.llmModelKeys,
		],
		queryFn,
		staleTime: 10_000,
		refetchOnWindowFocus: true,
	});
}

export function useCostByProvider(
	timeframe: CostTimeframe,
	filters?: {
		kind?: "all" | "stt" | "llm";
		sttModelKeys?: string[];
		llmModelKeys?: string[];
		excludeFreeTier?: boolean;
	},
) {
	const { normalized, queryFn } = createCostByProviderQueryFn(
		queryFnDeps,
		timeframe,
		filters,
	);

	return useQuery({
		queryKey: [
			"costByProvider",
			timeframe,
			normalized.kind ?? "all",
			normalized.excludeFreeTier ? "exclude_free" : "include_free",
			normalized.sttModelKeys,
			normalized.llmModelKeys,
		],
		queryFn,
		staleTime: 10_000,
		refetchOnWindowFocus: true,
	});
}
