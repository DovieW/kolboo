import {
	Accordion,
	Alert,
	Button,
	Group,
	Loader,
	Modal,
	Stack,
	Text,
} from "@mantine/core";
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
	const logsQuery = useRequestLogs(50);
	const logs = logsQuery.data;
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
	const [clearConfirmationOpened, setClearConfirmationOpened] = useState(false);
	const [fullExportConfirmationOpened, setFullExportConfirmationOpened] =
		useState(false);

	const player = useRecordingPlayer({
		onError: (message) => {
			notifications.show({
				title: "Playback",
				message,
				color: "red",
			});
		},
	});
	const stopPlayback = player.stop;

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
	const playbackOwnerId = pageLogs.some((log) => log.id === openedLogId)
		? openedLogId
		: null;
	useEffect(() => {
		// Keep playback ownership here, even when filtering/pagination hides a request
		// or unavailable query data unmounts the list before it can clear its selection.
		void playbackOwnerId;
		stopPlayback();
	}, [playbackOwnerId, stopPlayback]);
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
				setFullExportConfirmationOpened(false);
			},
			onError: (error) => {
				notifications.show(getLogsExportFailureNotification(error));
			},
		});
	};

	return (
		<div className="main-content logs-page">
			<header className="tv-page-header logs-page-header">
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
						setFilterText("");
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
					onExportFull={() => {
						logsOrchestration.setExportOpened(false);
						setFullExportConfirmationOpened(true);
					}}
					onClearAll={() => setClearConfirmationOpened(true)}
					clearAllPending={logsOrchestration.clearLogs.isPending}
				/>
			</header>
			<Stack gap="lg" className="main-content-inner">
				{logsQuery.isError ? (
					<Alert color="red" title="Couldn't load request logs">
						<Group justify="space-between" gap="sm">
							<Text size="sm">Your saved logs have not been changed.</Text>
							<Button
								variant="light"
								color="red"
								size="xs"
								onClick={() => void logsQuery.refetch()}
								loading={logsQuery.isFetching}
							>
								Retry
							</Button>
						</Group>
					</Alert>
				) : null}
				{logsQuery.isLoading ? (
					<Group
						justify="center"
						p="xl"
						role="status"
						aria-label="Loading request logs"
					>
						<Loader size="sm" />
						<Text size="sm" c="dimmed">
							Loading logs…
						</Text>
					</Group>
				) : logs !== undefined ? (
					<LogsRequestList
						logs={pageLogs}
						totalLogsCount={logs?.length ?? 0}
						openedLogId={openedLogId}
						onOpenedLogIdChange={setOpenedLogId}
						player={player}
					/>
				) : null}
				<Accordion variant="separated" className="logs-diagnostics">
					<Accordion.Item value="system-events">
						<Accordion.Control>
							<Group gap="xs">
								<Text size="sm" fw={500}>
									System events
								</Text>
								<Text size="xs" c="dimmed">
									{systemEvents.length} this session
									{hotkeyDebugEnabled ? " · Hotkey debug on" : ""}
								</Text>
							</Group>
						</Accordion.Control>
						<Accordion.Panel>
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
						</Accordion.Panel>
					</Accordion.Item>
				</Accordion>
			</Stack>
			<Modal
				opened={clearConfirmationOpened}
				onClose={() =>
					!logsOrchestration.clearLogs.isPending &&
					setClearConfirmationOpened(false)
				}
				title="Clear request logs?"
				centered
				closeOnEscape={!logsOrchestration.clearLogs.isPending}
				closeOnClickOutside={!logsOrchestration.clearLogs.isPending}
				withCloseButton={!logsOrchestration.clearLogs.isPending}
			>
				<Stack gap="md">
					<Text size="sm">
						This deletes all saved request logs, not just the filtered results.
						Your History and recordings are kept. This cannot be undone.
					</Text>
					<Group justify="flex-end">
						<Button
							variant="default"
							disabled={logsOrchestration.clearLogs.isPending}
							onClick={() => setClearConfirmationOpened(false)}
						>
							Cancel
						</Button>
						<Button
							color="red"
							loading={logsOrchestration.clearLogs.isPending}
							onClick={() =>
								logsOrchestration.clearLogs.mutate(undefined, {
									onSuccess: () => {
										setClearConfirmationOpened(false);
										setOpenedLogId(null);
									},
									onError: () =>
										notifications.show({
											title: "Couldn't clear logs",
											message:
												"Try again. Your History and recordings are unaffected.",
											color: "red",
										}),
								})
							}
						>
							Clear all request logs
						</Button>
					</Group>
				</Stack>
			</Modal>
			<Modal
				opened={fullExportConfirmationOpened}
				onClose={() =>
					!logsOrchestration.exportLogs.isPending &&
					setFullExportConfirmationOpened(false)
				}
				title="Export full debug logs?"
				centered
				closeOnEscape={!logsOrchestration.exportLogs.isPending}
				closeOnClickOutside={!logsOrchestration.exportLogs.isPending}
				withCloseButton={!logsOrchestration.exportLogs.isPending}
			>
				<Stack gap="md">
					<Text size="sm">
						Full logs may contain transcripts, prompts and provider responses.
						Only share them with someone you trust.
					</Text>
					<Group justify="flex-end">
						<Button
							variant="default"
							disabled={logsOrchestration.exportLogs.isPending}
							onClick={() => setFullExportConfirmationOpened(false)}
						>
							Cancel
						</Button>
						<Button
							variant="light"
							disabled={logsOrchestration.exportLogs.isPending}
							onClick={() => handleExport("privacySafe")}
						>
							Use privacy-safe export
						</Button>
						<Button
							loading={logsOrchestration.exportLogs.isPending}
							onClick={() => handleExport("full")}
						>
							Export full logs
						</Button>
					</Group>
				</Stack>
			</Modal>
		</div>
	);
}
