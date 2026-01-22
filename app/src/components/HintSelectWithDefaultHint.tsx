import { Text } from "@mantine/core";
import type { CSSProperties } from "react";

import { HintSelect, type HintSelectOption } from "./HintSelect";

export function HintSelectWithDefaultHint({
	data,
	value,
	onChange,
	placeholder,
	disabled,
	inputStyle,
	defaultValue,
	defaultHint,
	withinPortal,
}: {
	data: HintSelectOption[];
	value: string | null;
	onChange: (value: string | null) => void;
	placeholder?: string;
	disabled?: boolean;
	inputStyle?: CSSProperties;
	defaultValue: string;
	defaultHint: string;
	withinPortal?: boolean;
}) {
	return (
		<HintSelect
			data={data}
			value={value}
			onChange={onChange}
			placeholder={placeholder}
			disabled={disabled}
			inputStyle={inputStyle}
			withinPortal={withinPortal}
			renderSelected={({ option, placeholder: resolvedPlaceholder }) => {
				if (!option) {
					return (
						<Text size="sm" c="dimmed">
							{resolvedPlaceholder}
						</Text>
					);
				}

				if (option.value !== defaultValue) {
					return <Text size="sm">{option.label}</Text>;
				}

				return (
					<div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
						<span style={{ fontSize: 14 }}>{option.label}</span>
						<span
							style={{
								fontSize: 11,
								color: "var(--text-muted)",
								opacity: 0.9,
								lineHeight: 1,
							}}
						>
							· {defaultHint}
						</span>
					</div>
				);
			}}
			renderOption={({ option }) => {
				if (option.value !== defaultValue) {
					return <Text size="sm">{option.label}</Text>;
				}

				return (
					<div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
						<span style={{ fontSize: 14 }}>{option.label}</span>
						<span
							style={{
								fontSize: 11,
								color: "var(--text-muted)",
								opacity: 0.9,
								lineHeight: 1,
							}}
						>
							· {defaultHint}
						</span>
					</div>
				);
			}}
		/>
	);
}
