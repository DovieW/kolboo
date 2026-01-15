import { Combobox, Input, InputBase, useCombobox } from "@mantine/core";
import type { CSSProperties, ReactNode } from "react";

export type HintSelectOption = {
	value: string;
	label: string;
	disabled?: boolean;
};

export function HintSelect({
	data,
	value,
	onChange,
	placeholder,
	disabled,
	inputStyle,
	renderOption,
	renderSelected,
	withinPortal,
}: {
	data: HintSelectOption[];
	value: string | null;
	onChange: (value: string | null) => void;
	placeholder?: string;
	disabled?: boolean;
	inputStyle?: CSSProperties;
	renderOption?: (args: {
		option: HintSelectOption;
		selected: boolean;
	}) => ReactNode;
	renderSelected?: (args: {
		option: HintSelectOption | null;
		placeholder: string;
	}) => ReactNode;
	withinPortal?: boolean;
}) {
	const combobox = useCombobox({
		onDropdownClose: () => combobox.resetSelectedOption(),
	});

	const selected = value ? (data.find((o) => o.value === value) ?? null) : null;
	const resolvedPlaceholder = placeholder ?? "Select";

	return (
		<Combobox
			store={combobox}
			withinPortal={withinPortal ?? false}
			onOptionSubmit={(val) => {
				onChange(val);
				combobox.closeDropdown();
			}}
		>
			<Combobox.Target>
				<InputBase
					component="button"
					type="button"
					pointer
					disabled={disabled}
					rightSection={<Combobox.Chevron />}
					onClick={() => {
						if (disabled) return;
						combobox.toggleDropdown();
					}}
					styles={{
						input: {
							...(inputStyle ?? {}),
							textAlign: "left",
						},
					}}
				>
					{renderSelected ? (
						renderSelected({
							option: selected,
							placeholder: resolvedPlaceholder,
						})
					) : selected ? (
						selected.label
					) : (
						<Input.Placeholder>{resolvedPlaceholder}</Input.Placeholder>
					)}
				</InputBase>
			</Combobox.Target>

			<Combobox.Dropdown>
				<Combobox.Options>
					{data.map((option) => (
						<Combobox.Option
							key={option.value}
							value={option.value}
							disabled={option.disabled}
						>
							{renderOption
								? renderOption({
										option,
										selected: option.value === value,
									})
								: option.label}
						</Combobox.Option>
					))}
				</Combobox.Options>
			</Combobox.Dropdown>
		</Combobox>
	);
}
