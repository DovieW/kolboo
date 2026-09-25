import { useEffect, useMemo, useRef, useState } from "react";
import type {
	HistoryDeleteMode,
	HistoryDeleteOptions,
	HistoryDeleteResult,
	HistoryEntry,
} from "../tauri";

export interface HistoryDeleteOneContext {
	entryId: string;
	recordingId: string;
	refCount: number;
}

export type RetryLastFailedCandidate = {
	// The history entry id we should retry from.
	entryId: string;
	// The request id that likely owns the WAV recording.
	recordingRequestId: string;
};

export type HistoryDeletePlan =
	| {
			kind: "delete_entry_only";
			mode: "entry_only";
	  }
	| {
			kind: "delete_entry_and_recording";
			mode: "entry_and_recording";
	  }
	| {
			kind: "confirm_shared_recording";
			context: HistoryDeleteOneContext;
	  };

export type RetryLastFailedActionState = {
	canRetry: boolean;
	tooltip: string;
};

export type RetryLastFailedOutcome =
	| { kind: "no_candidate" }
	| { kind: "missing_recording"; candidate: RetryLastFailedCandidate }
	| {
			kind: "retried";
			candidate: RetryLastFailedCandidate;
			transcript: string;
	  };

export type RequestDeleteEntryOutcome =
	| { kind: "ignored" }
	| { kind: "opened_shared_dialog"; context: HistoryDeleteOneContext }
	| { kind: "deleted_entry"; result: HistoryDeleteResult }
	| { kind: "deleted_entry_and_recording"; result: HistoryDeleteResult };

export type DeleteOneTranscriptOutcome =
	| { kind: "no_context" }
	| {
			kind: "deleted_entry";
			context: HistoryDeleteOneContext;
			result: HistoryDeleteResult;
	  };

export type DeleteAllUsingRecordingOutcome =
	| { kind: "no_context" }
	| {
			kind: "deleted_recording_and_all_entries";
			context: HistoryDeleteOneContext;
			result: HistoryDeleteResult;
			hiddenEntryIds: string[];
	  };

type UseHistoryFeedOrchestrationArgs = {
	pageEntries: HistoryEntry[];
	retryActionEntries: HistoryEntry[];
	copyToClipboard: (value: string) => void;
	getRecordingAssetUrl: (requestId: string) => Promise<string | null>;
	getDeleteOptions: (entryId: string) => Promise<HistoryDeleteOptions>;
	deleteHistoryEntry: (args: {
		id: string;
		mode: HistoryDeleteMode;
	}) => Promise<HistoryDeleteResult>;
	retryEntry: (entryId: string) => Promise<string>;
};

export const COPIED_ENTRY_FEEDBACK_MS = 900;

function trimOrNull(value: string | null | undefined): string | null {
	const trimmed = (value ?? "").trim();
	return trimmed.length > 0 ? trimmed : null;
}

export function getHistoryEntryRecordingRequestId(
	entry: Pick<HistoryEntry, "id" | "recording_request_id">,
): string | null {
	return trimOrNull(entry.recording_request_id) ?? trimOrNull(entry.id);
}

export function getRetryLastFailedCandidate(
	entries: HistoryEntry[],
): RetryLastFailedCandidate | null {
	// Prefer error entries that *explicitly* point at a recording.
	for (const entry of entries) {
		if (entry.status !== "error") continue;

		const entryId = trimOrNull(entry.id);
		if (!entryId) continue;

		const recordingRequestId = trimOrNull(entry.recording_request_id);
		if (!recordingRequestId) continue;

		return { entryId, recordingRequestId };
	}

	// Fallback: some older entries might not have `recording_request_id` populated even
	// though a WAV exists under the entry id.
	for (const entry of entries) {
		if (entry.status !== "error") continue;

		const entryId = trimOrNull(entry.id);
		if (!entryId) continue;

		return { entryId, recordingRequestId: entryId };
	}

	return null;
}

export function getRetryLastFailedActionState(
	candidate: RetryLastFailedCandidate | null,
): RetryLastFailedActionState {
	return candidate
		? {
				canRetry: true,
				tooltip: "Retry the most recent failed request (copies result)",
			}
		: {
				canRetry: false,
				tooltip: "No failed requests with saved audio found",
			};
}

export function addHiddenHistoryEntryIds(
	prev: Set<string>,
	ids: Iterable<string>,
): Set<string> {
	const next = new Set(prev);
	for (const id of ids) {
		const trimmed = trimOrNull(id);
		if (trimmed) next.add(trimmed);
	}
	return next;
}

export function removeHiddenHistoryEntryIds(
	prev: Set<string>,
	ids: Iterable<string>,
): Set<string> {
	const next = new Set(prev);
	for (const id of ids) {
		const trimmed = trimOrNull(id);
		if (trimmed) next.delete(trimmed);
	}
	return next;
}

export function filterVisibleHistoryEntries(
	entries: HistoryEntry[],
	hiddenEntryIds: Set<string>,
): HistoryEntry[] {
	return entries.filter((entry) => !hiddenEntryIds.has(entry.id));
}

export function classifyHistoryDeleteOptions(
	entryId: string,
	options: HistoryDeleteOptions,
): HistoryDeletePlan {
	const recordingId = trimOrNull(options.recording_id);
	const hasRecording = Boolean(recordingId) && options.recording_exists;
	const refCount = Math.max(0, Math.floor(options.recording_ref_count ?? 0));

	if (!hasRecording) {
		return {
			kind: "delete_entry_only",
			mode: "entry_only",
		};
	}

	if (refCount <= 1) {
		return {
			kind: "delete_entry_and_recording",
			mode: "entry_and_recording",
		};
	}

	return {
		kind: "confirm_shared_recording",
		context: {
			entryId,
			recordingId: recordingId as string,
			refCount,
		},
	};
}

export function collectHistoryEntryIdsUsingRecording(
	entries: Array<Pick<HistoryEntry, "id" | "recording_request_id">>,
	recordingId: string,
	fallbackEntryId: string,
): string[] {
	const normalizedRecordingId = trimOrNull(recordingId);
	const fallbackId = trimOrNull(fallbackEntryId);
	if (!normalizedRecordingId) {
		return fallbackId ? [fallbackId] : [];
	}

	const ids: string[] = [];
	const seen = new Set<string>();

	for (const entry of entries) {
		if (getHistoryEntryRecordingRequestId(entry) !== normalizedRecordingId) {
			continue;
		}

		const entryId = trimOrNull(entry.id);
		if (!entryId || seen.has(entryId)) continue;

		seen.add(entryId);
		ids.push(entryId);
	}

	if (ids.length > 0) {
		return ids;
	}

	return fallbackId ? [fallbackId] : [];
}

export function useHistoryFeedOrchestration({
	pageEntries,
	retryActionEntries,
	copyToClipboard,
	getRecordingAssetUrl,
	getDeleteOptions,
	deleteHistoryEntry,
	retryEntry,
}: UseHistoryFeedOrchestrationArgs) {
	const [hiddenEntryIds, setHiddenEntryIds] = useState<Set<string>>(
		() => new Set(),
	);
	const [copiedEntryId, setCopiedEntryId] = useState<string | null>(null);
	const copiedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const [deleteOneOpened, setDeleteOneOpened] = useState(false);
	const [deleteOneContext, setDeleteOneContext] =
		useState<HistoryDeleteOneContext | null>(null);
	const [deleteOneBusy, setDeleteOneBusy] = useState(false);
	const deleteBusyRef = useRef(false);
	const deleteContextRef = useRef<HistoryDeleteOneContext | null>(null);

	const pageHistory = useMemo(
		() => filterVisibleHistoryEntries(pageEntries, hiddenEntryIds),
		[pageEntries, hiddenEntryIds],
	);

	const retryLastFailedCandidate = useMemo(
		() => getRetryLastFailedCandidate(retryActionEntries),
		[retryActionEntries],
	);
	const retryLastFailedAction = useMemo(
		() => getRetryLastFailedActionState(retryLastFailedCandidate),
		[retryLastFailedCandidate],
	);

	useEffect(() => {
		return () => {
			if (copiedTimerRef.current !== null) {
				clearTimeout(copiedTimerRef.current);
				copiedTimerRef.current = null;
			}
		};
	}, []);

	const hideEntries = (ids: Iterable<string>) => {
		setHiddenEntryIds((prev) => addHiddenHistoryEntryIds(prev, ids));
	};

	const unhideEntries = (ids: Iterable<string>) => {
		setHiddenEntryIds((prev) => removeHiddenHistoryEntryIds(prev, ids));
	};

	const handleCopyEntry = (
		entryId: string,
		text: string | null | undefined,
	) => {
		const value = text?.trim() ?? "";
		if (!value) return;

		copyToClipboard(value);
		setCopiedEntryId(entryId);

		if (copiedTimerRef.current !== null) {
			clearTimeout(copiedTimerRef.current);
		}

		copiedTimerRef.current = setTimeout(() => {
			setCopiedEntryId((current) => (current === entryId ? null : current));
			copiedTimerRef.current = null;
		}, COPIED_ENTRY_FEEDBACK_MS);
	};

	const retryLastFailed = async (): Promise<RetryLastFailedOutcome> => {
		const candidate = retryLastFailedCandidate;
		if (!candidate) {
			return { kind: "no_candidate" };
		}

		const url = await getRecordingAssetUrl(candidate.recordingRequestId);
		if (!url) {
			return { kind: "missing_recording", candidate };
		}

		const transcript = await retryEntry(candidate.entryId);
		copyToClipboard(transcript);

		return {
			kind: "retried",
			candidate,
			transcript,
		};
	};

	const requestDeleteEntry = async (
		entryId: string,
	): Promise<RequestDeleteEntryOutcome> => {
		if (deleteBusyRef.current || deleteContextRef.current)
			return { kind: "ignored" };
		deleteBusyRef.current = true;
		setDeleteOneBusy(true);
		try {
			const options = await getDeleteOptions(entryId);
			const plan = classifyHistoryDeleteOptions(entryId, options);

			if (plan.kind === "confirm_shared_recording") {
				deleteContextRef.current = plan.context;
				setDeleteOneContext(plan.context);
				setDeleteOneOpened(true);

				return {
					kind: "opened_shared_dialog",
					context: plan.context,
				};
			}

			hideEntries([entryId]);

			try {
				const result = await deleteHistoryEntry({
					id: entryId,
					mode: plan.mode,
				});

				return plan.kind === "delete_entry_and_recording"
					? { kind: "deleted_entry_and_recording", result }
					: { kind: "deleted_entry", result };
			} catch (error) {
				unhideEntries([entryId]);
				throw error;
			}
		} finally {
			deleteBusyRef.current = false;
			setDeleteOneBusy(false);
		}
	};

	const closeDeleteOneDialog = () => {
		if (deleteBusyRef.current) return;

		deleteContextRef.current = null;
		setDeleteOneOpened(false);
		setDeleteOneContext(null);
	};

	const deleteOnlyThisTranscript =
		async (): Promise<DeleteOneTranscriptOutcome> => {
			const context = deleteContextRef.current;
			if (!context || deleteBusyRef.current) {
				return { kind: "no_context" };
			}

			deleteBusyRef.current = true;
			setDeleteOneBusy(true);
			hideEntries([context.entryId]);

			try {
				const result = await deleteHistoryEntry({
					id: context.entryId,
					mode: "entry_only",
				});

				deleteContextRef.current = null;
				setDeleteOneOpened(false);
				setDeleteOneContext(null);

				return { kind: "deleted_entry", context, result };
			} catch (error) {
				unhideEntries([context.entryId]);
				throw error;
			} finally {
				deleteBusyRef.current = false;
				setDeleteOneBusy(false);
			}
		};

	const deleteAllUsingRecording =
		async (): Promise<DeleteAllUsingRecordingOutcome> => {
			const context = deleteContextRef.current;
			if (!context || deleteBusyRef.current) {
				return { kind: "no_context" };
			}

			deleteBusyRef.current = true;
			setDeleteOneBusy(true);

			const idsToHide = collectHistoryEntryIdsUsingRecording(
				pageEntries,
				context.recordingId,
				context.entryId,
			);
			hideEntries(idsToHide);

			try {
				const result = await deleteHistoryEntry({
					id: context.entryId,
					mode: "recording_and_all_entries",
				});

				deleteContextRef.current = null;
				setDeleteOneOpened(false);
				setDeleteOneContext(null);

				return {
					kind: "deleted_recording_and_all_entries",
					context,
					result,
					hiddenEntryIds: idsToHide,
				};
			} catch (error) {
				unhideEntries(idsToHide);
				throw error;
			} finally {
				deleteBusyRef.current = false;
				setDeleteOneBusy(false);
			}
		};

	return {
		copiedEntryId,
		handleCopyEntry,
		pageHistory,
		retryLastFailedCandidate,
		canRetryLastFailed: retryLastFailedAction.canRetry,
		retryLastFailedTooltip: retryLastFailedAction.tooltip,
		retryLastFailed,
		requestDeleteEntry,
		deleteOneOpened,
		deleteOneContext,
		deleteOneBusy,
		closeDeleteOneDialog,
		deleteOnlyThisTranscript,
		deleteAllUsingRecording,
	};
}
