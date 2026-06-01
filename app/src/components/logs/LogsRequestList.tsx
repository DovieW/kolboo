import { Accordion, Paper, Stack, Text } from "@mantine/core";
import { useEffect, useMemo } from "react";
import { getLogsEmptyState } from "../../lib/logs/readModel";
import type { RequestLog } from "../../lib/tauri";
import type { RecordingPlayerControls } from "../../lib/useRecordingPlayer";
import { RequestLogItem } from "./RequestLogItem";

export interface LogsRequestListProps {
	logs: RequestLog[];
	totalLogsCount: number;
	openedLogId: string | null;
	onOpenedLogIdChange: (id: string | null) => void;
	player: RecordingPlayerControls;
}

export function LogsRequestList({
	logs,
	totalLogsCount,
	openedLogId,
	onOpenedLogIdChange,
	player,
}: LogsRequestListProps) {
	const emptyState = useMemo(
		() => getLogsEmptyState({ totalLogsCount }),
		[totalLogsCount],
	);

	useEffect(() => {
		if (openedLogId && !logs.some((log) => log.id === openedLogId)) {
			onOpenedLogIdChange(null);
		}
	}, [logs, onOpenedLogIdChange, openedLogId]);

	if (logs.length === 0) {
		return (
			<Paper withBorder p="md">
				<Stack gap={4}>
					<Text fw={600}>{emptyState.title}</Text>
					<Text size="sm" c="dimmed">
						{emptyState.message}
					</Text>
				</Stack>
			</Paper>
		);
	}

	return (
		<Accordion
			variant="separated"
			value={openedLogId}
			onChange={onOpenedLogIdChange}
		>
			{logs.map((log) => (
				<RequestLogItem key={log.id} log={log} player={player} />
			))}
		</Accordion>
	);
}
