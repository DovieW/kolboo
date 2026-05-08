import { Stack } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import {
	getLogsExportFailureNotification,
	getLogsExportSuccessNotification,
	type LogsExportKind,
	useLogsViewOrchestration,
} from "../lib/logs/orchestration";
import {
	filterRequestLogs,
	getLogsPage,
	getLogsPageCount,
	hasActiveLogsFilters,
} from "../lib/logs/readModel";
import { useRequestLogs, useSettings } from "../lib/queries";
import type { SystemEvent } from "../lib/tauri";
import { useRecordingPlayer } from "../lib/useRecordingPlayer";
import { LogsRequestList } from "./logs/LogsRequestList";
import { LogsSystemEventsPanel } from "./logs/LogsSystemEventsPanel";
import { LogsToolbar } from "./logs/LogsToolbar";

export function LogsView(
	props: { jumpToLogId?: string | null; onJumpHandled?: () => void } = {},
) {
	const { jumpToLogId = null, onJumpHandled } = props;
	const { data: logs } = useRequestLogs(50);
	const { data: settings } = useSettings();
	const [systemEvents, setSystemEvents] = useState<SystemEvent[]>([]);
	const [filterText, setFilterText] = useState("");
	const [filtersOpened, setFiltersOpened] = useState(false);
	const [openedLogId, setOpenedLogId] = useState<string | null>(null);
	const [showSuccess, setShowSuccess] = useState(true);
	const [showError, setShowError] = useState(true);
	const [showCancelled, setShowCancelled] = useState(true);
	const [durationMinSecs, setDurationMinSecs] = useState<string | number>("");
	const [durationMaxSecs, setDurationMaxSecs] = useState<string | number>("");
	const [page, setPage] = useState(1);

	const player = useRecordingPlayer({
		onError: (message) => {
			notifications.show({
				title: "Playback",
				message,
				color: "red",
			});
		},
	});

	useEffect(() => {
		const unlistenPromise = listen<SystemEvent>("system-event", (event) => {
			// System events are intentionally frontend-local and capped; request logs remain
			// backend-owned and sanitized before they ever reach this view.
			setSystemEvents((previous) => [event.payload, ...previous].slice(0, 50));
		});

		return () => {
			void unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
		};
	}, []);

	useEffect(() => {
		if (!jumpToLogId) return;

		setFilterText(jumpToLogId);
		setShowSuccess(true);
		setShowError(true);
		setShowCancelled(true);
		setDurationMinSecs("");
		setDurationMaxSecs("");
		setFiltersOpened(false);
		setPage(1);
		setOpenedLogId(jumpToLogId);

		onJumpHandled?.();
	}, [jumpToLogId, onJumpHandled]);

	const filters = useMemo(
		() => ({
			filterText,
			showSuccess,
			showError,
			showCancelled,
			durationMinSecs,
			durationMaxSecs,
		}),
		[
			filterText,
			showSuccess,
			showError,
			showCancelled,
			durationMinSecs,
			durationMaxSecs,
		],
	);

	const filteredLogs = useMemo(
		() => filterRequestLogs(logs, filters),
		[filters, logs],
	);
	const totalPages = useMemo(
		() => getLogsPageCount(filteredLogs.length),
		[filteredLogs.length],
	);
	const pageLogs = useMemo(
		() => getLogsPage(filteredLogs, page),
		[filteredLogs, page],
	);
	const hasActiveFilters = useMemo(
		() => hasActiveLogsFilters(filters),
		[filters],
	);
	const filterResetKey = useMemo(
		() =>
			JSON.stringify([
				filterText,
				showSuccess,
				showError,
				showCancelled,
				durationMinSecs,
				durationMaxSecs,
			]),
		[
			filterText,
			showSuccess,
			showError,
			showCancelled,
			durationMinSecs,
			durationMaxSecs,
		],
	);

	useEffect(() => {
		void filterResetKey;
		setPage(1);
	}, [filterResetKey]);

	useEffect(() => {
		setPage((current) => Math.min(Math.max(1, current), totalPages));
	}, [totalPages]);

	const hotkeyDebugEnabled = settings?.hotkey_debug_enabled ?? false;
	const logsOrchestration = useLogsViewOrchestration({ hotkeyDebugEnabled });

	const handleExport = (kind: LogsExportKind) => {
		logsOrchestration.exportLogs.mutate(kind, {
			onSuccess: (result) => {
				if (result.kind !== "exported") {
					return;
				}

				notifications.show(getLogsExportSuccessNotification(result.exportKind));
			},
			onError: (error) => {
				notifications.show(getLogsExportFailureNotification(error));
			},
		});
	};

	return (
		<div style={{ width: "100%" }}>
			<Stack gap="md" className="tv-page-header">
				<LogsToolbar
					totalLogsCount={logs?.length ?? 0}
					filteredLogsCount={filteredLogs.length}
					filterText={filterText}
					onFilterTextChange={setFilterText}
					hasActiveFilters={hasActiveFilters}
					filtersOpened={filtersOpened}
					onFiltersOpenedChange={setFiltersOpened}
					showSuccess={showSuccess}
					onShowSuccessChange={setShowSuccess}
					showError={showError}
					onShowErrorChange={setShowError}
					showCancelled={showCancelled}
					onShowCancelledChange={setShowCancelled}
					durationMinSecs={durationMinSecs}
					onDurationMinSecsChange={setDurationMinSecs}
					durationMaxSecs={durationMaxSecs}
					onDurationMaxSecsChange={setDurationMaxSecs}
					onResetFilters={() => {
						setShowSuccess(true);
						setShowError(true);
						setShowCancelled(true);
						setDurationMinSecs("");
						setDurationMaxSecs("");
					}}
					page={page}
					totalPages={totalPages}
					onFirstPage={() => setPage(1)}
					onPreviousPage={() => setPage((current) => Math.max(1, current - 1))}
					onNextPage={() =>
						setPage((current) => Math.min(totalPages, current + 1))
					}
					onLastPage={() => setPage(totalPages)}
					exportOpened={logsOrchestration.exportOpened}
					onExportOpenedChange={logsOrchestration.setExportOpened}
					hasLogs={(logs?.length ?? 0) > 0}
					onExportPrivacySafe={() => handleExport("privacySafe")}
					onExportFull={() => handleExport("full")}
					onClearAll={() => logsOrchestration.clearLogs.mutate()}
					clearAllPending={logsOrchestration.clearLogs.isPending}
				/>

				<LogsSystemEventsPanel
					systemEvents={systemEvents}
					hotkeyDebugEnabled={hotkeyDebugEnabled}
					hotkeyDebugPending={
						logsOrchestration.updateHotkeyDebugEnabled.isPending
					}
					settingsLoaded={Boolean(settings)}
					onHotkeyDebugChange={(enabled) =>
						logsOrchestration.updateHotkeyDebugEnabled.mutate(enabled)
					}
					onClear={() => setSystemEvents([])}
				/>

				<LogsRequestList
					logs={pageLogs}
					totalLogsCount={logs?.length ?? 0}
					openedLogId={openedLogId}
					onOpenedLogIdChange={setOpenedLogId}
					player={player}
				/>
			</Stack>
		</div>
	);
}
