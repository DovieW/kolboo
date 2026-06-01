import {
	Accordion,
	Alert,
	Badge,
	Button,
	Code,
	Collapse,
	CopyButton,
	Group,
	Paper,
	Stack,
	Text,
	Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
	AlertTriangle,
	ArrowRightLeft,
	Braces,
	Bug,
	Check,
	Clock3,
	Copy,
	Info,
	Pause,
	Play,
	X,
} from "lucide-react";
import { useMemo, useState } from "react";
import {
	buildRequestLogViewModel,
	type LogEntryViewModel,
	type LogLevelIconName,
	type LogsTextSection,
	type RequestStatusIconName,
} from "../../lib/logs/readModel";
import type { RequestLog } from "../../lib/tauri";
import type { RecordingPlayerControls } from "../../lib/useRecordingPlayer";
import InlineTextDiff from "../InlineTextDiff";
import LogJsonModal from "../LogJsonModal";

export interface RequestLogItemProps {
	log: RequestLog;
	player: RecordingPlayerControls;
}

function RequestStatusIcon({ icon }: { icon: RequestStatusIconName }) {
	switch (icon) {
		case "success":
			return <Check size={14} />;
		case "error":
			return <X size={14} />;
		case "cancelled":
			return <AlertTriangle size={14} />;
		case "in_progress":
			return <Clock3 size={14} />;
		default:
			return <Info size={14} />;
	}
}

function LogLevelIcon({ icon }: { icon: LogLevelIconName }) {
	switch (icon) {
		case "debug":
			return <Bug size={14} />;
		case "info":
			return <Info size={14} />;
		case "warn":
			return <AlertTriangle size={14} />;
		case "error":
			return <X size={14} />;
		default:
			return <Info size={14} />;
	}
}

function TextSection({ section }: { section: LogsTextSection }) {
	return (
		<Stack gap={4}>
			<Text fw={500} size="sm">
				{section.label}
			</Text>
			<Paper withBorder p="sm" radius="md">
				<Text
					component="pre"
					size="sm"
					style={{ margin: 0, whiteSpace: "pre-wrap" }}
				>
					{section.value}
				</Text>
			</Paper>
		</Stack>
	);
}

function LogEntryItem({ entry }: { entry: LogEntryViewModel }) {
	return (
		<Paper withBorder p="sm" radius="md">
			<Stack gap={6}>
				<Group justify="space-between" align="flex-start" gap="xs" wrap="wrap">
					<Group gap="xs" wrap="wrap">
						<Badge
							variant="light"
							color={entry.levelView.color}
							leftSection={<LogLevelIcon icon={entry.levelView.icon} />}
						>
							{entry.levelView.icon.toUpperCase()}
						</Badge>
						<Text size="xs" c="dimmed">
							{entry.timeLabel}
						</Text>
					</Group>
				</Group>
				<Text size="sm">{entry.message}</Text>
				{entry.details ? (
					<Code block style={{ whiteSpace: "pre-wrap" }}>
						{entry.details}
					</Code>
				) : null}
			</Stack>
		</Paper>
	);
}

export function RequestLogItem({ log, player }: RequestLogItemProps) {
	const [jsonModalOpened, jsonModalHandlers] = useDisclosure(false);
	const [entriesOpened, setEntriesOpened] = useState(false);
	const view = useMemo(() => buildRequestLogViewModel(log), [log]);

	const playLabel = player.isPlaying(log.id)
		? "Pause Recording"
		: player.isLoading(log.id)
			? "Loading Recording..."
			: "Play Recording";

	return (
		<>
			<Accordion.Item value={log.id}>
				<Accordion.Control>
					<Stack gap="xs">
						<Group
							justify="space-between"
							align="flex-start"
							gap="sm"
							wrap="wrap"
						>
							<Group gap="xs" wrap="wrap">
								<Text fw={600}>#{view.id.slice(0, 8)}</Text>
								{view.kindBadge ? (
									<Badge variant="light" color={view.kindBadge.color}>
										{view.kindBadge.label}
									</Badge>
								) : null}
								{view.isManagedRequest ? (
									<Badge variant="light" color="violet">
										Managed
									</Badge>
								) : null}
								<Badge
									color={view.statusView.color}
									leftSection={
										<RequestStatusIcon icon={view.statusView.icon} />
									}
								>
									{view.statusView.label}
								</Badge>
							</Group>
							<Group gap="xs" wrap="wrap">
								{view.totalDurationLabel ? (
									<Text size="sm" c="dimmed">
										{view.totalDurationLabel}
									</Text>
								) : null}
								<Text size="sm" c="dimmed">
									{view.startedAtLabel}
								</Text>
							</Group>
						</Group>

						<Group gap="xs" wrap="wrap">
							<Badge variant="light" color="indigo">
								{view.sttSummaryLabel}
							</Badge>
							{view.quickAskSummaryLabel ? (
								<Badge variant="light" color="orange">
									{view.quickAskSummaryLabel}
								</Badge>
							) : null}
							{view.quickReplaceSummaryLabel ? (
								<Badge variant="light" color="cyan">
									{view.quickReplaceSummaryLabel}
								</Badge>
							) : null}
							{view.llmSummaryLabel ? (
								<Badge variant="light" color="grape">
									{view.llmSummaryLabel}
								</Badge>
							) : null}
							{view.rewriteSkippedSummaryLabel ? (
								<Tooltip label={view.rewriteSkippedTooltip} multiline maw={320}>
									<Badge variant="light" color="yellow">
										{view.rewriteSkippedSummaryLabel}
									</Badge>
								</Tooltip>
							) : null}
							{view.routerSummaryLabel ? (
								<Badge variant="light" color="teal">
									{view.routerSummaryLabel}
								</Badge>
							) : null}
							<Badge variant="outline" color="gray">
								{view.profileSummaryLabel}
							</Badge>
						</Group>

						{view.errorMessage ? (
							<Text size="sm" c="red" lineClamp={2}>
								{view.errorMessage}
							</Text>
						) : null}
					</Stack>
				</Accordion.Control>

				<Accordion.Panel>
					<Stack gap="md">
						<Group
							justify="space-between"
							align="flex-start"
							gap="sm"
							wrap="wrap"
						>
							<Group gap="xs" wrap="wrap">
								{view.copyActions.map((action) => (
									<CopyButton
										key={action.label}
										value={action.value}
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
												{copied ? "Copied" : action.label}
											</Button>
										)}
									</CopyButton>
								))}
							</Group>

							<Group gap="xs" wrap="wrap">
								<Button
									size="xs"
									variant="light"
									leftSection={
										player.isPlaying(log.id) ? (
											<Pause size={14} />
										) : (
											<Play size={14} />
										)
									}
									disabled={view.playDisabled}
									onClick={() => {
										void player.toggle(log.id);
									}}
								>
									{playLabel}
								</Button>
								<Button
									size="xs"
									variant="light"
									leftSection={<Braces size={14} />}
									onClick={jsonModalHandlers.open}
								>
									View JSON
								</Button>
							</Group>
						</Group>

						{view.showQuickAskPanel ? (
							<Paper withBorder p="sm" radius="md">
								<Stack gap="sm">
									<Text fw={600}>Quick Ask</Text>
									{view.quickAskSections.map((section) => (
										<TextSection key={section.label} section={section} />
									))}
								</Stack>
							</Paper>
						) : null}

						{view.showQuickReplacePanel ? (
							<Paper withBorder p="sm" radius="md">
								<Stack gap="sm">
									<Text fw={600}>Quick Replace</Text>
									{view.quickReplaceSections.map((section) => (
										<TextSection key={section.label} section={section} />
									))}
								</Stack>
							</Paper>
						) : null}

						{view.showTranscriptPanel ? (
							<Paper withBorder p="sm" radius="md">
								<Stack gap="sm">
									<Text fw={600}>
										{view.showRewriteTranscript
											? "Transcript + Rewrite"
											: "Transcript"}
									</Text>

									{view.showRewriteTranscript ? (
										<>
											{view.rawTranscriptSection ? (
												<TextSection section={view.rawTranscriptSection} />
											) : null}
											{view.rewriteOutputSection ? (
												<Stack gap={4}>
													<TextSection section={view.rewriteOutputSection} />
													{view.rewriteOutputUnchanged ? (
														<Text size="xs" c="dimmed">
															Rewrite output matches the raw transcript.
														</Text>
													) : null}
												</Stack>
											) : null}
											{view.rewriteClipboardContextSection ? (
												<TextSection
													section={view.rewriteClipboardContextSection}
												/>
											) : null}
											{view.rewriteDiffInfo ? (
												<Paper withBorder p="sm" radius="md">
													<Stack gap="xs">
														<Group gap="xs" wrap="wrap">
															<ArrowRightLeft size={16} />
															<Text fw={500}>Inline Diff</Text>
															<Text size="xs" c="dimmed">
																{view.rewriteDiffInfo.changeGroups} change group
																{view.rewriteDiffInfo.changeGroups === 1
																	? ""
																	: "s"}
															</Text>
														</Group>
														<InlineTextDiff
															chunks={view.rewriteDiffInfo.chunks}
														/>
													</Stack>
												</Paper>
											) : null}
										</>
									) : view.singleTranscriptSection ? (
										<TextSection section={view.singleTranscriptSection} />
									) : null}
								</Stack>
							</Paper>
						) : null}

						{view.errorMessage ? (
							<Alert color="red" icon={<AlertTriangle size={16} />}>
								{view.errorMessage}
							</Alert>
						) : null}

						{view.routerScores.length > 0 ? (
							<Paper withBorder p="sm" radius="md">
								<Stack gap="xs">
									<Text fw={600}>Router Scores</Text>
									<Group gap="xs" wrap="wrap">
										{view.routerScores.map((score) => (
											<Badge
												key={score.key}
												variant={score.selected ? "filled" : "light"}
												color={score.selected ? "teal" : "gray"}
											>
												{score.presetName}: {score.scoreLabel}
											</Badge>
										))}
									</Group>
								</Stack>
							</Paper>
						) : null}

						<Paper withBorder p="sm" radius="md">
							<Stack gap="sm">
								<Group
									justify="space-between"
									align="center"
									gap="xs"
									wrap="wrap"
								>
									<Text fw={600}>Log Entries ({view.logEntries.length})</Text>
									<Button
										size="xs"
										variant="subtle"
										onClick={() => setEntriesOpened((opened) => !opened)}
									>
										{entriesOpened ? "Hide entries" : "Show entries"}
									</Button>
								</Group>
								<Collapse in={entriesOpened}>
									<Stack gap="xs">
										{view.logEntries.map((entry) => (
											<LogEntryItem key={entry.key} entry={entry} />
										))}
									</Stack>
								</Collapse>
							</Stack>
						</Paper>
					</Stack>
				</Accordion.Panel>
			</Accordion.Item>

			<LogJsonModal
				log={log}
				opened={jsonModalOpened}
				onClose={jsonModalHandlers.close}
			/>
		</>
	);
}
