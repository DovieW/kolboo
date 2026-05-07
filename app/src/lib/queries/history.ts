import {
	keepPreviousData,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";

import { type HistoryPageQuery, sttAPI } from "../tauri";
import { createHistoryAllQueryFn, createHistoryPageQueryFn } from "./queryFns";
import { queryFnDeps } from "./shared";

// History hooks share filter normalization and retry-related invalidation, so
// they live together instead of spreading history semantics across the app.
export function useHistoryAll(options?: { enabled?: boolean }) {
	return useQuery({
		queryKey: ["historyAll"],
		queryFn: createHistoryAllQueryFn(queryFnDeps),
		enabled: options?.enabled ?? true,
	});
}

export function useHistoryPage(params: HistoryPageQuery) {
	const { normalized, queryFn } = createHistoryPageQueryFn(queryFnDeps, params);

	return useQuery({
		queryKey: [
			"historyPage",
			normalized.filterText,
			normalized.showFailed,
			normalized.showEmptyTranscript,
			normalized.selectedSttModelKeys,
			normalized.selectedLlmModelKeys,
			normalized.page,
			normalized.pageSize,
			normalized.includeUsageCounts,
		],
		queryFn,
		placeholderData: keepPreviousData,
		// Keep things feeling responsive while typing filters.
		refetchOnWindowFocus: true,
	});
}

// Retry a previous transcription attempt by request id (loads saved audio in backend).
export function useRetryTranscription() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (requestId: string) => sttAPI.retryTranscription({ requestId }),
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["history"] });
			queryClient.invalidateQueries({ queryKey: ["historyAll"] });
			queryClient.invalidateQueries({ queryKey: ["historyPage"] });
			queryClient.invalidateQueries({ queryKey: ["requestLogs"] });
		},
	});
}
