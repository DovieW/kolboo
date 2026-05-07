import type { QueryFnDeps } from "./shared";

export const createRecordingsStatsQueryFn = (deps: QueryFnDeps) => () =>
	deps.recordingsAPI.getRecordingsStats();

export const createDataStorageSummaryQueryFn = (deps: QueryFnDeps) => () =>
	deps.dataAPI.getStorageSummary();
