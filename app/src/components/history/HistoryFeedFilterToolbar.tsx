import {
	ActionIcon,
	Badge,
	Box,
	Button,
	Checkbox,
	Collapse,
	Divider,
	Group,
	Indicator,
	Popover,
	ScrollArea,
	Stack,
	Switch,
	Text,
	TextInput,
	Tooltip,
	UnstyledButton,
} from "@mantine/core";
import {
	Bot,
	ChevronDown,
	Filter,
	FolderOpen,
	RotateCcw,
	Search,
	Trash2,
	X,
} from "lucide-react";
import type { ReactNode } from "react";
import type { HistoryFeedFilterSection } from "../../lib/history/useHistoryFeedFilters";

type HistoryModelOption = {
	key: string;
	label: string;
};

function HistoryModelFilterSection({
	title,
	selectedKeys,
	expanded,
	onToggle,
	options,
	usageCounts,
	onChange,
}: {
	title: string;
	selectedKeys: string[];
	expanded: boolean;
	onToggle: () => void;
	options: HistoryModelOption[];
	usageCounts: Map<string, number>;
	onChange: (value: string[]) => void;
}) {
	return (
		<Box>
			<UnstyledButton
				onClick={onToggle}
				w="100%"
				py={8}
				px="xs"
				style={{
					display: "flex",
					alignItems: "center",
					justifyContent: "space-between",
				}}
			>
				<Group gap={8}>
					<Text size="xs" fw={500}>
						{title}
					</Text>
					{selectedKeys.length > 0 ? (
						<Badge size="xs" variant="filled" color="orange" circle>
							{selectedKeys.length}
						</Badge>
					) : null}
				</Group>
				<ChevronDown
					size={14}
					style={{
						transform: expanded ? "rotate(180deg)" : "rotate(0)",
						transition: "transform 150ms ease",
						color: "var(--text-secondary)",
					}}
				/>
			</UnstyledButton>
			<Collapse expanded={expanded}>
				<Box px="xs" pb="xs">
					{options.length === 0 ? (
						<Text c="dimmed" size="xs">
							No {title.toLowerCase()} available.
						</Text>
					) : (
						<ScrollArea.Autosize mah={140} type="auto" offsetScrollbars>
							<Checkbox.Group value={selectedKeys} onChange={onChange}>
								<Stack gap={6}>
									{options.map((option) => {
										const count = usageCounts.get(option.key) ?? 0;
										return (
											<Checkbox
												key={option.key}
												value={option.key}
												size="xs"
												label={
													<Group gap={6} wrap="nowrap">
														<Text size="xs" style={{ flex: 1 }}>
															{option.label}
														</Text>
														<Badge
															size="xs"
															variant="light"
															color={count > 0 ? "gray" : "dark"}
															styles={{
																root: {
																	minWidth: 24,
																	height: 16,
																	padding: "0 4px",
																},
															}}
														>
															{count}
														</Badge>
													</Group>
												}
												styles={{
													label: { width: "100%" },
													body: { alignItems: "center" },
												}}
											/>
										);
									})}
								</Stack>
							</Checkbox.Group>
						</ScrollArea.Autosize>
					)}
				</Box>
			</Collapse>
		</Box>
	);
}

export function HistoryFeedFilterToolbar({
	retryLastFailedTooltip,
	onRetryLastFailed,
	canRetryLastFailed,
	isRetryPending,
	openRecordingsTooltip,
	onOpenRecordingsFolder,
	onOpenAnalysis,
	onOpenDeleteAll,
	isDeleteAllPending,
	filterText,
	onFilterTextChange,
	onClearFilter,
	filtersOpened,
	onFiltersOpenedChange,
	onToggleFilters,
	hasActiveFilters,
	onResetFilters,
	showFailed,
	onShowFailedChange,
	showEmptyTranscript,
	onShowEmptyTranscriptChange,
	filtersExpandedSection,
	onFiltersExpandedSectionChange,
	availableSttModelOptions,
	sttModelUsageCounts,
	selectedSttModelKeys,
	onSelectedSttModelKeysChange,
	availableLlmModelOptions,
	llmModelUsageCounts,
	selectedLlmModelKeys,
	onSelectedLlmModelKeysChange,
	totalFilteredCount,
	pagination,
}: {
	retryLastFailedTooltip: string;
	onRetryLastFailed: () => void;
	canRetryLastFailed: boolean;
	isRetryPending: boolean;
	openRecordingsTooltip: string;
	onOpenRecordingsFolder: () => void;
	onOpenAnalysis: () => void;
	onOpenDeleteAll: () => void;
	isDeleteAllPending: boolean;
	filterText: string;
	onFilterTextChange: (value: string) => void;
	onClearFilter: () => void;
	filtersOpened: boolean;
	onFiltersOpenedChange: (opened: boolean) => void;
	onToggleFilters: () => void;
	hasActiveFilters: boolean;
	onResetFilters: () => void;
	showFailed: boolean;
	onShowFailedChange: (value: boolean) => void;
	showEmptyTranscript: boolean;
	onShowEmptyTranscriptChange: (value: boolean) => void;
	filtersExpandedSection: HistoryFeedFilterSection | null;
	onFiltersExpandedSectionChange: (
		section: HistoryFeedFilterSection | null,
	) => void;
	availableSttModelOptions: HistoryModelOption[];
	sttModelUsageCounts: Map<string, number>;
	selectedSttModelKeys: string[];
	onSelectedSttModelKeysChange: (keys: string[]) => void;
	availableLlmModelOptions: HistoryModelOption[];
	llmModelUsageCounts: Map<string, number>;
	selectedLlmModelKeys: string[];
	onSelectedLlmModelKeysChange: (keys: string[]) => void;
	totalFilteredCount: number;
	pagination: ReactNode;
}) {
	return (
		<div className="history-feed-toolbar">
			<div className="section-header">
				<span className="section-title section-title--no-accent">History</span>
				<Group gap={6}>
					<Tooltip label={retryLastFailedTooltip} withArrow>
						<Button
							variant="subtle"
							size="compact-sm"
							color="gray"
							px={6}
							onClick={onRetryLastFailed}
							disabled={!canRetryLastFailed || isRetryPending}
							aria-label="Retry last failed request"
						>
							<RotateCcw size={14} />
						</Button>
					</Tooltip>

					<Tooltip label={openRecordingsTooltip} withArrow>
						<Button
							variant="subtle"
							size="compact-sm"
							color="gray"
							px={6}
							onClick={onOpenRecordingsFolder}
							aria-label="Open recordings folder"
						>
							<FolderOpen size={14} />
						</Button>
					</Tooltip>

					<Tooltip label="Analyze transcripts" withArrow>
						<Button
							variant="subtle"
							size="compact-sm"
							color="gray"
							px={6}
							onClick={onOpenAnalysis}
							aria-label="Analyze transcripts"
						>
							<Bot size={14} />
						</Button>
					</Tooltip>

					<Tooltip label="Delete transcripts and recordings" withArrow>
						<Button
							variant="subtle"
							size="compact-sm"
							color="red"
							px={6}
							onClick={onOpenDeleteAll}
							disabled={isDeleteAllPending}
							aria-label="Delete transcripts and recordings"
						>
							<Trash2 size={14} />
						</Button>
					</Tooltip>
				</Group>
			</div>

			<div className="history-feed-toolbar__controls">
				<TextInput
					value={filterText}
					onChange={(event) => onFilterTextChange(event.currentTarget.value)}
					placeholder="Filter history…"
					leftSection={<Search size={14} />}
					rightSection={
						filterText.trim().length > 0 ? (
							<ActionIcon
								variant="subtle"
								size="sm"
								color="gray"
								onClick={onClearFilter}
								title="Clear filter"
							>
								<X size={14} />
							</ActionIcon>
						) : null
					}
					styles={{
						input: {
							backgroundColor: "transparent",
							borderColor: "var(--border-default)",
							color: "var(--text-primary)",
						},
					}}
					size="xs"
				/>

				<Popover
					opened={filtersOpened}
					onChange={onFiltersOpenedChange}
					position="bottom-end"
					shadow="lg"
					radius="md"
				>
					<Popover.Target>
						<Indicator
							size={8}
							offset={2}
							disabled={!hasActiveFilters}
							processing={hasActiveFilters}
						>
							<ActionIcon
								variant={hasActiveFilters ? "light" : "subtle"}
								size="sm"
								color={hasActiveFilters ? "orange" : "gray"}
								onClick={onToggleFilters}
								title="Filter options"
								aria-label="Filter options"
							>
								<Filter size={16} />
							</ActionIcon>
						</Indicator>
					</Popover.Target>
					<Popover.Dropdown
						p={0}
						style={{
							backgroundColor: "var(--bg-elevated)",
							border: "1px solid var(--border-default)",
							width: 280,
							overflow: "hidden",
						}}
					>
						<Group justify="space-between" p="xs" pb={8}>
							<Text size="sm" fw={600}>
								Filters
							</Text>
							{hasActiveFilters ? (
								<Button
									variant="subtle"
									size="compact-xs"
									color="gray"
									onClick={onResetFilters}
									styles={{ root: { height: 20, padding: "0 6px" } }}
								>
									Reset
								</Button>
							) : null}
						</Group>

						<Divider color="var(--border-default)" />

						<Stack gap={0} p="xs">
							<Group justify="space-between" py={4}>
								<Text size="xs">Show failed</Text>
								<Switch
									size="xs"
									checked={showFailed}
									onChange={(event) =>
										onShowFailedChange(event.currentTarget.checked)
									}
								/>
							</Group>
							<Group justify="space-between" py={4}>
								<Text size="xs">Show empty transcripts</Text>
								<Switch
									size="xs"
									checked={showEmptyTranscript}
									onChange={(event) =>
										onShowEmptyTranscriptChange(event.currentTarget.checked)
									}
								/>
							</Group>
						</Stack>

						<Divider color="var(--border-default)" />

						<HistoryModelFilterSection
							title="STT Models"
							selectedKeys={selectedSttModelKeys}
							expanded={filtersExpandedSection === "stt"}
							onToggle={() =>
								onFiltersExpandedSectionChange(
									filtersExpandedSection === "stt" ? null : "stt",
								)
							}
							options={availableSttModelOptions}
							usageCounts={sttModelUsageCounts}
							onChange={onSelectedSttModelKeysChange}
						/>

						<Divider color="var(--border-default)" />

						<HistoryModelFilterSection
							title="LLM Models"
							selectedKeys={selectedLlmModelKeys}
							expanded={filtersExpandedSection === "llm"}
							onToggle={() =>
								onFiltersExpandedSectionChange(
									filtersExpandedSection === "llm" ? null : "llm",
								)
							}
							options={availableLlmModelOptions}
							usageCounts={llmModelUsageCounts}
							onChange={onSelectedLlmModelKeysChange}
						/>
					</Popover.Dropdown>
				</Popover>

				<Text c="dimmed" size="xs" style={{ whiteSpace: "nowrap" }}>
					{totalFilteredCount} result{totalFilteredCount === 1 ? "" : "s"}
				</Text>

				<Group style={{ marginLeft: "auto" }} gap={6}>
					{pagination}
				</Group>
			</div>
		</div>
	);
}
