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

export interface RecordingExistenceProbeState {
	exists: boolean;
	checkedAt: number;
}

export type RecordingExistenceById = Map<string, RecordingExistenceProbeState>;

export interface RecordingProbePlan {
	batch: string[];
	shouldPollAgain: boolean;
}

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

const RECENT_RECORDING_WINDOW_MS = 30_000;
const RETRY_MISSING_RECORDING_AFTER_MS = 650;
const RECORDING_PROBE_POLL_INTERVAL_MS = 650;
const MAX_RECORDING_PROBES_PER_TICK = 12;
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

function isRecentOrInProgressHistoryEntry(
	entry: Pick<HistoryEntry, "timestamp" | "status">,
	now: number,
): boolean {
	const status = (entry.status ?? "success").toString();
	if (status === "in_progress") return true;
	if (status === "error") return false;

	const timestampMs = entry.timestamp
		? new Date(entry.timestamp).getTime()
		: Number.NaN;
	return Number.isFinite(timestampMs)
		? now - timestampMs < RECENT_RECORDING_WINDOW_MS
		: false;
}

export function buildRecordingProbePlan(
	entries: Array<
		Pick<HistoryEntry, "id" | "recording_request_id" | "timestamp" | "status">
	>,
	recordingExistsById: RecordingExistenceById,
	now = Date.now(),
): RecordingProbePlan {
	const candidates: Array<{ id: string; priority: number; order: number }> = [];
	let shouldPollAgain = false;

	for (const [index, entry] of entries.entries()) {
		const recordingId = getHistoryEntryRecordingRequestId(entry);
		if (!recordingId) continue;

		const shouldPoll = isRecentOrInProgressHistoryEntry(entry, now);
		const cached = recordingExistsById.get(recordingId);

		if (shouldPoll && (!cached || !cached.exists)) {
			shouldPollAgain = true;
		}

		// Probe immediately the first time an entry becomes visible.
		if (!cached) {
			candidates.push({
				id: recordingId,
				priority: shouldPoll ? 2 : 1,
				order: index,
			});
			continue;
		}

		// Keep polling a little for recent/in-progress entries when the WAV may not exist yet.
		if (
			shouldPoll &&
			!cached.exists &&
			now - cached.checkedAt > RETRY_MISSING_RECORDING_AFTER_MS
		) {
			candidates.push({ id: recordingId, priority: 2, order: index });
		}
	}

	const selected: string[] = [];
	const seen = new Set<string>();

	candidates
		.sort((a, b) => b.priority - a.priority || a.order - b.order)
		.forEach((candidate) => {
			if (seen.has(candidate.id)) return;
			seen.add(candidate.id);
			selected.push(candidate.id);
		});

	return {
		batch: selected.slice(0, MAX_RECORDING_PROBES_PER_TICK),
		shouldPollAgain,
	};
}

export function setRecordingProbeResult(
	prev: RecordingExistenceById,
	id: string,
	exists: boolean,
	checkedAt = Date.now(),
): RecordingExistenceById {
	const next = new Map(prev);
	next.set(id, { exists, checkedAt });
	return next;
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
	const [recordingExistsById, setRecordingExistsById] =
		useState<RecordingExistenceById>(() => new Map());
	const [recordingsProbeTick, setRecordingsProbeTick] = useState(0);
	const [hiddenEntryIds, setHiddenEntryIds] = useState<Set<string>>(
		() => new Set(),
	);
	const [copiedEntryId, setCopiedEntryId] = useState<string | null>(null);
	const copiedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const [deleteOneOpened, setDeleteOneOpened] = useState(false);
	const [deleteOneContext, setDeleteOneContext] =
		useState<HistoryDeleteOneContext | null>(null);
	const [deleteOneBusy, setDeleteOneBusy] = useState(false);

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

	useEffect(() => {
		void recordingsProbeTick;

		let cancelled = false;
		let timeout: ReturnType<typeof setTimeout> | null = null;

		const { batch, shouldPollAgain } = buildRecordingProbePlan(
			pageHistory,
			recordingExistsById,
		);

		if (shouldPollAgain) {
			timeout = setTimeout(() => {
				setRecordingsProbeTick((tick) => tick + 1);
			}, RECORDING_PROBE_POLL_INTERVAL_MS);
		}

		if (batch.length === 0) {
			return () => {
				cancelled = true;
				if (timeout !== null) clearTimeout(timeout);
			};
		}

		void (async () => {
			await Promise.all(
				batch.map(async (recordingId) => {
					try {
						const url = await getRecordingAssetUrl(recordingId);
						if (cancelled) return;

						setRecordingExistsById((prev) =>
							setRecordingProbeResult(prev, recordingId, Boolean(url)),
						);
					} catch {
						// Treat errors as "unknown" so the UI does not permanently hide
						// playback/rerun actions because of a transient lookup failure.
					}
				}),
			);
		})();

		return () => {
			cancelled = true;
			if (timeout !== null) clearTimeout(timeout);
		};
	}, [
		pageHistory,
		recordingExistsById,
		recordingsProbeTick,
		getRecordingAssetUrl,
	]);

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
		const options = await getDeleteOptions(entryId);
		const plan = classifyHistoryDeleteOptions(entryId, options);

		if (plan.kind === "confirm_shared_recording") {
			setDeleteOneContext(plan.context);
			setDeleteOneOpened(true);

			return {
				kind: "opened_shared_dialog",
				context: plan.context,
			};
		}

		hideEntries([entryId]);

		try {
			const result = await deleteHistoryEntry({ id: entryId, mode: plan.mode });

			return plan.kind === "delete_entry_and_recording"
				? { kind: "deleted_entry_and_recording", result }
				: { kind: "deleted_entry", result };
		} catch (error) {
			unhideEntries([entryId]);
			throw error;
		}
	};

	const closeDeleteOneDialog = () => {
		if (deleteOneBusy) return;

		setDeleteOneOpened(false);
		setDeleteOneContext(null);
	};

	const deleteOnlyThisTranscript =
		async (): Promise<DeleteOneTranscriptOutcome> => {
			const context = deleteOneContext;
			if (!context) {
				return { kind: "no_context" };
			}

			setDeleteOneBusy(true);
			hideEntries([context.entryId]);

			try {
				const result = await deleteHistoryEntry({
					id: context.entryId,
					mode: "entry_only",
				});

				setDeleteOneOpened(false);
				setDeleteOneContext(null);

				return { kind: "deleted_entry", context, result };
			} catch (error) {
				unhideEntries([context.entryId]);
				throw error;
			} finally {
				setDeleteOneBusy(false);
			}
		};

	const deleteAllUsingRecording =
		async (): Promise<DeleteAllUsingRecordingOutcome> => {
			const context = deleteOneContext;
			if (!context) {
				return { kind: "no_context" };
			}

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
				setDeleteOneBusy(false);
			}
		};

	return {
		copiedEntryId,
		handleCopyEntry,
		recordingExistsById,
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
