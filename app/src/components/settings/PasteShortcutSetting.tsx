import { ActionIcon, Select, Tooltip } from "@mantine/core";
import { RotateCcw } from "lucide-react";
import { useUpdateOutputPasteShortcut } from "../../lib/queries";
import type {
	PasteShortcut,
	RewriteProgramPromptProfile,
} from "../../lib/tauri";
import { normalizePasteShortcut } from "../../lib/tauri/settingsNormalizers";
import { SettingsRow } from "./SettingsRow";

const OPTIONS = [
	{ value: "system", label: "System default (Ctrl / ⌘ + V)" },
	{ value: "ctrl_v", label: "Ctrl+V" },
	{ value: "ctrl_shift_v", label: "Ctrl+Shift+V" },
	{ value: "shift_insert", label: "Shift+Insert" },
	{ value: "cmd_v", label: "⌘+V" },
];

export function PasteShortcutSetting({
	globalValue = "system",
	profile,
	disabled,
	updateProfile,
}: {
	globalValue?: PasteShortcut;
	profile: RewriteProgramPromptProfile | null;
	disabled: boolean;
	updateProfile: (patch: Partial<RewriteProgramPromptProfile>) => void;
}) {
	const updateGlobal = useUpdateOutputPasteShortcut();
	const override = normalizePasteShortcut(profile?.output_paste_shortcut);
	return (
		<SettingsRow
			label="Paste shortcut"
			right={
				<>
					{profile && override !== null && (
						<Tooltip label="Use Default profile shortcut">
							<ActionIcon
								aria-label="Reset paste shortcut"
								variant="subtle"
								color="gray"
								disabled={disabled}
								onClick={() => updateProfile({ output_paste_shortcut: null })}
							>
								<RotateCcw size={14} />
							</ActionIcon>
						</Tooltip>
					)}
					<Select
						aria-label="Paste shortcut"
						data={OPTIONS}
						value={override ?? globalValue}
						disabled={disabled}
						allowDeselect={false}
						w={260}
						onChange={(value) => {
							const shortcut = value as PasteShortcut;
							if (profile) updateProfile({ output_paste_shortcut: shortcut });
							else updateGlobal.mutate(shortcut);
						}}
					/>
				</>
			}
		/>
	);
}
