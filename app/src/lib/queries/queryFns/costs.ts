import type { CostTimeframe, ModelPricingKind } from "../../tauri";
import {
	type CostFilters,
	createCostQueryFn,
	normalizeModelPricingParams,
	type QueryFnDeps,
} from "./shared";

export const createModelPricingQueryFn = (
	deps: QueryFnDeps,
	params: {
		provider: string | null;
		kind: ModelPricingKind;
		model: string | null;
	},
) => {
	const normalized = normalizeModelPricingParams(params);
	return {
		normalized,
		queryFn: () =>
			deps.tauriAPI.getModelPricing({
				provider: normalized.provider,
				kind: normalized.kind,
				model: normalized.model,
			}),
	};
};

export const createCostSummaryQueryFn = (
	deps: QueryFnDeps,
	timeframe: CostTimeframe,
	filters?: CostFilters,
) => {
	return createCostQueryFn(timeframe, filters, (params) =>
		deps.tauriAPI.getCostSummary(params),
	);
};

export const createCostByProviderQueryFn = (
	deps: QueryFnDeps,
	timeframe: CostTimeframe,
	filters?: CostFilters,
) => {
	return createCostQueryFn(timeframe, filters, (params) =>
		deps.tauriAPI.getCostByProvider(params),
	);
};
