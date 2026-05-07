import { Button, Text } from "@mantine/core";
import type { ReactNode } from "react";
import type { DataStorageBreakdownItem } from "../../../lib/settings/dataLifecycle";

export type DangerZoneAction = {
	key: string;
	label: string;
	icon: ReactNode;
	variant: "outline" | "filled";
	color: string;
	fullWidth?: boolean;
	onClick: () => void;
};

type DataDangerZoneSectionProps = {
	storageSummaryLoading: boolean;
	storageBreakdownItems: DataStorageBreakdownItem[];
	actions: DangerZoneAction[];
};

export function DataDangerZoneSection({
	storageSummaryLoading,
	storageBreakdownItems,
	actions,
}: DataDangerZoneSectionProps) {
	return (
		<div
			style={{
				marginTop: 16,
				border: "1px solid rgba(239, 68, 68, 0.20)",
				borderRadius: 12,
				padding: 12,
				background: "rgba(239, 68, 68, 0.05)",
			}}
		>
			<div>
				<p
					className="settings-label"
					style={{ color: "rgba(255, 150, 150, 0.95)" }}
				>
					Danger zone
				</p>
				<p className="settings-description">
					Destructive actions (cannot be undone)
				</p>

				<div
					style={{
						marginTop: 8,
						display: "grid",
						gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))",
						gap: 12,
						alignItems: "start",
					}}
				>
					<div>
						{storageSummaryLoading ? (
							<Text size="xs" c="dimmed">
								Calculating what’s stored…
							</Text>
						) : storageBreakdownItems.length > 0 ? (
							<div
								style={{
									display: "grid",
									gridTemplateColumns: "auto 1fr",
									gap: "2px 12px",
									alignItems: "baseline",
								}}
							>
								{storageBreakdownItems.map((item) => (
									<div
										key={item.label}
										style={{
											display: "contents",
										}}
									>
										<Text size="xs" c="dimmed">
											{item.label}
										</Text>
										<Text size="xs" c="dimmed">
											{item.value}
										</Text>
									</div>
								))}
							</div>
						) : null}
					</div>

					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
							gap: 8,
							width: "min(560px, 100%)",
						}}
					>
						{actions.map((action) => (
							<Button
								key={action.key}
								color={action.color}
								variant={action.variant}
								size="xs"
								leftSection={action.icon}
								style={action.fullWidth ? { gridColumn: "1 / -1" } : undefined}
								onClick={action.onClick}
							>
								{action.label}
							</Button>
						))}
					</div>
				</div>
			</div>
		</div>
	);
}
