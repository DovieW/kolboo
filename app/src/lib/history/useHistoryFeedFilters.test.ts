import { describe, expect, it } from "vitest";
import {
	buildHistoryPageQuery,
	createDefaultHistoryFeedFilters,
	createPersistedHistoryFilterSnapshot,
	getHistoryPageCount,
	getHistoryPageResetKey,
	hasActiveHistoryFilters,
	normalizeHistoryServerPage,
} from "./useHistoryFeedFilters";

describe("History feed filter helpers", () => {
	it("provides the default persisted filter shape", () => {
		expect(createDefaultHistoryFeedFilters()).toEqual({
			filterText: "",
			showFailed: true,
			showEmptyTranscript: false,
			selectedSttModelKeys: [],
			selectedLlmModelKeys: [],
		});
	});

	it("detects whether any non-text filters are active", () => {
		expect(
			hasActiveHistoryFilters({
				showFailed: true,
				showEmptyTranscript: false,
				selectedSttModelKeys: [],
				selectedLlmModelKeys: [],
			}),
		).toBe(false);

		expect(
			hasActiveHistoryFilters({
				showFailed: false,
				showEmptyTranscript: false,
				selectedSttModelKeys: [],
				selectedLlmModelKeys: [],
			}),
		).toBe(true);
	});

	it("normalizes the main history page query", () => {
		expect(
			buildHistoryPageQuery({
				filterText: "meeting",
				showFailed: false,
				showEmptyTranscript: true,
				selectedSttModelKeys: ["stt-a"],
				selectedLlmModelKeys: ["llm-a"],
				page: 0,
				pageSize: 0,
				includeUsageCounts: false,
			}),
		).toEqual({
			filterText: "meeting",
			showFailed: false,
			showEmptyTranscript: true,
			selectedSttModelKeys: ["stt-a"],
			selectedLlmModelKeys: ["llm-a"],
			page: 1,
			pageSize: 1,
			includeUsageCounts: false,
		});
	});

	it("creates copied persistence snapshots for model-key selections", () => {
		const selectedSttModelKeys = ["stt-a"];
		const selectedLlmModelKeys = ["llm-a"];

		const snapshot = createPersistedHistoryFilterSnapshot({
			filterText: "meeting",
			showFailed: false,
			showEmptyTranscript: true,
			selectedSttModelKeys,
			selectedLlmModelKeys,
		});

		expect(snapshot).toEqual({
			filterText: "meeting",
			showFailed: false,
			showEmptyTranscript: true,
			selectedSttModelKeys: ["stt-a"],
			selectedLlmModelKeys: ["llm-a"],
		});
		expect(snapshot.selectedSttModelKeys).not.toBe(selectedSttModelKeys);
		expect(snapshot.selectedLlmModelKeys).not.toBe(selectedLlmModelKeys);
	});

	it("builds a stable page-reset key from persisted filter inputs", () => {
		const filters = {
			filterText: "retro",
			showFailed: true,
			showEmptyTranscript: false,
			selectedSttModelKeys: ["stt-a"],
			selectedLlmModelKeys: ["llm-a"],
		};

		expect(getHistoryPageResetKey(filters)).toBe(
			getHistoryPageResetKey({
				...filters,
				selectedSttModelKeys: [...filters.selectedSttModelKeys],
				selectedLlmModelKeys: [...filters.selectedLlmModelKeys],
			}),
		);
		expect(
			getHistoryPageResetKey({
				...filters,
				showEmptyTranscript: true,
			}),
		).not.toBe(getHistoryPageResetKey(filters));
	});

	it("normalizes synced server pages defensively", () => {
		expect(normalizeHistoryServerPage(undefined)).toBeNull();
		expect(normalizeHistoryServerPage(Number.NaN)).toBeNull();
		expect(normalizeHistoryServerPage(0)).toBe(1);
		expect(normalizeHistoryServerPage(3.9)).toBe(3);
	});

	it("clamps total pages to at least one", () => {
		expect(getHistoryPageCount(0)).toBe(1);
		expect(getHistoryPageCount(26)).toBe(2);
		expect(getHistoryPageCount(Number.NaN)).toBe(1);
	});
});
