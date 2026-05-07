import { ActionIcon, Group } from "@mantine/core";
import {
	ChevronLeft,
	ChevronRight,
	ChevronsLeft,
	ChevronsRight,
} from "lucide-react";

export function HistoryFeedPagination({
	canGoPrev,
	canGoNext,
	onFirstPage,
	onPreviousPage,
	onNextPage,
	onLastPage,
}: {
	canGoPrev: boolean;
	canGoNext: boolean;
	onFirstPage: () => void;
	onPreviousPage: () => void;
	onNextPage: () => void;
	onLastPage: () => void;
}) {
	return (
		<Group gap={6}>
			<ActionIcon
				variant="subtle"
				size="sm"
				color="gray"
				onClick={onFirstPage}
				disabled={!canGoPrev}
				title="First page"
			>
				<ChevronsLeft size={16} />
			</ActionIcon>
			<ActionIcon
				variant="subtle"
				size="sm"
				color="gray"
				onClick={onPreviousPage}
				disabled={!canGoPrev}
				title="Previous page"
			>
				<ChevronLeft size={16} />
			</ActionIcon>
			<ActionIcon
				variant="subtle"
				size="sm"
				color="gray"
				onClick={onNextPage}
				disabled={!canGoNext}
				title="Next page"
			>
				<ChevronRight size={16} />
			</ActionIcon>
			<ActionIcon
				variant="subtle"
				size="sm"
				color="gray"
				onClick={onLastPage}
				disabled={!canGoNext}
				title="Last page"
			>
				<ChevronsRight size={16} />
			</ActionIcon>
		</Group>
	);
}
