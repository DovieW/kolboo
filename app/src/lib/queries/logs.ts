import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { logsAPI } from "../tauri";
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

export function useClearRequestLogs() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => logsAPI.clearRequestLogs(),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		},
	});
}
