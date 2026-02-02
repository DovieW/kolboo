import type { ReactNode } from "react";

export type HotkeyShortcutCardProps = {
	title: ReactNode;
	description?: ReactNode;
	children: ReactNode;
	actions?: ReactNode;
	footer?: ReactNode;
};

export function HotkeyShortcutCard({
	title,
	description,
	children,
	actions,
	footer,
}: HotkeyShortcutCardProps) {
	return (
		<div className="hotkey-shortcut-card">
			<div className="hotkey-shortcut-card__header">
				<div className="hotkey-shortcut-card__meta">
					<div className="hotkey-shortcut-card__title">{title}</div>
					{description ? (
						<div className="hotkey-shortcut-card__description">
							{description}
						</div>
					) : null}
				</div>
				{actions ? (
					<div className="hotkey-shortcut-card__actions">{actions}</div>
				) : null}
			</div>
			<div className="hotkey-shortcut-card__body">{children}</div>
			{footer ? (
				<div className="hotkey-shortcut-card__footer">{footer}</div>
			) : null}
		</div>
	);
}
