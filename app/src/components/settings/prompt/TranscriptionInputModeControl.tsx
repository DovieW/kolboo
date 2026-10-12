import { Select, Text } from "@mantine/core";
import type { SttInputMode } from "../../../lib/tauri";
import { normalizeSttInputMode } from "../../../lib/tauri/settingsNormalizers";

/** Shared profile/preset control; browsing does not change a model or its source. */
export function TranscriptionInputModeControl({
	value,
	inheritedMode,
	onChange,
	canInherit = false,
	realtimeOnly = false,
	disabled = false,
}: {
	value: SttInputMode | null;
	inheritedMode: SttInputMode;
	onChange: (mode: SttInputMode | null) => void;
	canInherit?: boolean;
	realtimeOnly?: boolean;
	disabled?: boolean;
}) {
	const effective = value ?? inheritedMode;
	return (
		<div>
			<Select
				label="Input mode"
				aria-label="Transcription input mode"
				allowDeselect={false}
				value={canInherit && value === null ? "inherit" : effective}
				disabled={disabled}
				data={[
					...(canInherit
						? [
								{
									value: "inherit",
									label: `Inherit (${inheritedMode === "auto" ? "Auto" : "After recording"})`,
								},
							]
						: []),
					{ value: "auto", label: "Auto" },
					{
						value: "after_recording",
						label: "After recording",
						disabled: realtimeOnly,
					},
				]}
				onChange={(mode) =>
					onChange(normalizeSttInputMode(mode === "inherit" ? null : mode))
				}
				description="Auto uses genuine realtime input when supported. After recording uploads only after you stop."
			/>
			{realtimeOnly && (
				<Text
					size="xs"
					c={effective === "after_recording" ? "orange" : "dimmed"}
				>
					This model requires realtime input. After recording is unavailable;
					change the model or choose Auto.
				</Text>
			)}
		</div>
	);
}
