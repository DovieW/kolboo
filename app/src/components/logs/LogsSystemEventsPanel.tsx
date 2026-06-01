import {
	Badge,
	Button,
	Code,
	CopyButton,
	Group,
	Paper,
	ScrollArea,
	Stack,
	Switch,
	Text,
} from "@mantine/core";
import { Check, Copy } from "lucide-react";
import { buildSystemEventViewModel } from "../../lib/logs/readModel";
import type { SystemEvent } from "../../lib/tauri";

export interface LogsSystemEventsPanelProps {
	systemEvents: SystemEvent[];
	hotkeyDebugEnabled: boolean;
	hotkeyDebugPending: boolean;
	settingsLoaded: boolean;
	onHotkeyDebugChange: (enabled: boolean) => void;
	onClear: () => void;
}

export function LogsSystemEventsPanel({
	systemEvents,
	hotkeyDebugEnabled,
	hotkeyDebugPending,
	settingsLoaded,
	onHotkeyDebugChange,
	onClear,
}: LogsSystemEventsPanelProps) {
	return (
		<Paper withBorder p="md" radius="md">
			<Stack gap="sm">
				<Group justify="space-between" align="flex-start" gap="sm" wrap="wrap">
					<Stack gap={2}>
						<Text fw={600}>System Events</Text>
						<Text size="sm" c="dimmed">
							Live frontend events stay local to this view for debugging.
							Request logs below remain backend-owned and already sanitized.
						</Text>
					</Stack>
					<Group gap="xs" wrap="wrap">
						<Switch
							label="Hotkey debug"
							checked={hotkeyDebugEnabled}
							disabled={!settingsLoaded || hotkeyDebugPending}
							onChange={(event) => {
								onHotkeyDebugChange(event.currentTarget.checked);
							}}
						/>
						<CopyButton
							value={JSON.stringify(systemEvents, null, 2)}
							timeout={1200}
						>
							{({ copied, copy }) => (
								<Button
									size="xs"
									variant="light"
									leftSection={
										copied ? <Check size={14} /> : <Copy size={14} />
									}
									onClick={copy}
								>
									{copied ? "Copied" : "Copy JSON"}
								</Button>
							)}
						</CopyButton>
						<Button size="xs" variant="default" onClick={onClear}>
							Clear
						</Button>
					</Group>
				</Group>

				{systemEvents.length === 0 ? (
					<Text size="sm" c="dimmed">
						No system events yet.
					</Text>
				) : (
					<ScrollArea.Autosize mah={260}>
						<Stack gap="xs">
							{systemEvents.map((event) => {
								const view = buildSystemEventViewModel(event);
								return (
									<Paper key={view.key} withBorder p="sm" radius="md">
										<Stack gap={6}>
											<Group
												justify="space-between"
												align="flex-start"
												gap="xs"
												wrap="wrap"
											>
												<Group gap="xs" wrap="wrap">
													<Badge variant="light" color={view.badgeColor}>
														{view.eventType}
													</Badge>
													<Text size="xs" c="dimmed">
														{view.timeLabel}
													</Text>
												</Group>
											</Group>
											<Text size="sm">{view.message}</Text>
											{view.details ? (
												<Code block style={{ whiteSpace: "pre-wrap" }}>
													{view.details}
												</Code>
											) : null}
										</Stack>
									</Paper>
								);
							})}
						</Stack>
					</ScrollArea.Autosize>
				)}
			</Stack>
		</Paper>
	);
}
