import {
	Badge,
	Button,
	Combobox,
	Group,
	Input,
	InputBase,
	ScrollArea,
	Stack,
	Tabs,
	Text,
	useCombobox,
} from "@mantine/core";
import { useState } from "react";
import {
	type ModelChoice,
	modelSelectionKey,
	submitModelChoice,
} from "../lib/modelSelection";
import type { ModelSelection } from "../lib/tauri/types";
import type { ModelProviderSetup } from "./ModelNavigation";

/** Pure controlled picker: browsing/searching/details never persist a choice. */
export function ModelPicker({
	label,
	value,
	choices,
	onChange,
	disabled = false,
	loading = false,
	managedMessage,
	onRetry,
	onSetup,
	setupProviders = [],
	onInherit,
	onManagedAccess,
	clearable = true,
	legacyLabel,
}: {
	label: string;
	value: ModelSelection | null;
	choices: ModelChoice[];
	onChange: (selection: ModelSelection | null) => void;
	disabled?: boolean;
	loading?: boolean;
	managedMessage?: string;
	onRetry?: () => void;
	onSetup?: (selection: ModelProviderSetup) => void;
	setupProviders?: { provider_id: string; label: string }[];
	onInherit?: () => void;
	onManagedAccess?: () => void;
	clearable?: boolean;
	legacyLabel?: string;
}) {
	const [tab, setTab] = useState<string | null>(value?.source ?? "byok");
	const [search, setSearch] = useState("");
	const combobox = useCombobox({
		onDropdownOpen: () => combobox.focusSearchInput(),
		onDropdownClose: () => {
			combobox.resetSelectedOption();
			setSearch("");
		},
	});
	const selected = value
		? choices.find(
				(choice) =>
					modelSelectionKey(choice.selection) === modelSelectionKey(value),
			)
		: undefined;
	const matching = choices.filter(
		(choice) =>
			choice.selection.source === tab &&
			`${choice.displayName} ${choice.providerName} ${choice.selection.model_id}`
				.toLocaleLowerCase()
				.includes(search.trim().toLocaleLowerCase()),
	);
	return (
		<Stack gap={4}>
			<Combobox
				store={combobox}
				withinPortal={false}
				onOptionSubmit={(key) => {
					submitModelChoice(matching, key, disabled, (selection) => {
						onChange(selection);
						combobox.closeDropdown();
					});
				}}
			>
				<Combobox.Target>
					<InputBase
						component="button"
						type="button"
						label={label}
						aria-label={label}
						disabled={disabled}
						pointer
						rightSection={<Combobox.Chevron />}
						onClick={() => combobox.toggleDropdown()}
						styles={{
							input: { textAlign: "left", height: "auto", minHeight: 36 },
						}}
					>
						{value ? (
							<>
								{selected?.displayName ?? value.model_id} ·{" "}
								{selected?.providerName ?? value.provider_id} ·{" "}
								{value.source === "managed" ? "Managed" : "Your models"}
							</>
						) : legacyLabel ? legacyLabel : (
							<Input.Placeholder>
								{loading ? "Loading models…" : "Choose a model"}
							</Input.Placeholder>
						)}
					</InputBase>
				</Combobox.Target>
				<Combobox.Dropdown>
					<Tabs
						value={tab}
						onChange={(next) => {
							setTab(next);
							combobox.resetSelectedOption();
						}}
					>
						<Tabs.List grow>
							<Tabs.Tab value="managed" disabled={disabled}>
								Kolboo Managed
							</Tabs.Tab>
							<Tabs.Tab value="byok" disabled={disabled}>
								Your models
							</Tabs.Tab>
						</Tabs.List>
					</Tabs>
					<Combobox.Search
						aria-label="Search models"
						disabled={disabled}
						placeholder="Search names, providers or IDs"
						value={search}
						onChange={(event) => {
							setSearch(event.currentTarget.value);
							combobox.resetSelectedOption();
						}}
					/>
					{tab === "managed" && managedMessage && (
						<Text size="sm" c="dimmed" p="xs" role="status">
							{managedMessage}
						</Text>
					)}
					{tab === "managed" && managedMessage && onManagedAccess && (
						<Button size="compact-xs" variant="subtle" onClick={onManagedAccess} disabled={disabled}>Sign in / View account access</Button>
					)}
					<ScrollArea.Autosize mah={320} type="auto">
						<Combobox.Options>
							{matching.map((choice) => (
								<div key={modelSelectionKey(choice.selection)}>
									<Combobox.Option
										value={modelSelectionKey(choice.selection)}
										disabled={disabled || !choice.ready}
										active={
											value !== null &&
											modelSelectionKey(choice.selection) ===
												modelSelectionKey(value)
										}
									>
										<Group justify="space-between" wrap="nowrap">
											<Text size="sm" fw={500}>
												{choice.displayName}
											</Text>
											<Badge
												variant="light"
												color={choice.ready ? "green" : "gray"}
											>
												{choice.ready ? "Ready" : "Unavailable"}
											</Badge>
										</Group>
										<Text size="xs" c="dimmed">
											{choice.providerName} · {choice.capabilities.join(" · ")}
										</Text>
										{choice.reason && <Text size="xs">{choice.reason}</Text>}
									</Combobox.Option>
									<details style={{ padding: "0 12px 8px" }}>
										<summary style={{ cursor: "pointer", fontSize: 12 }}>
											Model details
										</summary>
										<Text size="xs">Model ID: {choice.selection.model_id}</Text>
										<Text size="xs">
											{choice.pricing ??
												"Pricing unavailable — not assumed free."}
										</Text>
										{choice.limitations && (
											<Text size="xs">{choice.limitations}</Text>
										)}
										{!choice.ready && onSetup && (
											<Button
												variant="subtle"
												size="compact-xs"
												disabled={disabled}
												onClick={() => onSetup(choice.selection)}
											>
												Set up provider securely
											</Button>
										)}
									</details>
								</div>
							))}
							{matching.length === 0 && (
								<Combobox.Empty>
									{loading ? "Loading models…" : "No matching models"}
								</Combobox.Empty>
							)}
						</Combobox.Options>
					</ScrollArea.Autosize>
					{tab === "byok" && onSetup && setupProviders.map((provider) => (
						<Button key={provider.provider_id} variant="subtle" size="compact-xs"
							disabled={disabled}
							onClick={() => onSetup({ source: "byok", provider_id: provider.provider_id })}>
							Set up {provider.label}
						</Button>
					))}
					{onRetry && (
						<Button
							variant="subtle"
							size="compact-xs"
							disabled={disabled}
							onClick={onRetry}
						>
							Retry loading models
						</Button>
					)}
					{onInherit && (
						<Button
							variant="subtle"
							size="compact-xs"
							disabled={disabled}
							onClick={() => {
								onInherit();
								combobox.closeDropdown();
							}}
						>
							Use inherited selection
						</Button>
					)}
				</Combobox.Dropdown>
			</Combobox>
			<Group gap="xs">
				{value && clearable && (
					<Button
						variant="subtle"
						size="compact-xs"
						disabled={disabled}
						aria-label={`Clear ${label.toLowerCase()}`}
						onClick={() => onChange(null)}
					>
						Clear choice
					</Button>
				)}
				{value && !selected?.ready && (
					<Text size="xs" c="orange">
						Saved selection unavailable. It has not been replaced.
					</Text>
				)}
			</Group>
		</Stack>
	);
}
