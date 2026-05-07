import type { QueryFnDeps } from "./shared";

export const createRequestLogsQueryFn =
	(deps: QueryFnDeps, limit?: number) => () =>
		deps.logsAPI.getRequestLogs(limit);
