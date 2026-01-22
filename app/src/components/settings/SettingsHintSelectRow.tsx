import type { ReactNode } from "react";

import { SettingsInheritanceIndicator } from "./SettingsInheritance";
import { SettingsRow } from "./SettingsRow";

type SettingsHintSelectRowProps = {
	label: string;
	description: ReactNode;
	isDefaultScope: boolean;
	inheriting: boolean;
	inheritTooltip: string;
	onDisableOverride: () => void;
	disabled?: boolean;
	children: ReactNode;
};

export function SettingsHintSelectRow({
	label,
	description,
	isDefaultScope,
	inheriting,
	inheritTooltip,
	onDisableOverride,
	disabled,
	children,
}: SettingsHintSelectRowProps) {
	return (
		<SettingsRow
			label={label}
			description={description}
			right={
				<>
					<SettingsInheritanceIndicator
						isDefaultScope={isDefaultScope}
						inheriting={inheriting}
						inheritTooltip={inheritTooltip}
						onDisableOverride={onDisableOverride}
						disabled={disabled}
					/>
					{children}
				</>
			}
		/>
	);
}
