import { Info, RotateCcw } from "lucide-react";
import { SettingsIconButton, SettingsTooltipIcon } from "./SettingsRow";

export function SettingsInheritanceIndicator({
	isDefaultScope,
	inheriting,
	inheritTooltip,
	onDisableOverride,
	disableOverrideLabel,
	disabled,
}: {
	isDefaultScope: boolean;
	inheriting: boolean;
	inheritTooltip: string;
	onDisableOverride: () => void;
	disableOverrideLabel?: string;
	disabled?: boolean;
}) {
	if (isDefaultScope) return null;

	if (inheriting) {
		return (
			<SettingsTooltipIcon label={inheritTooltip}>
				<Info size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
			</SettingsTooltipIcon>
		);
	}

	return (
		<SettingsIconButton
			label={disableOverrideLabel ?? "Disable override (inherit from Default)"}
			onClick={onDisableOverride}
			disabled={disabled}
		>
			<RotateCcw size={14} style={{ opacity: 0.65 }} />
		</SettingsIconButton>
	);
}
