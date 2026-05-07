import { useClipboard, useDisclosure, useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatErrorMessage } from "../lib/formatError";
import {
	type AnalysisPromptStyle,
	buildAnalysisPrompt,
	estimateTokenCount,
	getHistoryFeedEmptyState,
	groupHistoryForDisplay,
	HISTORY_PAGE_SIZE,
} from "../lib/history/readModel";
import {
	getHistoryPageCount,
	useHistoryFeedFilters,
} from "../lib/history/useHistoryFeedFilters";
import { getRetryLastFailedCandidate } from "../lib/historyRetry";
import { listAllLlmModelKeys, listAllSttModelKeys } from "../lib/modelOptions";
import {
	useHistoryAll,
	useHistoryPage,
	useRecordingsStats,
	useRequestLogs,
	useRetryTranscription,
	useSettings,
} from "../lib/queries";
import {
	dataAPI,
	type HistoryDeleteMode,
	llmAPI,
	recordingsAPI,
	tauriAPI,
} from "../lib/tauri";
import { useRecordingPlayer } from "../lib/useRecordingPlayer";
import { HistoryAnalysisPanel } from "./history/HistoryAnalysisPanel";
import {
	HistoryDeleteDialogs,
	type HistoryDeleteOneContext,
} from "./history/HistoryDeleteDialogs";
import { HistoryFeedFilterToolbar } from "./history/HistoryFeedFilterToolbar";
import { HistoryFeedList } from "./history/HistoryFeedList";
import { HistoryFeedPagination } from "./history/HistoryFeedPagination";

export function HistoryFeed({
	onJumpToLog,
}: {
	onJumpToLog?: (logId: string) => void;
} = {}) {
	const queryClient = useQueryClient();
	const recordingsStats = useRecordingsStats();

	const invalidateHistoryQueries = () => {
		queryClient.invalidateQueries({ queryKey: ["history"] });
		queryClient.invalidateQueries({ queryKey: ["historyAll"] });
		queryClient.invalidateQueries({ queryKey: ["historyPage"] });
		queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });
		// Notify other windows about history change
		tauriAPI.emitHistoryChanged();
	};

	const deleteHistoryEntryEx = useMutation({
		mutationFn: async (args: { id: string; mode: HistoryDeleteMode }) =>
			tauriAPI.deleteHistoryEntryEx(args.id, args.mode),
		onSuccess: () => {
			invalidateHistoryQueries();
		},
	});

	const deleteAllHistoryAndRecordings = useMutation({
		mutationFn: async () => {
			const deletedRecordings = await dataAPI.deleteAllRecordings();
			await tauriAPI.clearHistory();
			await tauriAPI.emitHistoryChanged();
			return deletedRecordings;
		},
		onSuccess: (deletedRecordings) => {
			queryClient.invalidateQueries({ queryKey: ["history"] });
			queryClient.invalidateQueries({ queryKey: ["historyAll"] });
			queryClient.invalidateQueries({ queryKey: ["historyPage"] });
			queryClient.invalidateQueries({ queryKey: ["recordingsStats"] });

			notifications.show({
				title: "History",
				message: `Deleted transcripts and ${Number(
					deletedRecordings,
				).toLocaleString()} recording${deletedRecordings === 1 ? "" : "s"}.`,
				color: "green",
			});
		},
		onError: (e) => {
			notifications.show({
				title: "History",
				message: formatErrorMessage(e),
				color: "red",
			});
		},
	});
	const retryMutation = useRetryTranscription();
	const clipboard = useClipboard();

	// Cache whether a recording exists for a given request id (used to decide whether to show Rerun).
	const [recordingExistsById, setRecordingExistsById] = useState<
		Map<string, { exists: boolean; checkedAt: number }>
	>(new Map());

	// Internal tick to allow short polling for newly-created/in-progress entries.
	// Without this, a "missing" cache entry would only re-check when some other state changes.
	const [recordingsProbeTick, setRecordingsProbeTick] = useState(0);

	// Optimistic UI: hide deleted entries immediately, delete in background.
	const [hiddenEntryIds, setHiddenEntryIds] = useState<Set<string>>(
		() => new Set(),
	);

	const hideEntries = (ids: Iterable<string>) => {
		setHiddenEntryIds((prev) => {
			const next = new Set(prev);
			for (const id of ids) next.add(id);
			return next;
		});
	};

	const unhideEntries = (ids: Iterable<string>) => {
		setHiddenEntryIds((prev) => {
			const next = new Set(prev);
			for (const id of ids) next.delete(id);
			return next;
		});
	};

	const [copiedEntryId, setCopiedEntryId] = useState<string | null>(null);
	const copiedTimerRef = useRef<number | null>(null);

	useEffect(() => {
		return () => {
			if (copiedTimerRef.current !== null) {
				window.clearTimeout(copiedTimerRef.current);
				copiedTimerRef.current = null;
			}
		};
	}, []);

	const triggerCopiedFx = (entryId: string) => {
		setCopiedEntryId(entryId);

		if (copiedTimerRef.current !== null) {
			window.clearTimeout(copiedTimerRef.current);
		}

		copiedTimerRef.current = window.setTimeout(() => {
			setCopiedEntryId((cur) => (cur === entryId ? null : cur));
			copiedTimerRef.current = null;
		}, 900);
	};

	const handleCopyEntry = (
		entryId: string,
		text: string | null | undefined,
	) => {
		const value = text?.trim() ?? "";
		if (!value) return;
		clipboard.copy(value);
		triggerCopiedFx(entryId);
	};

	const { data: settings } = useSettings();
	const requestLogsLimit = (() => {
		const fallback = 50;
		const mode = settings?.request_logs_retention_mode;
		if (mode === "amount") {
			const amount = settings?.request_logs_retention_amount;
			if (typeof amount === "number" && Number.isFinite(amount)) {
				return Math.max(1, Math.min(200, Math.floor(amount)));
			}
		}
		return fallback;
	})();
	const { data: requestLogs } = useRequestLogs(requestLogsLimit);
	const requestLogIds = useMemo(
		() => new Set((requestLogs ?? []).map((l) => l.id)),
		[requestLogs],
	);

	const recordingsGbForTooltip = (() => {
		const bytes = recordingsStats.data?.bytes;
		if (typeof bytes !== "number" || !Number.isFinite(bytes)) return null;
		return bytes / 1024 ** 3;
	})();

	const player = useRecordingPlayer({
		onError: (message) => {
			notifications.show({
				title: "Playback",
				message,
				color: "red",
			});
		},
	});

	const isDeleteDialogBusy = deleteAllHistoryAndRecordings.isPending;
	const [confirmOpened, { open: openConfirm, close: closeConfirm }] =
		useDisclosure(false);
	const [analysisOpened, analysisHandlers] = useDisclosure(false);
	const filters = useHistoryFeedFilters();

	// Main view: fetch only the current page (server-side filtering + pagination).
	const { data: historyPage, isLoading, error } = useHistoryPage(filters.query);

	// Lightweight query used only to power the "Retry last failed" quick action.
	// We intentionally *don't* use the current filter state here so the action can
	// work even if the user is filtering the list.
	const retryActionHistoryQuery = useHistoryPage({
		filterText: "",
		showFailed: true,
		showEmptyTranscript: true,
		selectedSttModelKeys: [],
		selectedLlmModelKeys: [],
		page: 1,
		pageSize: 200,
		includeUsageCounts: false,
	});

	const retryLastFailedCandidate = useMemo(() => {
		const items = retryActionHistoryQuery.data?.items ?? [];
		return getRetryLastFailedCandidate(items);
	}, [retryActionHistoryQuery.data?.items]);

	// Optional: fetch full history only when the analysis modal is opened.
	const allHistoryQuery = useHistoryAll({ enabled: analysisOpened });

	const pageHistory = (historyPage?.items ?? []).filter(
		(e) => !hiddenEntryIds.has(e.id),
	);
	const totalHistoryCount = historyPage?.totalAll ?? 0;
	const totalFilteredCount = historyPage?.totalFiltered ?? 0;
	const totalPages = getHistoryPageCount(
		totalFilteredCount,
		historyPage?.pageSize ?? HISTORY_PAGE_SIZE,
	);

	// Keep local page state aligned with backend clamping.
	useEffect(() => {
		filters.syncServerPage(historyPage?.page);
	}, [filters.syncServerPage, historyPage?.page]);

	const [analysisPrompt, setAnalysisPrompt] = useState<string>("");
	const [analysisSystemPrompt, setAnalysisSystemPrompt] = useState<string>("");
	const [analysisUserPrompt, setAnalysisUserPrompt] = useState<string>("");
	const [analysisIncludedCount, setAnalysisIncludedCount] = useState(0);
	const [_analysisTotalCount, setAnalysisTotalCount] = useState(0);
	const [
		analysisAvailableTranscriptsCount,
		setAnalysisAvailableTranscriptsCount,
	] = useState(0);
	const [
		analysisIncludeFromLastHoursInput,
		setAnalysisIncludeFromLastHoursInput,
	] = useState<string | number>("");
	const [analysisPromptStyle, setAnalysisPromptStyle] =
		useState<AnalysisPromptStyle>("productive");

	const [sendDrawerOpened, sendDrawerHandlers] = useDisclosure(false);
	const isNarrow = useMediaQuery("(max-width: 900px)");

	const [deleteOneOpened, deleteOneHandlers] = useDisclosure(false);
	const [deleteOneContext, setDeleteOneContext] =
		useState<HistoryDeleteOneContext | null>(null);
	const [deleteOneBusy, setDeleteOneBusy] = useState(false);

	const { data: llmProviders } = useQuery({
		queryKey: ["llmProviders"],
		queryFn: () => llmAPI.getLlmProviders(),
		staleTime: 60_000,
	});

	const hasAnyLlmProviders = (llmProviders?.length ?? 0) > 0;

	const [sendProvider, setSendProvider] = useState<string | null>(null);
	const [sendModel, setSendModel] = useState<string | null>(null);
	const [sendOutput, setSendOutput] = useState<string>("");
	const [sendProviderUsed, setSendProviderUsed] = useState<string>("");
	const [sendModelUsed, setSendModelUsed] = useState<string>("");

	const sendToLlmMutation = useMutation({
		mutationFn: async (args: {
			provider: string;
			model: string | null;
			systemPrompt: string;
			userPrompt: string;
		}) =>
			llmAPI.complete({
				provider: args.provider,
				model: args.model,
				systemPrompt: args.systemPrompt,
				userPrompt: args.userPrompt,
			}),
	});

	const analysisEstimatedTokens = useMemo(
		() => estimateTokenCount(analysisPrompt),
		[analysisPrompt],
	);

	// Listen for history changes from other windows (e.g., overlay after transcription)
	useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setup = async () => {
			unlisten = await tauriAPI.onHistoryChanged(() => {
				queryClient.invalidateQueries({ queryKey: ["history"] });
				queryClient.invalidateQueries({ queryKey: ["historyAll"] });
				queryClient.invalidateQueries({ queryKey: ["historyPage"] });
			});
		};

		void setup();

		return () => {
			unlisten?.();
		};
	}, [queryClient]);

	const handleDelete = (id: string) => {
		void (async () => {
			try {
				const options = await tauriAPI.getHistoryDeleteOptions(id);

				const recordingId = (options.recording_id ?? "").trim();
				const hasRecording = Boolean(recordingId) && options.recording_exists;
				const refCount = options.recording_ref_count ?? 0;

				// No recording: delete transcript only.
				if (!hasRecording) {
					hideEntries([id]);
					deleteHistoryEntryEx.mutate(
						{ id, mode: "entry_only" },
						{
							onSuccess: () => {
								notifications.show({
									title: "History",
									message: "Deleted transcript.",
									color: "green",
								});
							},
							onError: (e) => {
								unhideEntries([id]);
								notifications.show({
									title: "History",
									message: formatErrorMessage(e),
									color: "red",
								});
							},
						},
					);
					return;
				}

				// Unshared recording: delete transcript + recording immediately.
				if (refCount <= 1) {
					hideEntries([id]);
					deleteHistoryEntryEx.mutate(
						{ id, mode: "entry_and_recording" },
						{
							onSuccess: (res) => {
								notifications.show({
									title: "History",
									message: res.deleted_recording
										? "Deleted transcript and recording."
										: "Deleted transcript.",
									color: "green",
								});
							},
							onError: (e) => {
								unhideEntries([id]);
								notifications.show({
									title: "History",
									message: formatErrorMessage(e),
									color: "red",
								});
							},
						},
					);
					return;
				}

				// Shared recording: ask what to delete.
				setDeleteOneContext({ entryId: id, recordingId, refCount });
				deleteOneHandlers.open();
			} catch (e) {
				notifications.show({
					title: "History",
					message: formatErrorMessage(e),
					color: "red",
				});
			}
		})();
	};

	const handleDeleteAll = () => {
		deleteAllHistoryAndRecordings.mutate(undefined, {
			onSuccess: () => {
				closeConfirm();
			},
		});
	};

	const handleOpenFolder = async () => {
		try {
			await recordingsAPI.openRecordingsFolder();
		} catch (e) {
			notifications.show({
				title: "Recordings",
				message: formatErrorMessage(e),
				color: "red",
			});
		}
	};

	const handleGenerateAnalysisPrompt = () => {
		if (allHistoryQuery.isLoading) {
			notifications.show({
				title: "Analyze transcripts",
				message: "Loading history…",
				color: "gray",
			});
			return;
		}

		const parsedHours =
			typeof analysisIncludeFromLastHoursInput === "number"
				? analysisIncludeFromLastHoursInput
				: analysisIncludeFromLastHoursInput.trim().length > 0
					? Number.parseFloat(analysisIncludeFromLastHoursInput)
					: NaN;
		const includeFromLastHours =
			Number.isFinite(parsedHours) && parsedHours > 0 ? parsedHours : null;

		const {
			prompt,
			systemPrompt,
			userPrompt,
			includedCount,
			totalCount,
			availableTranscriptsCount,
		} = buildAnalysisPrompt(allHistoryQuery.data ?? [], {
			includeFromLastHours,
			style: analysisPromptStyle,
		});
		setAnalysisPrompt(prompt);
		setAnalysisSystemPrompt(systemPrompt);
		setAnalysisUserPrompt(userPrompt);
		setAnalysisIncludedCount(includedCount);
		setAnalysisTotalCount(totalCount);
		setAnalysisAvailableTranscriptsCount(availableTranscriptsCount);
	};

	const sttModelUsageCounts = useMemo(() => {
		const counts = new Map<string, number>();
		for (const item of historyPage?.sttModelUsage ?? []) {
			counts.set(item.key, item.count);
		}
		return counts;
	}, [historyPage?.sttModelUsage]);

	const llmModelUsageCounts = useMemo(() => {
		const counts = new Map<string, number>();
		for (const item of historyPage?.llmModelUsage ?? []) {
			counts.set(item.key, item.count);
		}
		return counts;
	}, [historyPage?.llmModelUsage]);

	const availableSttModelOptions = useMemo(() => listAllSttModelKeys(), []);
	const availableLlmModelOptions = useMemo(() => listAllLlmModelKeys(), []);

	const canGoPrev = filters.page > 1;
	const canGoNext = filters.page < totalPages;

	// Important UX detail:
	// keep the filter input mounted even while the query is refetching,
	// otherwise typing can cause focus loss / flashing.
	const isInitialLoading = isLoading && !historyPage;

	const groupedHistory = useMemo(
		() => groupHistoryForDisplay(pageHistory),
		[pageHistory],
	);
	const emptyState =
		totalFilteredCount === 0
			? getHistoryFeedEmptyState({
					totalHistoryCount,
					isFiltering: filters.isFiltering,
				})
			: null;

	// Probe recordings for currently visible entries (best-effort).
	useEffect(() => {
		void recordingsProbeTick;
		let cancelled = false;

		let timeout: number | null = null;

		const now = Date.now();
		const retryMissingAfterMs = 650;
		const pollIntervalMs = 650;
		const recentWindowMs = 30_000;
		const maxChecksPerTick = 12;

		const isEntryRecentOrInProgress = (entry: {
			timestamp?: string;
			status?: string;
		}) => {
			const status = (entry.status ?? "success").toString();
			if (status === "in_progress") return true;
			if (status === "error") return false;
			const ts = entry.timestamp ? new Date(entry.timestamp).getTime() : NaN;
			return Number.isFinite(ts) ? now - ts < recentWindowMs : false;
		};

		const candidates: Array<{ id: string; priority: number }> = [];
		let shouldPollAgain = false;

		for (const entry of pageHistory) {
			const recordingId =
				(entry.recording_request_id ?? entry.id)?.trim?.() ?? "";
			if (!recordingId) continue;

			const shouldPoll = isEntryRecentOrInProgress(entry);
			const cached = recordingExistsById.get(recordingId);

			if (shouldPoll && (!cached || !cached.exists)) {
				shouldPollAgain = true;
			}

			// Probe on first-seen.
			if (!cached) {
				candidates.push({ id: recordingId, priority: shouldPoll ? 2 : 1 });
				continue;
			}

			// Re-check quickly for recent/in-progress entries when previously missing.
			if (
				shouldPoll &&
				!cached.exists &&
				now - cached.checkedAt > retryMissingAfterMs
			) {
				candidates.push({ id: recordingId, priority: 2 });
			}
		}

		if (shouldPollAgain) {
			timeout = window.setTimeout(() => {
				setRecordingsProbeTick((t) => t + 1);
			}, pollIntervalMs);
		}

		if (candidates.length === 0) {
			return () => {
				cancelled = true;
				if (timeout !== null) window.clearTimeout(timeout);
			};
		}

		// De-dupe + prioritize newer/in-progress checks.
		const seen = new Set<string>();
		const selected: string[] = [];
		candidates
			.sort((a, b) => b.priority - a.priority)
			.forEach((x) => {
				if (seen.has(x.id)) return;
				seen.add(x.id);
				selected.push(x.id);
			});

		const batch = selected.slice(0, maxChecksPerTick);

		void (async () => {
			await Promise.all(
				batch.map(async (id) => {
					try {
						const url = await recordingsAPI.getRecordingAssetUrl({
							requestId: id,
						});
						if (cancelled) return;
						setRecordingExistsById((prev) => {
							const next = new Map(prev);
							next.set(id, { exists: Boolean(url), checkedAt: Date.now() });
							return next;
						});
					} catch {
						// Treat errors as "unknown"; don't force-hide actions.
					}
				}),
			);
		})();

		return () => {
			cancelled = true;
			if (timeout !== null) window.clearTimeout(timeout);
		};
	}, [pageHistory, recordingExistsById, recordingsProbeTick]);

	const canRetryLastFailed = Boolean(retryLastFailedCandidate);
	const retryLastFailedTooltip = canRetryLastFailed
		? "Retry the most recent failed request (copies result)"
		: "No failed requests with saved audio found";
	const openRecordingsTooltip =
		recordingsStats.isLoading || recordingsGbForTooltip === null
			? "Open recordings folder"
			: `Open recordings folder • ${recordingsGbForTooltip.toFixed(2)} GB`;

	const handleRetryLastFailed = () => {
		void (async () => {
			const candidate = retryLastFailedCandidate;
			if (!candidate) return;

			try {
				// Quick guard so we don't kick off an expensive retry if the WAV isn't there.
				const url = await recordingsAPI.getRecordingAssetUrl({
					requestId: candidate.recordingRequestId,
				});
				if (!url) {
					notifications.show({
						title: "Retry",
						message: "No saved audio found for the most recent failed request.",
						color: "yellow",
					});
					return;
				}

				const transcript = await retryMutation.mutateAsync(candidate.entryId);
				clipboard.copy(transcript);
				notifications.show({
					title: "Retry",
					message:
						"Retried the most recent failed request and copied the result.",
					color: "green",
				});
			} catch (e) {
				notifications.show({
					title: "Retry failed",
					message: formatErrorMessage(e),
					color: "red",
				});
			}
		})();
	};

	const handleRetryEntry = (entryId: string) => {
		notifications.show({
			title: "Rerunning",
			message: "Re-running transcription…",
			color: "orange",
		});

		retryMutation.mutate(entryId, {
			onSuccess: () => {
				notifications.show({
					title: "Rerun complete",
					message: "Check History / Request Logs for the new entry.",
					color: "teal",
				});
			},
			onError: (error) => {
				notifications.show({
					title: "Rerun failed",
					message: formatErrorMessage(error),
					color: "red",
				});
			},
		});
	};

	const handleCloseDeleteOneDialog = () => {
		if (deleteOneBusy || deleteHistoryEntryEx.isPending) return;
		deleteOneHandlers.close();
		setDeleteOneContext(null);
	};

	const handleDeleteOnlyThisTranscript = () => {
		if (!deleteOneContext) return;

		setDeleteOneBusy(true);
		hideEntries([deleteOneContext.entryId]);

		deleteHistoryEntryEx.mutate(
			{ id: deleteOneContext.entryId, mode: "entry_only" },
			{
				onSuccess: () => {
					notifications.show({
						title: "History",
						message: "Deleted transcript.",
						color: "green",
					});
					deleteOneHandlers.close();
					setDeleteOneContext(null);
				},
				onError: (error) => {
					unhideEntries([deleteOneContext.entryId]);
					notifications.show({
						title: "History",
						message: formatErrorMessage(error),
						color: "red",
					});
				},
				onSettled: () => setDeleteOneBusy(false),
			},
		);
	};

	const handleDeleteAllUsingRecording = () => {
		if (!deleteOneContext) return;

		setDeleteOneBusy(true);

		const recordingId = deleteOneContext.recordingId;
		const visibleIdsToHide: string[] = [];
		for (const entry of historyPage?.items ?? []) {
			const source = (entry.recording_request_id ?? entry.id)?.trim?.() ?? "";
			if (source && source === recordingId) visibleIdsToHide.push(entry.id);
		}

		const idsToHide =
			visibleIdsToHide.length > 0
				? visibleIdsToHide
				: [deleteOneContext.entryId];
		hideEntries(idsToHide);

		deleteHistoryEntryEx.mutate(
			{
				id: deleteOneContext.entryId,
				mode: "recording_and_all_entries",
			},
			{
				onSuccess: (result) => {
					notifications.show({
						title: "History",
						message: `Deleted ${result.deleted_entries.toLocaleString()} transcript${
							result.deleted_entries === 1 ? "" : "s"
						}${result.deleted_recording ? " and recording" : ""}.`,
						color: "green",
					});
					deleteOneHandlers.close();
					setDeleteOneContext(null);
				},
				onError: (error) => {
					unhideEntries(idsToHide);
					notifications.show({
						title: "History",
						message: formatErrorMessage(error),
						color: "red",
					});
				},
				onSettled: () => setDeleteOneBusy(false),
			},
		);
	};

	const handleOpenSendDrawer = () => {
		if (!analysisSystemPrompt || !analysisUserPrompt) {
			handleGenerateAnalysisPrompt();
		}

		sendDrawerHandlers.open();

		const firstProvider = (llmProviders ?? [])[0];
		setSendProvider((current) => current ?? firstProvider?.id ?? null);
		setSendModel((current) => {
			if (current) return current;
			if (!firstProvider) return null;
			return firstProvider.default_model ?? firstProvider.models?.[0] ?? null;
		});
	};

	const handleSendProviderChange = (providerId: string | null) => {
		setSendProvider(providerId);
		const provider = (llmProviders ?? []).find(
			(item) => item.id === providerId,
		);
		setSendModel(provider?.default_model ?? provider?.models?.[0] ?? null);
	};

	const handleGenerateSendOutput = () => {
		void (async () => {
			const provider = sendProvider ?? "";
			if (!provider) {
				notifications.show({
					title: "Send to LLM",
					message: "Select a provider.",
					color: "red",
				});
				return;
			}

			if (!analysisSystemPrompt || !analysisUserPrompt) {
				handleGenerateAnalysisPrompt();
			}

			if (!analysisUserPrompt.trim()) {
				notifications.show({
					title: "Send to LLM",
					message:
						"No transcripts matched the filter. Try a larger hour window, or record more.",
					color: "red",
				});
				return;
			}

			try {
				const result = await sendToLlmMutation.mutateAsync({
					provider,
					model: sendModel ?? null,
					systemPrompt: analysisSystemPrompt,
					userPrompt: analysisUserPrompt,
				});
				setSendOutput(result.output);
				setSendProviderUsed(result.provider_used);
				setSendModelUsed(result.model_used);
			} catch (error) {
				notifications.show({
					title: "Send to LLM",
					message: formatErrorMessage(error),
					color: "red",
				});
			}
		})();
	};

	const retryPendingEntryId =
		typeof retryMutation.variables === "string"
			? retryMutation.variables
			: undefined;
	const deleteOnlyThisTranscriptLoading =
		deleteOneBusy && deleteHistoryEntryEx.variables?.mode === "entry_only";
	const deleteAllUsingRecordingLoading =
		deleteOneBusy &&
		deleteHistoryEntryEx.variables?.mode === "recording_and_all_entries";
	const deleteOneActionsDisabled =
		deleteOneBusy || deleteHistoryEntryEx.isPending;

	return (
		<div className="animate-in animate-in-delay-2">
			<HistoryFeedFilterToolbar
				retryLastFailedTooltip={retryLastFailedTooltip}
				onRetryLastFailed={handleRetryLastFailed}
				canRetryLastFailed={canRetryLastFailed}
				isRetryPending={retryMutation.isPending}
				openRecordingsTooltip={openRecordingsTooltip}
				onOpenRecordingsFolder={handleOpenFolder}
				onOpenAnalysis={analysisHandlers.open}
				onOpenDeleteAll={openConfirm}
				isDeleteAllPending={deleteAllHistoryAndRecordings.isPending}
				filterText={filters.filterText}
				onFilterTextChange={filters.setFilterText}
				onClearFilter={() => filters.setFilterText("")}
				filtersOpened={filters.filtersOpened}
				onFiltersOpenedChange={filters.setFiltersOpened}
				onToggleFilters={filters.toggleFiltersOpened}
				hasActiveFilters={filters.hasActiveFilters}
				onResetFilters={filters.resetFilters}
				showFailed={filters.showFailed}
				onShowFailedChange={filters.setShowFailed}
				showEmptyTranscript={filters.showEmptyTranscript}
				onShowEmptyTranscriptChange={filters.setShowEmptyTranscript}
				filtersExpandedSection={filters.filtersExpandedSection}
				onFiltersExpandedSectionChange={filters.setFiltersExpandedSection}
				availableSttModelOptions={availableSttModelOptions}
				sttModelUsageCounts={sttModelUsageCounts}
				selectedSttModelKeys={filters.selectedSttModelKeys}
				onSelectedSttModelKeysChange={filters.setSelectedSttModelKeys}
				availableLlmModelOptions={availableLlmModelOptions}
				llmModelUsageCounts={llmModelUsageCounts}
				selectedLlmModelKeys={filters.selectedLlmModelKeys}
				onSelectedLlmModelKeysChange={filters.setSelectedLlmModelKeys}
				totalFilteredCount={totalFilteredCount}
				pagination={
					<HistoryFeedPagination
						canGoPrev={canGoPrev}
						canGoNext={canGoNext}
						onFirstPage={() => filters.setPage(1)}
						onPreviousPage={() =>
							filters.setPage((current) => Math.max(1, current - 1))
						}
						onNextPage={() =>
							filters.setPage((current) => Math.min(totalPages, current + 1))
						}
						onLastPage={() => filters.setPage(totalPages)}
					/>
				}
			/>

			<HistoryDeleteDialogs
				confirmOpened={confirmOpened}
				onCloseConfirm={() => {
					if (isDeleteDialogBusy) return;
					closeConfirm();
				}}
				onDeleteAll={handleDeleteAll}
				isDeleteAllPending={deleteAllHistoryAndRecordings.isPending}
				deleteOneOpened={deleteOneOpened}
				onCloseDeleteOne={handleCloseDeleteOneDialog}
				deleteOneContext={deleteOneContext}
				disableDeleteOneActions={deleteOneActionsDisabled}
				deleteOnlyThisTranscriptLoading={deleteOnlyThisTranscriptLoading}
				deleteAllUsingRecordingLoading={deleteAllUsingRecordingLoading}
				onDeleteOnlyThisTranscript={handleDeleteOnlyThisTranscript}
				onDeleteAllUsingRecording={handleDeleteAllUsingRecording}
			/>

			<HistoryAnalysisPanel
				analysisOpened={analysisOpened}
				onCloseAnalysis={analysisHandlers.close}
				analysisIncludedCount={analysisIncludedCount}
				analysisEstimatedTokens={analysisEstimatedTokens}
				analysisAvailableTranscriptsCount={analysisAvailableTranscriptsCount}
				analysisIncludeFromLastHoursInput={analysisIncludeFromLastHoursInput}
				onAnalysisIncludeFromLastHoursInputChange={
					setAnalysisIncludeFromLastHoursInput
				}
				analysisPromptStyle={analysisPromptStyle}
				onAnalysisPromptStyleChange={setAnalysisPromptStyle}
				onGenerateAnalysisPrompt={handleGenerateAnalysisPrompt}
				isAnalysisLoading={analysisOpened && allHistoryQuery.isLoading}
				analysisPrompt={analysisPrompt}
				onAnalysisPromptChange={setAnalysisPrompt}
				onCopyAnalysisPrompt={() => clipboard.copy(analysisPrompt)}
				hasAnyLlmProviders={hasAnyLlmProviders}
				onOpenSendDrawer={handleOpenSendDrawer}
				sendDrawerOpened={sendDrawerOpened}
				onCloseSendDrawer={sendDrawerHandlers.close}
				isNarrow={Boolean(isNarrow)}
				llmProviders={llmProviders}
				sendProvider={sendProvider}
				onSendProviderChange={handleSendProviderChange}
				sendModel={sendModel}
				onSendModelChange={setSendModel}
				sendProviderUsed={sendProviderUsed}
				sendModelUsed={sendModelUsed}
				onGenerateSendOutput={handleGenerateSendOutput}
				isSendPending={sendToLlmMutation.isPending}
				sendOutput={sendOutput}
				onSendOutputChange={setSendOutput}
				onCopySendOutput={() => clipboard.copy(sendOutput)}
			/>

			<HistoryFeedList
				isInitialLoading={isInitialLoading}
				hasError={Boolean(error)}
				emptyState={emptyState}
				groupedHistory={groupedHistory}
				copiedEntryId={copiedEntryId}
				onCopyEntry={handleCopyEntry}
				onRetryEntry={handleRetryEntry}
				isRetryPending={retryMutation.isPending}
				retryPendingEntryId={retryPendingEntryId}
				recordingExistsById={recordingExistsById}
				isRecordingPlaying={(recordingId) => player.isPlaying(recordingId)}
				isRecordingLoading={(recordingId) => player.isLoading(recordingId)}
				onToggleRecording={(recordingId) => {
					void player.toggle(recordingId);
				}}
				requestLogIds={requestLogIds}
				onJumpToLog={onJumpToLog}
				onDeleteEntry={handleDelete}
				isDeleteDisabled={deleteHistoryEntryEx.isPending || deleteOneBusy}
			/>
		</div>
	);
}
