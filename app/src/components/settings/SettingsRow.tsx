import { ActionIcon, Tooltip } from "@mantine/core";
import type { ReactNode } from "react";

export function SettingsRow({
	label,
	description,
	right,
	className,
	noDivider,
}: {
	label: ReactNode;
	description?: ReactNode;
	right: ReactNode;
	className?: string;
	noDivider?: boolean;
}) {
	return (
		<div
			className={[
				"settings-row",
				noDivider ? "no-divider" : null,
				className ?? null,
			]
				.filter(Boolean)
				.join(" ")}
		>
			<div>
				<p className="settings-label">{label}</p>
				{description ? (
					<p className="settings-description">{description}</p>
				) : null}
			</div>
			<div className="settings-row-actions">{right}</div>
		</div>
	);
}

export function SettingsIconButton({
	label,
	children,
	onClick,
	disabled,
}: {
	label: string;
	children: ReactNode;
	onClick: () => void;
	disabled?: boolean;
}) {
	return (
		<Tooltip label={label} withArrow>
			<ActionIcon
				variant="subtle"
				color="gray"
				size="sm"
				disabled={disabled}
				onClick={onClick}
			>
				{children}
			</ActionIcon>
		</Tooltip>
	);
}

export function SettingsTooltipIcon({
	label,
	children,
}: {
	label: string;
	children: ReactNode;
}) {
	return (
		<Tooltip label={label} withArrow>
			<span style={{ display: "inline-flex" }}>{children}</span>
		</Tooltip>
	);
}
