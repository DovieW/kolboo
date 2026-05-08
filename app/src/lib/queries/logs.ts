import { useQuery } from "@tanstack/react-query";

import { createRequestLogsQueryFn } from "./queryFns";
import { queryFnDeps } from "./shared";

// Request-log hooks stay together because they share the same live-refresh and
// clearing invalidation behavior.
export function useRequestLogs(limit?: number) {
	return useQuery({
		queryKey: ["requestLogs", limit],
		queryFn: createRequestLogsQueryFn(queryFnDeps, limit),
		refetchInterval: 2000,
	});
}
