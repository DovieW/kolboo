import { describe, expect, it } from "vitest";
import type { RequestLog, SystemEvent } from "../tauri";
import {
	buildLogEntryViewModel,
	buildRequestLogViewModel,
	buildRewriteDiffInfo,
	buildSystemEventViewModel,
	filterRequestLogs,
	formatCallPriceLabel,
	formatDuration,
	formatRequestLogTimestamp,
	formatUsdFromMicros,
	getLogLevelView,
	getLogsEmptyState,
	getLogsPage,
	getLogsPageCount,
	getRequestLogTotalDurationMs,
	getRequestStatusView,
	hasActiveLogsFilters,
	logEntryKey,
} from "./readModel";

function baseLog(overrides: Partial<RequestLog> = {}): RequestLog {
	return {
		id: "req-1",
		started_at: "2026-05-07T12:00:00Z",
		ended_at: "2026-05-07T12:00:01Z",
		stt_provider: "deepgram",
		stt_model: "nova-3",
		llm_provider: "openai",
		llm_model: "gpt-5.4-mini",
		managed_inference: false,
		profile_id: "default",
		profile_name: null,
		preset_id: null,
		preset_name: null,
		raw_transcript: "raw text",
		final_text: "rewritten text",
		rewrite_clipboard_context: null,
		quick_ask_question: null,
		quick_ask_context_text: null,
		quick_ask_clipboard_context: null,
		quick_ask_answer: null,
		quick_ask_provider: null,
		quick_ask_model: null,
		quick_ask_duration_ms: null,
		quick_replace_instructions: null,
		quick_replace_selected_text: null,
		quick_replace_output_text: null,
		quick_replace_clipboard_context: null,
		quick_replace_provider: null,
		quick_replace_model: null,
		quick_replace_duration_ms: null,
		total_duration_ms: 1000,
		stt_duration_ms: 500,
		llm_duration_ms: 250,
		llm_outcome: "succeeded",
		llm_not_attempted_reason: null,
		llm_error_message: null,
		router_duration_ms: null,
		router_strategy: null,
		router_scores: null,
		status: "success",
		error_message: null,
		entries: [
			{
				timestamp: "2026-05-07T12:00:00.123Z",
				level: "info",
				message: "started",
				details: null,
			},
		],
		stt_is_free_tier: false,
		llm_is_free_tier: false,
		stt_estimated_cost_usd_micros: 45_000,
		llm_estimated_cost_usd_micros: 125_000,
		...overrides,
	};
}

describe("Logs View read model", () => {
	it("formats timestamps and durations defensively", () => {
		expect(formatRequestLogTimestamp("2026-05-07T12:00:00Z")).toBe(
			new Date("2026-05-07T12:00:00Z").toLocaleString(undefined, {
				month: "short",
				day: "numeric",
				hour: "2-digit",
				minute: "2-digit",
				second: "2-digit",
			}),
		);
		expect(formatRequestLogTimestamp("not-a-date")).toBe("not-a-date");
		expect(formatDuration(850)).toBe("850ms");
		expect(formatDuration(1250)).toBe("1.25s");
	});

	it("formats request costs for paid and free-tier requests", () => {
		expect(formatUsdFromMicros(125_000)).toBe("$0.125");
		expect(
			formatCallPriceLabel({
				isFreeTier: true,
				estimatedCostUsdMicros: 999_999,
			}),
		).toBe("$0 (free)");
		expect(
			formatCallPriceLabel({
				isFreeTier: false,
				estimatedCostUsdMicros: null,
			}),
		).toBe("—");
	});

	it("returns stable status and log-level metadata", () => {
		expect(getRequestStatusView("success")).toEqual({
			label: "Success",
			color: "green",
			icon: "success",
		});
		expect(getRequestStatusView("in_progress")).toEqual({
			label: "In Progress",
			color: "orange",
			icon: "in_progress",
		});
		expect(getLogLevelView("warn")).toEqual({
			color: "yellow",
			icon: "warn",
		});
		expect(getLogLevelView("debug")).toEqual({
			color: "dimmed",
			icon: "debug",
		});
	});

	it("builds stable entry and system-event view models", () => {
		const logView = buildLogEntryViewModel({
			timestamp: "2026-05-07T12:00:00.123Z",
			level: "error",
			message: "oops",
			details: "details",
		});
		expect(logView).toMatchObject({
			key: logEntryKey({
				timestamp: "2026-05-07T12:00:00.123Z",
				level: "error",
				message: "oops",
				details: "details",
			}),
			message: "oops",
			details: "details",
			levelView: { color: "red", icon: "error" },
		});

		const event: SystemEvent = {
			timestamp: "2026-05-07T12:30:00Z",
			event_type: "shortcut",
			message: "Toggle pressed",
			details: "detail",
		};
		expect(buildSystemEventViewModel(event)).toMatchObject({
			badgeColor: "blue",
			eventType: "shortcut",
			message: "Toggle pressed",
			details: "detail",
		});
	});

	it("derives total duration from timestamps when the backend field is missing", () => {
		expect(
			getRequestLogTotalDurationMs(
				baseLog({
					total_duration_ms: null,
					started_at: "2026-05-07T12:00:00Z",
					ended_at: "2026-05-07T12:00:02Z",
				}),
			),
		).toBe(2000);
		expect(
			getRequestLogTotalDurationMs(
				baseLog({ total_duration_ms: null, ended_at: null }),
			),
		).toBeNull();
		expect(
			getRequestLogTotalDurationMs(
				baseLog({
					total_duration_ms: null,
					started_at: "2026-05-07T12:00:03Z",
					ended_at: "2026-05-07T12:00:02Z",
				}),
			),
		).toBeNull();
	});

	it("filters request logs by text, status, and duration while always keeping in-progress items", () => {
		const logs = [
			baseLog({
				id: "success",
				final_text: "meeting notes",
				total_duration_ms: 2000,
			}),
			baseLog({
				id: "error",
				status: "error",
				error_message: "backend exploded",
				final_text: null,
				raw_transcript: null,
				total_duration_ms: 7000,
			}),
			baseLog({
				id: "cancelled",
				status: "cancelled",
				total_duration_ms: 4000,
			}),
			baseLog({
				id: "progress",
				status: "in_progress",
				ended_at: null,
				total_duration_ms: null,
			}),
		];

		expect(
			filterRequestLogs(logs, {
				filterText: "meeting",
				showSuccess: true,
				showError: true,
				showCancelled: true,
				durationMinSecs: "",
				durationMaxSecs: "",
			}).map((log) => log.id),
		).toEqual(["success", "progress"]);

		expect(
			filterRequestLogs(logs, {
				filterText: "",
				showSuccess: false,
				showError: true,
				showCancelled: false,
				durationMinSecs: 3,
				durationMaxSecs: 5,
			}).map((log) => log.id),
		).toEqual(["progress"]);
	});

	it("tracks active filters, empty states, and pagination deterministically", () => {
		expect(
			hasActiveLogsFilters({
				filterText: "",
				showSuccess: true,
				showError: true,
				showCancelled: true,
				durationMinSecs: "",
				durationMaxSecs: "",
			}),
		).toBe(false);
		expect(
			hasActiveLogsFilters({
				filterText: "",
				showSuccess: false,
				showError: true,
				showCancelled: true,
				durationMinSecs: "",
				durationMaxSecs: "",
			}),
		).toBe(true);

		expect(getLogsPageCount(0)).toBe(1);
		expect(getLogsPageCount(51)).toBe(3);
		expect(getLogsPage([1, 2, 3, 4, 5], 2, 2)).toEqual([3, 4]);

		expect(getLogsEmptyState({ totalLogsCount: 4 })).toEqual({
			title: "No matches",
			message: "Try a different filter.",
		});
		expect(getLogsEmptyState({ totalLogsCount: 0 })).toEqual({
			title: "No request logs yet",
			message: "Start a voice transcription to see logs here.",
		});
	});

	it("builds rich transcription request view models with rewrite summaries and router scores", () => {
		const view = buildRequestLogViewModel(
			baseLog({
				raw_transcript:
					"This transcript has enough shared characters to keep the inline diff visible.",
				final_text:
					"This transcript has enough shared characters to keep the inline diff visible, now rewritten.",
				rewrite_clipboard_context: "clipboard context",
				router_duration_ms: 320,
				router_strategy: "embeddings",
				router_scores: [
					{
						preset_id: "draft",
						preset_name: "Draft",
						score: 0.91234,
						selected: true,
					},
				],
			}),
		);

		expect(view.startedAtLabel).toBe(
			formatRequestLogTimestamp("2026-05-07T12:00:00Z"),
		);
		expect(view.totalDurationLabel).toBe("Total: 1.00s");
		expect(view.sttSummaryLabel).toContain("STT 500ms · deepgram / nova-3 ·");
		expect(view.llmSummaryLabel).toContain(
			"LLM 250ms · openai / gpt-5.4-mini ·",
		);
		expect(view.profileSummaryLabel).toBe("Profile · Default: Default");
		expect(view.showTranscriptPanel).toBe(true);
		expect(view.showRewriteTranscript).toBe(true);
		expect(view.rewriteClipboardContextSection).toEqual({
			label: "Clipboard Context:",
			value: "clipboard context",
		});
		expect(view.rewriteDiffInfo?.changeGroups).toBeGreaterThan(0);
		expect(view.routerSummaryLabel).toBe("Router 320ms · embeddings");
		expect(view.routerScores).toEqual([
			{
				key: "draft",
				presetName: "Draft",
				selected: true,
				scoreLabel: "0.912",
			},
		]);
		expect(view.copyActions.map((action) => action.label)).toEqual([
			"Copy Raw",
			"Copy Rewrite",
		]);
	});

	it("builds quick ask and quick replace view models without leaking empty sections", () => {
		const quickAskView = buildRequestLogViewModel(
			baseLog({
				kind: "quick_ask",
				raw_transcript: null,
				final_text: null,
				quick_ask_context_text: "context",
				quick_ask_question: "question",
				quick_ask_answer: "answer",
				quick_ask_provider: "anthropic",
				quick_ask_model: "claude-sonnet",
				quick_ask_duration_ms: 1800,
			}),
		);
		expect(quickAskView.kindBadge).toEqual({
			label: "Quick Ask",
			color: "orange",
		});
		expect(quickAskView.showQuickAskPanel).toBe(true);
		expect(quickAskView.showTranscriptPanel).toBe(false);
		expect(
			quickAskView.quickAskSections.map((section) => section.label),
		).toEqual(["Context:", "Question:", "Answer:"]);
		expect(quickAskView.quickAskSummaryLabel).toBe(
			"Quick Ask 1.80s · anthropic / claude-sonnet",
		);
		expect(quickAskView.copyActions.map((action) => action.label)).toEqual([
			"Copy Question",
			"Copy Answer",
		]);

		const quickReplaceView = buildRequestLogViewModel(
			baseLog({
				kind: "quick_replace",
				raw_transcript: "transcript fallback",
				final_text: "transcript fallback",
				quick_replace_selected_text: "selected",
				quick_replace_instructions: "rewrite it",
				quick_replace_output_text: "rewritten",
				quick_replace_provider: "openai",
				quick_replace_model: "gpt-5.4-mini",
				quick_replace_duration_ms: 900,
			}),
		);
		expect(quickReplaceView.kindBadge).toEqual({
			label: "Quick Replace",
			color: "cyan",
		});
		expect(quickReplaceView.showQuickReplacePanel).toBe(true);
		expect(quickReplaceView.showTranscriptPanel).toBe(false);
		expect(quickReplaceView.quickReplaceSummaryLabel).toBe(
			"Quick Replace 900ms · openai / gpt-5.4-mini",
		);
		expect(quickReplaceView.copyActions.map((action) => action.label)).toEqual([
			"Copy Selection",
			"Copy Instructions",
			"Copy Output",
		]);
	});

	it("surfaces rewrite skip reasons and drops inline diffs when edits are too large", () => {
		const skipped = buildRequestLogViewModel(
			baseLog({
				llm_duration_ms: null,
				llm_outcome: "not_attempted",
				llm_not_attempted_reason: "quiet_audio_gate",
				llm_error_message: "too quiet",
				final_text: "raw text",
			}),
		);
		expect(skipped.rewriteSkippedSummaryLabel).toBe(
			"Rewrite skipped · quiet audio gate",
		);
		expect(skipped.rewriteSkippedTooltip).toBe("too quiet");

		expect(
			buildRewriteDiffInfo(
				"alpha beta gamma delta epsilon zeta eta theta",
				"completely different words without any overlap at all whatsoever",
			),
		).toBeNull();
	});
});
