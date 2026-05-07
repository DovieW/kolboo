import { useCallback, useEffect, useMemo, useState } from "react";
import type { HistoryPageQuery } from "../tauri/types";
import {
	HISTORY_PAGE_SIZE,
	type PersistedHistoryFilters,
	readPersistedHistoryFilters,
	writePersistedHistoryFilters,
} from "./readModel";

export type HistoryFeedFilterSection = "stt" | "llm";

export function createDefaultHistoryFeedFilters(): PersistedHistoryFilters {
	return {
		filterText: "",
		showFailed: true,
		showEmptyTranscript: false,
		selectedSttModelKeys: [],
		selectedLlmModelKeys: [],
	};
}

export function hasActiveHistoryFilters(
	filters: Pick<
		PersistedHistoryFilters,
		| "showFailed"
		| "showEmptyTranscript"
		| "selectedSttModelKeys"
		| "selectedLlmModelKeys"
	>,
): boolean {
	return (
		!filters.showFailed ||
		filters.showEmptyTranscript ||
		filters.selectedSttModelKeys.length > 0 ||
		filters.selectedLlmModelKeys.length > 0
	);
}

export function createPersistedHistoryFilterSnapshot(
	filters: PersistedHistoryFilters,
): PersistedHistoryFilters {
	return {
		filterText: filters.filterText,
		showFailed: filters.showFailed,
		showEmptyTranscript: filters.showEmptyTranscript,
		// Copy arrays so persistence snapshots and query inputs do not share
		// mutable references across state boundaries.
		selectedSttModelKeys: [...filters.selectedSttModelKeys],
		selectedLlmModelKeys: [...filters.selectedLlmModelKeys],
	};
}

export function getHistoryPageResetKey(
	filters: PersistedHistoryFilters,
): string {
	return JSON.stringify(createPersistedHistoryFilterSnapshot(filters));
}

export function normalizeHistoryServerPage(
	serverPage: number | undefined,
): number | null {
	if (typeof serverPage !== "number" || !Number.isFinite(serverPage)) {
		return null;
	}

	return Math.max(1, Math.floor(serverPage));
}

export function buildHistoryPageQuery(
	args: PersistedHistoryFilters & {
		page: number;
		pageSize?: number;
		includeUsageCounts?: boolean;
	},
): HistoryPageQuery {
	return {
		filterText: args.filterText,
		showFailed: args.showFailed,
		showEmptyTranscript: args.showEmptyTranscript,
		selectedSttModelKeys: [...args.selectedSttModelKeys],
		selectedLlmModelKeys: [...args.selectedLlmModelKeys],
		page: Math.max(1, Math.floor(args.page)),
		pageSize: Math.max(1, Math.floor(args.pageSize ?? HISTORY_PAGE_SIZE)),
		includeUsageCounts: args.includeUsageCounts ?? true,
	};
}

export function getHistoryPageCount(
	totalFilteredCount: number,
	pageSize = HISTORY_PAGE_SIZE,
): number {
	const safeTotal = Number.isFinite(totalFilteredCount)
		? Math.max(0, Math.floor(totalFilteredCount))
		: 0;
	const safePageSize = Math.max(1, Math.floor(pageSize));
	return Math.max(1, Math.ceil(safeTotal / safePageSize));
}

export type HistoryFeedFiltersController = {
	filterText: string;
	setFilterText: (value: string) => void;
	showFailed: boolean;
	setShowFailed: (value: boolean) => void;
	showEmptyTranscript: boolean;
	setShowEmptyTranscript: (value: boolean) => void;
	selectedSttModelKeys: string[];
	setSelectedSttModelKeys: (keys: string[]) => void;
	selectedLlmModelKeys: string[];
	setSelectedLlmModelKeys: (keys: string[]) => void;
	filtersOpened: boolean;
	setFiltersOpened: (opened: boolean) => void;
	toggleFiltersOpened: () => void;
	filtersExpandedSection: HistoryFeedFilterSection | null;
	setFiltersExpandedSection: (section: HistoryFeedFilterSection | null) => void;
	page: number;
	setPage: (update: number | ((page: number) => number)) => void;
	query: HistoryPageQuery;
	hasActiveFilters: boolean;
	isFiltering: boolean;
	resetFilters: () => void;
	syncServerPage: (page: number | undefined) => void;
};

export function useHistoryFeedFilters(): HistoryFeedFiltersController {
	const defaults = useMemo(() => createDefaultHistoryFeedFilters(), []);
	const [filterText, setFilterText] = useState(defaults.filterText);
	const [showFailed, setShowFailed] = useState(defaults.showFailed);
	const [showEmptyTranscript, setShowEmptyTranscript] = useState(
		defaults.showEmptyTranscript,
	);
	const [selectedSttModelKeys, setSelectedSttModelKeys] = useState<string[]>(
		defaults.selectedSttModelKeys,
	);
	const [selectedLlmModelKeys, setSelectedLlmModelKeys] = useState<string[]>(
		defaults.selectedLlmModelKeys,
	);
	const [filtersOpened, setFiltersOpened] = useState(false);
	const [filtersExpandedSection, setFiltersExpandedSection] =
		useState<HistoryFeedFilterSection | null>(null);
	const [page, setPageState] = useState(1);
	const [hasHydratedPersistedFilters, setHasHydratedPersistedFilters] =
		useState(false);

	const setPage = useCallback((update: number | ((page: number) => number)) => {
		setPageState((current) => {
			const next = typeof update === "function" ? update(current) : update;
			return Math.max(1, Math.floor(next));
		});
	}, []);

	const resetFilters = useCallback(() => {
		setShowFailed(defaults.showFailed);
		setShowEmptyTranscript(defaults.showEmptyTranscript);
		setSelectedSttModelKeys([]);
		setSelectedLlmModelKeys([]);
	}, [defaults.showEmptyTranscript, defaults.showFailed]);

	const toggleFiltersOpened = useCallback(() => {
		setFiltersOpened((opened) => !opened);
	}, []);

	const syncServerPage = useCallback((serverPage: number | undefined) => {
		const normalizedServerPage = normalizeHistoryServerPage(serverPage);
		if (normalizedServerPage == null) {
			return;
		}

		setPageState((current) =>
			current === normalizedServerPage ? current : normalizedServerPage,
		);
	}, []);

	const hasActiveFilters = useMemo(
		() =>
			hasActiveHistoryFilters({
				showFailed,
				showEmptyTranscript,
				selectedSttModelKeys,
				selectedLlmModelKeys,
			}),
		[
			showFailed,
			showEmptyTranscript,
			selectedSttModelKeys,
			selectedLlmModelKeys,
		],
	);

	const isFiltering = filterText.trim().length > 0 || hasActiveFilters;
	const persistedFilters = useMemo(
		() =>
			createPersistedHistoryFilterSnapshot({
				filterText,
				showFailed,
				showEmptyTranscript,
				selectedSttModelKeys,
				selectedLlmModelKeys,
			}),
		[
			filterText,
			showFailed,
			showEmptyTranscript,
			selectedSttModelKeys,
			selectedLlmModelKeys,
		],
	);

	const query = useMemo(
		() =>
			buildHistoryPageQuery({
				filterText,
				showFailed,
				showEmptyTranscript,
				selectedSttModelKeys,
				selectedLlmModelKeys,
				page,
				pageSize: HISTORY_PAGE_SIZE,
				includeUsageCounts: true,
			}),
		[
			filterText,
			showFailed,
			showEmptyTranscript,
			selectedSttModelKeys,
			selectedLlmModelKeys,
			page,
		],
	);
	const pageResetSignature = useMemo(
		() => getHistoryPageResetKey(persistedFilters),
		[persistedFilters],
	);

	useEffect(() => {
		let cancelled = false;

		const hydrate = async () => {
			try {
				const normalized = await readPersistedHistoryFilters();

				if (!normalized || cancelled) return;

				setFilterText(normalized.filterText);
				setShowFailed(normalized.showFailed);
				setShowEmptyTranscript(normalized.showEmptyTranscript);
				setSelectedSttModelKeys(normalized.selectedSttModelKeys);
				setSelectedLlmModelKeys(normalized.selectedLlmModelKeys);
			} catch (error) {
				// Persistence is a convenience layer; never block the History tab on it.
				console.warn("Failed to hydrate history filters:", error);
			} finally {
				if (!cancelled) setHasHydratedPersistedFilters(true);
			}
		};

		void hydrate();

		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => {
		if (!hasHydratedPersistedFilters) return;

		const timeout = window.setTimeout(() => {
			const persist = async () => {
				try {
					await writePersistedHistoryFilters(persistedFilters);
				} catch (error) {
					console.warn("Failed to persist history filters:", error);
				}
			};

			void persist();
		}, 250);

		return () => window.clearTimeout(timeout);
	}, [hasHydratedPersistedFilters, persistedFilters]);

	useEffect(() => {
		void pageResetSignature;
		// Keep the currently visible page predictable whenever the server-side
		// filter inputs change. The backend still clamps defensively, but the UI
		// should not wait for a round trip to know page 1 is now the right answer.
		setPageState(1);
	}, [pageResetSignature]);

	return {
		filterText,
		setFilterText,
		showFailed,
		setShowFailed,
		showEmptyTranscript,
		setShowEmptyTranscript,
		selectedSttModelKeys,
		setSelectedSttModelKeys,
		selectedLlmModelKeys,
		setSelectedLlmModelKeys,
		filtersOpened,
		setFiltersOpened,
		toggleFiltersOpened,
		filtersExpandedSection,
		setFiltersExpandedSection,
		page,
		setPage,
		query,
		hasActiveFilters,
		isFiltering,
		resetFilters,
		syncServerPage,
	};
}
