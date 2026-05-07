import { Stack } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import {
	filterRequestLogs,
	getLogsPage,
	getLogsPageCount,
	hasActiveLogsFilters,
} from "../lib/logs/readModel";
import {
	useClearRequestLogs,
	useRequestLogs,
	useSettings,
	useUpdateHotkeyDebugEnabled,
} from "../lib/queries";
import { logsAPI, type SystemEvent, tauriAPI } from "../lib/tauri";
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
	const updateHotkeyDebugEnabled = useUpdateHotkeyDebugEnabled();
	const clearLogsMutation = useClearRequestLogs();
	const [exportOpened, exportPopover] = useDisclosure(false);
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
	const hotkeyDebugEnabledRef = useRef(hotkeyDebugEnabled);

	useEffect(() => {
		hotkeyDebugEnabledRef.current = hotkeyDebugEnabled;
	}, [hotkeyDebugEnabled]);

	useEffect(() => {
		return () => {
			if (hotkeyDebugEnabledRef.current) {
				// Hotkey debug is intentionally ephemeral. If the user leaves the page,
				// switch the noisy stream back off without blocking unmount.
				void tauriAPI.updateHotkeyDebugEnabled(false);
			}
		};
	}, []);

	const exportPrivacySafe = async () => {
		try {
			const path = await save({
				defaultPath: "kolboo-request-logs.json",
				filters: [{ name: "JSON", extensions: ["json"] }],
			});
			if (!path) return;

			await logsAPI.exportRequestLogsToFile({
				path,
				stripTextAndPayloads: true,
			});
			exportPopover.close();
			notifications.show({
				title: "Export",
				message: "Exported privacy-safe request logs.",
				color: "teal",
			});
		} catch (error) {
			notifications.show({
				title: "Export failed",
				message: String(error),
				color: "red",
			});
		}
	};

	const exportFull = async () => {
		try {
			const path = await save({
				defaultPath: "kolboo-request-logs-full.json",
				filters: [{ name: "JSON", extensions: ["json"] }],
			});
			if (!path) return;

			await logsAPI.exportRequestLogsToFile({
				path,
				stripTextAndPayloads: false,
			});
			exportPopover.close();
			notifications.show({
				title: "Export",
				message: "Exported full request logs.",
				color: "teal",
			});
		} catch (error) {
			notifications.show({
				title: "Export failed",
				message: String(error),
				color: "red",
			});
		}
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
					exportOpened={exportOpened}
					onExportOpenedChange={(opened) => {
						if (opened) exportPopover.open();
						else exportPopover.close();
					}}
					hasLogs={(logs?.length ?? 0) > 0}
					onExportPrivacySafe={() => {
						void exportPrivacySafe();
					}}
					onExportFull={() => {
						void exportFull();
					}}
					onClearAll={() => clearLogsMutation.mutate()}
					clearAllPending={clearLogsMutation.isPending}
				/>

				<LogsSystemEventsPanel
					systemEvents={systemEvents}
					hotkeyDebugEnabled={hotkeyDebugEnabled}
					hotkeyDebugPending={updateHotkeyDebugEnabled.isPending}
					settingsLoaded={Boolean(settings)}
					onHotkeyDebugChange={(enabled) =>
						updateHotkeyDebugEnabled.mutate(enabled)
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
