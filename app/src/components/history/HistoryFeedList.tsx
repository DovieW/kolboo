import { ActionIcon, Badge, Group, Loader, Text, Tooltip } from "@mantine/core";
import {
	Check,
	Copy,
	FileText,
	MessageSquare,
	Pause,
	Play,
	RotateCcw,
	Trash2,
} from "lucide-react";
import type {
	GroupedHistoryViewModel,
	HistoryFeedEmptyState,
} from "../../lib/history/readModel";

export function HistoryFeedList({
	isInitialLoading,
	hasError,
	emptyState,
	groupedHistory,
	copiedEntryId,
	onCopyEntry,
	onRetryEntry,
	isRetryPending,
	retryPendingEntryId,
	recordingExistsById,
	isRecordingPlaying,
	isRecordingLoading,
	onToggleRecording,
	requestLogIds,
	onJumpToLog,
	onDeleteEntry,
	isDeleteDisabled,
}: {
	isInitialLoading: boolean;
	hasError: boolean;
	emptyState: HistoryFeedEmptyState | null;
	groupedHistory: GroupedHistoryViewModel[];
	copiedEntryId: string | null;
	onCopyEntry: (entryId: string, text: string | null | undefined) => void;
	onRetryEntry: (entryId: string) => void;
	isRetryPending: boolean;
	retryPendingEntryId?: string;
	recordingExistsById: Map<string, { exists: boolean; checkedAt: number }>;
	isRecordingPlaying: (recordingId: string) => boolean;
	isRecordingLoading: (recordingId: string) => boolean;
	onToggleRecording: (recordingId: string) => void;
	requestLogIds: Set<string>;
	onJumpToLog?: (logId: string) => void;
	onDeleteEntry: (entryId: string) => void;
	isDeleteDisabled: boolean;
}) {
	if (isInitialLoading) {
		return (
			<div className="empty-state">
				<p className="empty-state-text">Loading history...</p>
			</div>
		);
	}

	if (hasError) {
		return (
			<div className="empty-state">
				<p className="empty-state-text" style={{ color: "#ef4444" }}>
					Failed to load history
				</p>
			</div>
		);
	}

	if (emptyState) {
		return (
			<div className="empty-state">
				<MessageSquare className="empty-state-icon" />
				<h4 className="empty-state-title">{emptyState.title}</h4>
				<p className="empty-state-text">{emptyState.message}</p>
			</div>
		);
	}

	return (
		<>
			{groupedHistory.map((group) => (
				<div key={group.date} style={{ marginBottom: 24 }}>
					<p
						className="section-title"
						style={{ marginBottom: 12, fontSize: 11 }}
					>
						{group.date}
					</p>
					<div className="history-feed">
						{group.items.map((entry) => {
							const recordingId = entry.recordingRequestId ?? "";
							const cached = recordingId
								? recordingExistsById.get(recordingId)
								: undefined;
							const isKnownMissing = !recordingId || cached?.exists === false;
							const isPlaying = recordingId
								? isRecordingPlaying(recordingId)
								: false;
							const isInProgress = entry.contentKind === "in_progress";
							const wrapStyle = {
								whiteSpace: "pre-wrap",
								overflowWrap: "anywhere",
								wordBreak: "break-word",
							} as const;

							return (
								<div key={entry.id} className="history-item">
									<button
										type="button"
										className="history-item-button"
										onClick={() => onCopyEntry(entry.id, entry.copyValue)}
										title={entry.hasCopyValue ? "Click to copy" : undefined}
										disabled={!entry.hasCopyValue}
									>
										<span className="history-time">{entry.timestampLabel}</span>
										<div className="history-text">
											{entry.contentKind === "in_progress" ? (
												<Group gap={8} wrap="nowrap" style={{ minWidth: 0 }}>
													<Loader size="xs" color="orange" />
													<Text size="sm" c="dimmed" style={{ minWidth: 0 }}>
														{entry.displayText}
													</Text>
												</Group>
											) : entry.contentKind === "error" ? (
												<Group
													gap={8}
													wrap="nowrap"
													align="flex-start"
													style={{ minWidth: 0 }}
												>
													<Text size="sm" c="red">
														Failed
													</Text>
													<Text
														size="sm"
														c="dimmed"
														style={{ flex: 1, minWidth: 0, ...wrapStyle }}
														title={entry.displayTitle}
													>
														{entry.displayText}
													</Text>
												</Group>
											) : (
												<Text
													size="sm"
													c={
														entry.contentKind === "empty" ? "dimmed" : undefined
													}
													style={
														entry.contentKind === "empty"
															? { ...wrapStyle, fontStyle: "italic" }
															: wrapStyle
													}
													title={entry.displayTitle}
												>
													{entry.displayText}
												</Text>
											)}
										</div>
									</button>
									<div className="history-actions">
										{entry.profilePresetLabel ? (
											<Badge size="xs" variant="light" color="gray">
												{entry.profilePresetLabel}
											</Badge>
										) : null}
										<Tooltip
											label={copiedEntryId === entry.id ? "Copied" : "Copy"}
											withArrow
										>
											<ActionIcon
												variant="subtle"
												size="sm"
												color="gray"
												onClick={(event) => {
													event.stopPropagation();
													onCopyEntry(entry.id, entry.copyValue);
												}}
												disabled={!entry.hasCopyValue}
												aria-label="Copy"
											>
												<span
													className={
														"history-copy-icon" +
														(copiedEntryId === entry.id
															? " history-copy-icon--checked"
															: "")
													}
												>
													{copiedEntryId === entry.id ? (
														<Check size={14} />
													) : (
														<Copy size={14} />
													)}
												</span>
											</ActionIcon>
										</Tooltip>

										{!isKnownMissing ? (
											<Tooltip
												label={isInProgress ? "Already transcribing" : "Rerun"}
												withArrow
											>
												<ActionIcon
													variant="subtle"
													size="sm"
													color="gray"
													disabled={isInProgress}
													loading={
														isRetryPending && retryPendingEntryId === entry.id
													}
													onClick={(event) => {
														event.stopPropagation();
														onRetryEntry(entry.id);
													}}
													aria-label="Rerun"
												>
													<RotateCcw size={14} />
												</ActionIcon>
											</Tooltip>
										) : null}

										<Tooltip
											label={
												isKnownMissing
													? "No recording"
													: isPlaying
														? "Pause"
														: "Play"
											}
											withArrow
										>
											<ActionIcon
												variant="subtle"
												size="sm"
												color="gray"
												disabled={isInProgress || isKnownMissing}
												loading={
													recordingId ? isRecordingLoading(recordingId) : false
												}
												onClick={(event) => {
													event.stopPropagation();
													if (!recordingId) return;
													onToggleRecording(recordingId);
												}}
												aria-label={
													isKnownMissing
														? "No recording"
														: isPlaying
															? "Pause"
															: "Play"
												}
											>
												{isPlaying ? <Pause size={14} /> : <Play size={14} />}
											</ActionIcon>
										</Tooltip>

										{onJumpToLog && requestLogIds.has(entry.id) ? (
											<Tooltip label="Log" withArrow>
												<ActionIcon
													variant="subtle"
													size="sm"
													color="gray"
													onClick={(event) => {
														event.stopPropagation();
														onJumpToLog(entry.id);
													}}
													aria-label="Log"
												>
													<FileText size={14} />
												</ActionIcon>
											</Tooltip>
										) : null}

										<Tooltip label="Delete" withArrow>
											<ActionIcon
												variant="subtle"
												size="sm"
												color="red"
												onClick={(event) => {
													event.stopPropagation();
													onDeleteEntry(entry.id);
												}}
												disabled={isDeleteDisabled}
												aria-label="Delete"
											>
												<Trash2 size={14} />
											</ActionIcon>
										</Tooltip>
									</div>
								</div>
							);
						})}
					</div>
				</div>
			))}
		</>
	);
}
