import type {
	LogEntry,
	LogLevel,
	RequestKind,
	RequestLog,
	RequestStatus,
	SystemEvent,
} from "../tauri";
import { diffTextInline, type TextDiffChunk } from "../textDiff";

export const LOGS_PAGE_SIZE = 25;

export type LogsDurationInput = number | string;

export interface LogsFilters {
	filterText: string;
	showSuccess: boolean;
	showError: boolean;
	showCancelled: boolean;
	durationMinSecs: LogsDurationInput;
	durationMaxSecs: LogsDurationInput;
}

export interface LogsEmptyState {
	title: string;
	message: string;
}

export type RequestStatusIconName =
	| "success"
	| "error"
	| "cancelled"
	| "in_progress"
	| "unknown";

export interface RequestStatusViewModel {
	label: string;
	color: string;
	icon: RequestStatusIconName;
}

export type LogLevelIconName = "debug" | "info" | "warn" | "error" | "none";

export interface LogLevelViewModel {
	color: string;
	icon: LogLevelIconName;
}

export interface LogsTextSection {
	label: string;
	value: string;
}

export interface LogsCopyAction {
	label: string;
	value: string;
}

export interface RewriteDiffInfo {
	chunks: TextDiffChunk[];
	changeGroups: number;
}

export interface RouterScoreViewModel {
	key: string;
	presetName: string;
	selected: boolean;
	scoreLabel: string;
}

export interface LogEntryViewModel {
	key: string;
	timeLabel: string;
	levelView: LogLevelViewModel;
	message: string;
	details: string | null;
}

export interface SystemEventViewModel {
	key: string;
	timeLabel: string;
	badgeColor: string;
	eventType: string;
	message: string;
	details: string | null;
}

export interface RequestLogViewModel {
	id: string;
	startedAtLabel: string;
	kind: RequestKind | "transcription";
	kindBadge: { label: string; color: string } | null;
	isManagedRequest: boolean;
	statusView: RequestStatusViewModel;
	totalDurationMs: number | null;
	totalDurationLabel: string | null;
	sttSummaryLabel: string;
	quickAskSummaryLabel: string | null;
	quickReplaceSummaryLabel: string | null;
	llmSummaryLabel: string | null;
	rewriteSkippedSummaryLabel: string | null;
	rewriteSkippedTooltip: string | null;
	routerSummaryLabel: string | null;
	profileSummaryLabel: string;
	quickAskSections: LogsTextSection[];
	quickReplaceSections: LogsTextSection[];
	showQuickAskPanel: boolean;
	showQuickReplacePanel: boolean;
	showTranscriptPanel: boolean;
	showRewriteTranscript: boolean;
	rawTranscriptSection: LogsTextSection | null;
	rewriteOutputSection: LogsTextSection | null;
	rewriteOutputUnchanged: boolean;
	rewriteClipboardContextSection: LogsTextSection | null;
	singleTranscriptSection: LogsTextSection | null;
	rewriteDiffInfo: RewriteDiffInfo | null;
	errorMessage: string | null;
	routerScores: RouterScoreViewModel[];
	playDisabled: boolean;
	logEntries: LogEntryViewModel[];
	copyActions: LogsCopyAction[];
}

function trimToNull(value: string | null | undefined): string | null {
	const trimmed = (value ?? "").trim();
	return trimmed.length > 0 ? trimmed : null;
}

function addTextSection(
	sections: LogsTextSection[],
	label: string,
	value: string | null | undefined,
) {
	const trimmed = trimToNull(value);
	if (!trimmed) return;
	sections.push({ label, value: trimmed });
}

function addCopyAction(
	actions: LogsCopyAction[],
	label: string,
	value: string | null | undefined,
) {
	const trimmed = trimToNull(value);
	if (!trimmed) return;
	actions.push({ label, value: trimmed });
}

function formatDateWithOptions(
	value: string,
	options: Intl.DateTimeFormatOptions,
): string {
	const date = new Date(value);
	if (!Number.isFinite(date.getTime())) return value;
	return date.toLocaleString(undefined, options);
}

export function formatRequestLogTimestamp(timestamp: string): string {
	return formatDateWithOptions(timestamp, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
	});
}

export function formatLogEntryTime(timestamp: string): string {
	const date = new Date(timestamp);
	if (!Number.isFinite(date.getTime())) return timestamp;
	return date.toLocaleTimeString(undefined, {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		fractionalSecondDigits: 3,
	});
}

export function formatSystemEventTime(timestamp: string): string {
	const date = new Date(timestamp);
	if (!Number.isFinite(date.getTime())) return timestamp;
	return date.toLocaleTimeString(undefined, {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
	});
}

export function formatDuration(ms: number): string {
	if (ms < 1000) return `${ms}ms`;
	return `${(ms / 1000).toFixed(2)}s`;
}

export function formatUsdFromMicros(micros: number): string {
	const dollars = micros / 1_000_000;

	let fixed: string;
	if (dollars >= 10) fixed = dollars.toFixed(2);
	else if (dollars >= 1) fixed = dollars.toFixed(3);
	else if (dollars >= 0.1) fixed = dollars.toFixed(4);
	else fixed = dollars.toFixed(6);

	// Keep cost chips compact without hiding meaningful precision on small calls.
	fixed = fixed.replace(/\.0+$/, "").replace(/(\.\d*[1-9])0+$/, "$1");
	return `$${fixed}`;
}

export function formatCallPriceLabel(params: {
	isFreeTier: boolean;
	estimatedCostUsdMicros: number | null | undefined;
}): string {
	if (params.isFreeTier) return "$0 (free)";
	if (typeof params.estimatedCostUsdMicros === "number") {
		return formatUsdFromMicros(params.estimatedCostUsdMicros);
	}
	return "—";
}

export function getRequestStatusView(
	status: RequestStatus,
): RequestStatusViewModel {
	switch (status) {
		case "success":
			return { label: "Success", color: "green", icon: "success" };
		case "error":
			return { label: "Error", color: "red", icon: "error" };
		case "cancelled":
			return { label: "Cancelled", color: "yellow", icon: "cancelled" };
		case "in_progress":
			return { label: "In Progress", color: "orange", icon: "in_progress" };
		default:
			return { label: status, color: "gray", icon: "unknown" };
	}
}

export function getLogLevelView(level: LogLevel): LogLevelViewModel {
	switch (level) {
		case "debug":
			return { color: "dimmed", icon: "debug" };
		case "info":
			return { color: "blue", icon: "info" };
		case "warn":
			return { color: "yellow", icon: "warn" };
		case "error":
			return { color: "red", icon: "error" };
		default:
			return { color: "gray", icon: "none" };
	}
}

export function getSystemEventBadgeColor(eventType: string): string {
	switch (eventType) {
		case "error":
			return "red";
		case "shortcut":
			return "blue";
		default:
			return "gray";
	}
}

export function logEntryKey(entry: LogEntry): string {
	// The backend doesn't provide a stable id per log entry, so keep the frontend key derived from
	// content exactly like the previous inline implementation.
	return `${entry.timestamp}-${entry.level}-${entry.message}-${entry.details ?? ""}`;
}

export function buildLogEntryViewModel(entry: LogEntry): LogEntryViewModel {
	return {
		key: logEntryKey(entry),
		timeLabel: formatLogEntryTime(entry.timestamp),
		levelView: getLogLevelView(entry.level),
		message: entry.message,
		details: entry.details,
	};
}

export function systemEventKey(event: SystemEvent): string {
	return `${event.timestamp}-${event.event_type}-${event.message}-${event.details ?? ""}`;
}

export function buildSystemEventViewModel(
	event: SystemEvent,
): SystemEventViewModel {
	return {
		key: systemEventKey(event),
		timeLabel: formatSystemEventTime(event.timestamp),
		badgeColor: getSystemEventBadgeColor(event.event_type),
		eventType: event.event_type,
		message: event.message,
		details: event.details,
	};
}

export function getRequestLogTotalDurationMs(log: RequestLog): number | null {
	if (typeof log.total_duration_ms === "number") return log.total_duration_ms;
	if (!log.ended_at) return null;

	const start = Date.parse(log.started_at);
	const end = Date.parse(log.ended_at);
	if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
	if (end < start) return null;
	return end - start;
}

function buildProviderMetaLabel(
	provider: string | null | undefined,
	model: string | null | undefined,
): string | null {
	const providerTrimmed = trimToNull(provider);
	const modelTrimmed = trimToNull(model);
	if (!providerTrimmed && !modelTrimmed) return null;
	if (providerTrimmed && modelTrimmed) {
		return `${providerTrimmed} / ${modelTrimmed}`;
	}
	return providerTrimmed ?? modelTrimmed;
}

function getRequestLogProfileLabel(log: RequestLog): string {
	const profileName = trimToNull(log.profile_name);
	const profileId = trimToNull(log.profile_id);
	if (profileName) return profileName;
	if (!profileId || profileId === "default") return "Default";
	return profileId;
}

function getRequestLogPresetLabel(log: RequestLog): string {
	return trimToNull(log.preset_name) ?? trimToNull(log.preset_id) ?? "Default";
}

export function getRewriteSkippedReasonLabel(
	reason: RequestLog["llm_not_attempted_reason"],
): string {
	switch (reason) {
		case "quiet_audio_gate":
			return "quiet audio gate";
		case "no_speech_detected_by_vad":
			return "no speech detected";
		case "disabled_default_profile":
			return "disabled (default profile)";
		case "disabled_profile":
			return "disabled (profile)";
		case "disabled_preset":
			return "disabled (preset)";
		case "provider_unavailable":
			return "provider unavailable";
		case "unknown":
			return "unknown";
		default:
			return "unknown";
	}
}

export function buildRewriteDiffInfo(
	before: string,
	after: string,
): RewriteDiffInfo | null {
	const chunks = diffTextInline(before, after);

	let equalChars = 0;
	let changedChars = 0;
	for (const chunk of chunks) {
		if (chunk.added || chunk.removed) changedChars += chunk.value.length;
		else equalChars += chunk.value.length;
	}

	const total = equalChars + changedChars;
	const changeRatio = total > 0 ? changedChars / total : 0;
	if (changeRatio >= 0.62 || equalChars < 24) {
		return null;
	}

	let changeGroups = 0;
	let inGroup = false;
	for (const chunk of chunks) {
		const changed = Boolean(chunk.added) || Boolean(chunk.removed);
		if (!changed) {
			inGroup = false;
			continue;
		}
		if (!inGroup) {
			changeGroups += 1;
			inGroup = true;
		}
	}

	return { chunks, changeGroups };
}

export function buildRequestLogViewModel(log: RequestLog): RequestLogViewModel {
	const kind = log.kind ?? "transcription";
	const isQuickAsk = kind === "quick_ask";
	const isQuickReplace = kind === "quick_replace";
	const isTranscription = kind === "transcription";
	const llmAttempted = typeof log.llm_duration_ms === "number";
	const rewriteSkipped = log.llm_outcome === "not_attempted";
	const rewriteSkippedReasonLabel = getRewriteSkippedReasonLabel(
		log.llm_not_attempted_reason ?? null,
	);
	const totalDurationMs = getRequestLogTotalDurationMs(log);
	const totalDurationLabel =
		totalDurationMs !== null
			? `Total: ${formatDuration(totalDurationMs)}`
			: null;
	const sttMetaLabel =
		buildProviderMetaLabel(log.stt_provider, log.stt_model) ?? "unknown";
	const llmMetaLabel =
		buildProviderMetaLabel(log.llm_provider, log.llm_model) ?? "unknown";
	const quickAskMetaLabel = buildProviderMetaLabel(
		log.quick_ask_provider,
		log.quick_ask_model,
	);
	const quickReplaceMetaLabel = buildProviderMetaLabel(
		log.quick_replace_provider,
		log.quick_replace_model,
	);
	const sttPriceLabel = formatCallPriceLabel({
		isFreeTier: log.stt_is_free_tier,
		estimatedCostUsdMicros: log.stt_estimated_cost_usd_micros,
	});
	const llmPriceLabel = formatCallPriceLabel({
		isFreeTier: log.llm_is_free_tier,
		estimatedCostUsdMicros: log.llm_estimated_cost_usd_micros,
	});
	const routerAttempted =
		typeof log.router_duration_ms === "number" ||
		Boolean(trimToNull(log.router_strategy)) ||
		(Array.isArray(log.router_scores) && log.router_scores.length > 0);
	const profileLabel = getRequestLogProfileLabel(log);
	const presetLabel = getRequestLogPresetLabel(log);
	const rawTranscriptTrimmed = trimToNull(log.raw_transcript);
	const finalOutputTrimmed = trimToNull(log.final_text);
	const quickAskSections: LogsTextSection[] = [];
	const quickReplaceSections: LogsTextSection[] = [];
	const copyActions: LogsCopyAction[] = [];

	addTextSection(quickAskSections, "Context:", log.quick_ask_context_text);
	addTextSection(
		quickAskSections,
		"Clipboard Context:",
		log.quick_ask_clipboard_context,
	);
	addTextSection(quickAskSections, "OCR Context:", log.ocr_context_text);
	addTextSection(quickAskSections, "Question:", log.quick_ask_question);
	addTextSection(quickAskSections, "Answer:", log.quick_ask_answer);

	addTextSection(
		quickReplaceSections,
		"Selected Text:",
		log.quick_replace_selected_text,
	);
	addTextSection(
		quickReplaceSections,
		"Clipboard Context:",
		log.quick_replace_clipboard_context,
	);
	addTextSection(quickReplaceSections, "OCR Context:", log.ocr_context_text);
	addTextSection(
		quickReplaceSections,
		"Instructions:",
		log.quick_replace_instructions,
	);
	addTextSection(
		quickReplaceSections,
		"Output:",
		log.quick_replace_output_text,
	);

	addCopyAction(copyActions, "Copy Question", log.quick_ask_question);
	addCopyAction(copyActions, "Copy Answer", log.quick_ask_answer);
	addCopyAction(copyActions, "Copy Selection", log.quick_replace_selected_text);
	addCopyAction(
		copyActions,
		"Copy Instructions",
		log.quick_replace_instructions,
	);
	addCopyAction(copyActions, "Copy Output", log.quick_replace_output_text);
	if (!isQuickReplace && !isQuickAsk) {
		addCopyAction(copyActions, "Copy Raw", rawTranscriptTrimmed);
		addCopyAction(copyActions, "Copy Rewrite", finalOutputTrimmed);
	}

	const showQuickAskPanel = isQuickAsk && quickAskSections.length > 0;
	const showQuickReplacePanel =
		isQuickReplace && quickReplaceSections.length > 0;
	const showTranscriptPanel =
		Boolean(log.raw_transcript || log.final_text) &&
		!isQuickAsk &&
		(!isQuickReplace || !showQuickReplacePanel);
	const showRewriteTranscript = llmAttempted && showTranscriptPanel;
	const rewriteOutputUnchanged =
		typeof log.raw_transcript === "string" &&
		typeof log.final_text === "string" &&
		log.raw_transcript === log.final_text;
	const rewriteDiffInfo =
		showRewriteTranscript &&
		typeof log.raw_transcript === "string" &&
		typeof log.final_text === "string" &&
		log.raw_transcript !== log.final_text
			? buildRewriteDiffInfo(log.raw_transcript, log.final_text)
			: null;

	return {
		id: log.id,
		startedAtLabel: formatRequestLogTimestamp(log.started_at),
		kind,
		kindBadge: isQuickAsk
			? { label: "Quick Ask", color: "orange" }
			: isQuickReplace
				? { label: "Quick Replace", color: "cyan" }
				: null,
		isManagedRequest: log.managed_inference ?? false,
		statusView: getRequestStatusView(log.status),
		totalDurationMs,
		totalDurationLabel,
		sttSummaryLabel:
			typeof log.stt_duration_ms === "number"
				? `STT ${formatDuration(log.stt_duration_ms)} · ${sttMetaLabel} · ${sttPriceLabel}`
				: `STT · ${sttMetaLabel} · ${sttPriceLabel}`,
		quickAskSummaryLabel:
			isQuickAsk &&
			(quickAskMetaLabel || typeof log.quick_ask_duration_ms === "number")
				? `Quick Ask${
						typeof log.quick_ask_duration_ms === "number"
							? ` ${formatDuration(log.quick_ask_duration_ms)}`
							: ""
					}${quickAskMetaLabel ? ` · ${quickAskMetaLabel}` : ""}`
				: null,
		quickReplaceSummaryLabel:
			isQuickReplace &&
			(quickReplaceMetaLabel ||
				typeof log.quick_replace_duration_ms === "number")
				? `Quick Replace${
						typeof log.quick_replace_duration_ms === "number"
							? ` ${formatDuration(log.quick_replace_duration_ms)}`
							: ""
					}${quickReplaceMetaLabel ? ` · ${quickReplaceMetaLabel}` : ""}`
				: null,
		llmSummaryLabel:
			typeof log.llm_duration_ms === "number"
				? `LLM ${formatDuration(log.llm_duration_ms)} · ${llmMetaLabel} · ${llmPriceLabel}`
				: isTranscription && (llmAttempted || Boolean(log.llm_provider))
					? `LLM · ${llmMetaLabel} · ${llmPriceLabel}`
					: null,
		rewriteSkippedSummaryLabel:
			isTranscription && rewriteSkipped
				? `Rewrite skipped · ${rewriteSkippedReasonLabel}`
				: null,
		rewriteSkippedTooltip:
			isTranscription && rewriteSkipped
				? log.llm_error_message?.trim() ||
					`Rewrite skipped (${rewriteSkippedReasonLabel})`
				: null,
		routerSummaryLabel: routerAttempted
			? `Router${
					typeof log.router_duration_ms === "number"
						? ` ${formatDuration(log.router_duration_ms)}`
						: ""
				}${trimToNull(log.router_strategy) ? ` · ${trimToNull(log.router_strategy)}` : ""}`
			: null,
		profileSummaryLabel: `Profile · ${profileLabel}${
			isTranscription ? `: ${presetLabel}` : ""
		}`,
		quickAskSections,
		quickReplaceSections,
		showQuickAskPanel,
		showQuickReplacePanel,
		showTranscriptPanel,
		showRewriteTranscript,
		rawTranscriptSection: rawTranscriptTrimmed
			? { label: "Raw Transcript:", value: rawTranscriptTrimmed }
			: null,
		rewriteOutputSection: finalOutputTrimmed
			? { label: "Rewrite Output:", value: finalOutputTrimmed }
			: null,
		rewriteOutputUnchanged,
		rewriteClipboardContextSection: trimToNull(log.rewrite_clipboard_context)
			? {
					label: "Clipboard Context:",
					value: trimToNull(log.rewrite_clipboard_context) ?? "",
				}
			: null,
		singleTranscriptSection:
			showTranscriptPanel && !showRewriteTranscript
				? {
						label: isQuickReplace
							? "Instructions (transcript):"
							: "Transcript:",
						value: log.final_text ?? log.raw_transcript ?? "(empty)",
					}
				: null,
		rewriteDiffInfo,
		errorMessage: trimToNull(log.error_message),
		routerScores: Array.isArray(log.router_scores)
			? log.router_scores.map((score) => ({
					key: score.preset_id,
					presetName: score.preset_name,
					selected: score.selected,
					scoreLabel:
						typeof score.score === "number" && Number.isFinite(score.score)
							? score.score.toFixed(3)
							: "—",
				}))
			: [],
		playDisabled: log.status === "in_progress",
		logEntries: log.entries.map((entry) => buildLogEntryViewModel(entry)),
		copyActions,
	};
}

function buildRequestLogSearchHaystack(log: RequestLog): string {
	return [
		log.id,
		log.kind ?? "",
		log.error_message ?? "",
		log.raw_transcript ?? "",
		log.final_text ?? "",
		log.quick_ask_question ?? "",
		log.quick_ask_context_text ?? "",
		log.quick_ask_answer ?? "",
		log.quick_replace_instructions ?? "",
		log.quick_replace_selected_text ?? "",
		log.quick_replace_output_text ?? "",
	]
		.join("\n")
		.toLowerCase();
}

function parseDurationFilterMs(value: LogsDurationInput): number | null {
	const parsed =
		typeof value === "number" ? value : Number.parseFloat(String(value));
	if (!Number.isFinite(parsed) || parsed < 0) return null;
	return parsed * 1000;
}

export function hasActiveLogsFilters(filters: LogsFilters): boolean {
	return (
		!filters.showSuccess ||
		!filters.showError ||
		!filters.showCancelled ||
		String(filters.durationMinSecs).trim().length > 0 ||
		String(filters.durationMaxSecs).trim().length > 0
	);
}

export function filterRequestLogs(
	logs: RequestLog[] | null | undefined,
	filters: LogsFilters,
): RequestLog[] {
	if (!logs) return [];

	const query = filters.filterText.trim().toLowerCase();
	const minMs = parseDurationFilterMs(filters.durationMinSecs);
	const maxMs = parseDurationFilterMs(filters.durationMaxSecs);

	return logs.filter((log) => {
		// Preserve the old UX rule: live in-progress requests always stay visible.
		if (log.status === "in_progress") return true;

		if (query && !buildRequestLogSearchHaystack(log).includes(query)) {
			return false;
		}

		if (!filters.showSuccess && log.status === "success") return false;
		if (!filters.showError && log.status === "error") return false;
		if (!filters.showCancelled && log.status === "cancelled") return false;

		if (minMs !== null || maxMs !== null) {
			const totalMs = getRequestLogTotalDurationMs(log);
			if (typeof totalMs === "number") {
				if (minMs !== null && totalMs < minMs) return false;
				if (maxMs !== null && totalMs > maxMs) return false;
			}
		}

		return true;
	});
}

export function getLogsPageCount(
	totalLogs: number,
	pageSize = LOGS_PAGE_SIZE,
): number {
	return Math.max(1, Math.ceil(totalLogs / pageSize));
}

export function getLogsPage<T>(
	items: T[],
	page: number,
	pageSize = LOGS_PAGE_SIZE,
): T[] {
	const start = (page - 1) * pageSize;
	return items.slice(start, start + pageSize);
}

export function getLogsEmptyState(params: {
	totalLogsCount: number;
}): LogsEmptyState {
	if (params.totalLogsCount > 0) {
		return {
			title: "No matches",
			message: "Try a different filter.",
		};
	}

	return {
		title: "No request logs yet",
		message: "Start a voice transcription to see logs here.",
	};
}
