import {
	ActionIcon,
	Badge,
	Button,
	Checkbox,
	Divider,
	Group,
	NumberInput,
	Popover,
	Stack,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import {
	ChevronLeft,
	ChevronRight,
	ChevronsLeft,
	ChevronsRight,
	Download,
	Filter,
	Search,
	SlidersHorizontal,
	Trash2,
	X,
} from "lucide-react";
import type { LogsDurationInput } from "../../lib/logs/readModel";

export interface LogsToolbarProps {
	totalLogsCount: number;
	filteredLogsCount: number;
	filterText: string;
	onFilterTextChange: (value: string) => void;
	hasActiveFilters: boolean;
	filtersOpened: boolean;
	onFiltersOpenedChange: (opened: boolean) => void;
	showSuccess: boolean;
	onShowSuccessChange: (value: boolean) => void;
	showError: boolean;
	onShowErrorChange: (value: boolean) => void;
	showCancelled: boolean;
	onShowCancelledChange: (value: boolean) => void;
	durationMinSecs: LogsDurationInput;
	onDurationMinSecsChange: (value: LogsDurationInput) => void;
	durationMaxSecs: LogsDurationInput;
	onDurationMaxSecsChange: (value: LogsDurationInput) => void;
	onResetFilters: () => void;
	page: number;
	totalPages: number;
	onFirstPage: () => void;
	onPreviousPage: () => void;
	onNextPage: () => void;
	onLastPage: () => void;
	exportOpened: boolean;
	onExportOpenedChange: (opened: boolean) => void;
	hasLogs: boolean;
	onExportPrivacySafe: () => void;
	onExportFull: () => void;
	onClearAll: () => void;
	clearAllPending: boolean;
}

export function LogsToolbar({
	totalLogsCount,
	filteredLogsCount,
	filterText,
	onFilterTextChange,
	hasActiveFilters,
	filtersOpened,
	onFiltersOpenedChange,
	showSuccess,
	onShowSuccessChange,
	showError,
	onShowErrorChange,
	showCancelled,
	onShowCancelledChange,
	durationMinSecs,
	onDurationMinSecsChange,
	durationMaxSecs,
	onDurationMaxSecsChange,
	onResetFilters,
	page,
	totalPages,
	onFirstPage,
	onPreviousPage,
	onNextPage,
	onLastPage,
	exportOpened,
	onExportOpenedChange,
	hasLogs,
	onExportPrivacySafe,
	onExportFull,
	onClearAll,
	clearAllPending,
}: LogsToolbarProps) {
	const canGoBack = page > 1;
	const canGoForward = page < totalPages;

	return (
		<Stack gap="sm">
			<Group justify="space-between" align="flex-start" gap="sm" wrap="wrap">
				<Stack gap={2}>
					<Group gap="xs" wrap="wrap">
						<Text fw={700} size="lg">
							Request Logs
						</Text>
						<Badge variant="light" color="gray">
							{filteredLogsCount} shown
						</Badge>
						<Badge variant="outline" color="gray">
							{totalLogsCount} total
						</Badge>
					</Group>
					<Text size="sm" c="dimmed">
						Backend request logs stay the source of truth; this view only
						derives display state, filters, and pagination.
					</Text>
				</Stack>

				<Group gap="xs" wrap="wrap">
					<Popover
						opened={exportOpened}
						onChange={onExportOpenedChange}
						position="bottom-end"
						withArrow
					>
						<Popover.Target>
							<Button
								size="xs"
								variant="light"
								leftSection={<Download size={14} />}
								disabled={!hasLogs}
								onClick={() => onExportOpenedChange(!exportOpened)}
							>
								Export
							</Button>
						</Popover.Target>
						<Popover.Dropdown>
							<Stack gap="xs">
								<Button
									size="xs"
									variant="subtle"
									onClick={onExportPrivacySafe}
								>
									Export privacy-safe JSON
								</Button>
								<Button size="xs" variant="subtle" onClick={onExportFull}>
									Export full debug JSON
								</Button>
							</Stack>
						</Popover.Dropdown>
					</Popover>

					<Button
						size="xs"
						variant="default"
						leftSection={<Trash2 size={14} />}
						disabled={!hasLogs || clearAllPending}
						loading={clearAllPending}
						onClick={onClearAll}
					>
						Clear logs
					</Button>
				</Group>
			</Group>

			<Group justify="space-between" align="center" gap="sm" wrap="wrap">
				<Group gap="xs" wrap="wrap" style={{ flex: 1 }}>
					<TextInput
						style={{ flex: 1, minWidth: 260 }}
						placeholder="Search ids, transcripts, quick actions, or errors"
						value={filterText}
						onChange={(event) => onFilterTextChange(event.currentTarget.value)}
						leftSection={<Search size={14} />}
						rightSection={
							filterText ? (
								<ActionIcon
									variant="subtle"
									color="gray"
									aria-label="Clear logs filter"
									onClick={() => onFilterTextChange("")}
								>
									<X size={14} />
								</ActionIcon>
							) : null
						}
					/>

					<Popover
						opened={filtersOpened}
						onChange={onFiltersOpenedChange}
						position="bottom-start"
						withArrow
					>
						<Popover.Target>
							<Button
								size="sm"
								variant={hasActiveFilters ? "filled" : "light"}
								leftSection={
									hasActiveFilters ? (
										<SlidersHorizontal size={14} />
									) : (
										<Filter size={14} />
									)
								}
								onClick={() => onFiltersOpenedChange(!filtersOpened)}
							>
								Filters
							</Button>
						</Popover.Target>
						<Popover.Dropdown>
							<Stack gap="sm" maw={280}>
								<Text fw={600}>Status</Text>
								<Checkbox
									label="Show success"
									checked={showSuccess}
									onChange={(event) =>
										onShowSuccessChange(event.currentTarget.checked)
									}
								/>
								<Checkbox
									label="Show errors"
									checked={showError}
									onChange={(event) =>
										onShowErrorChange(event.currentTarget.checked)
									}
								/>
								<Checkbox
									label="Show cancelled"
									checked={showCancelled}
									onChange={(event) =>
										onShowCancelledChange(event.currentTarget.checked)
									}
								/>

								<Divider />

								<Text fw={600}>Duration</Text>
								<Group grow>
									<NumberInput
										label="Min seconds"
										placeholder="0"
										min={0}
										value={durationMinSecs}
										onChange={onDurationMinSecsChange}
									/>
									<NumberInput
										label="Max seconds"
										placeholder="∞"
										min={0}
										value={durationMaxSecs}
										onChange={onDurationMaxSecsChange}
									/>
								</Group>

								<Button size="xs" variant="subtle" onClick={onResetFilters}>
									Reset filters
								</Button>
							</Stack>
						</Popover.Dropdown>
					</Popover>
				</Group>

				<Group gap={4} wrap="nowrap">
					<Tooltip label="First page">
						<ActionIcon
							variant="default"
							disabled={!canGoBack}
							aria-label="Go to first logs page"
							onClick={onFirstPage}
						>
							<ChevronsLeft size={16} />
						</ActionIcon>
					</Tooltip>
					<Tooltip label="Previous page">
						<ActionIcon
							variant="default"
							disabled={!canGoBack}
							aria-label="Go to previous logs page"
							onClick={onPreviousPage}
						>
							<ChevronLeft size={16} />
						</ActionIcon>
					</Tooltip>
					<Text size="sm" c="dimmed" miw={84} ta="center">
						Page {page} / {totalPages}
					</Text>
					<Tooltip label="Next page">
						<ActionIcon
							variant="default"
							disabled={!canGoForward}
							aria-label="Go to next logs page"
							onClick={onNextPage}
						>
							<ChevronRight size={16} />
						</ActionIcon>
					</Tooltip>
					<Tooltip label="Last page">
						<ActionIcon
							variant="default"
							disabled={!canGoForward}
							aria-label="Go to last logs page"
							onClick={onLastPage}
						>
							<ChevronsRight size={16} />
						</ActionIcon>
					</Tooltip>
				</Group>
			</Group>
		</Stack>
	);
}
