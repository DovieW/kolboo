import type { HistoryPageQuery } from "../../tauri";
import { normalizeHistoryPageQuery, type QueryFnDeps } from "./shared";

export const createHistoryAllQueryFn = (deps: QueryFnDeps) => () =>
	deps.tauriAPI.getHistory(undefined);

export const createHistoryPageQueryFn = (
	deps: QueryFnDeps,
	params: HistoryPageQuery,
) => {
	const normalized = normalizeHistoryPageQuery(params);
	return {
		normalized,
		queryFn: () => deps.tauriAPI.getHistoryPage(normalized),
	};
};
