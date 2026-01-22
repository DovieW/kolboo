import { ActionIcon, Tooltip } from "@mantine/core";
import type { ReactNode } from "react";

type SettingsRowProps =
	| {
			left: ReactNode;
			right: ReactNode;
			className?: string;
			noDivider?: boolean;
			label?: never;
			description?: never;
	}
	| {
			label: ReactNode;
			description?: ReactNode;
			right: ReactNode;
			className?: string;
			noDivider?: boolean;
			left?: never;
	};

export function SettingsRow({
	left,
	label,
	description,
	right,
	className,
	noDivider,
}: SettingsRowProps) {
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
				{left ?? (
					<>
						<div className="settings-label">{label}</div>
						{description ? (
							<div className="settings-description">{description}</div>
						) : null}
					</>
				)}
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
