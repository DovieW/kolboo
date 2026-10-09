import {
	ActionIcon,
	Button,
	Group,
	Stack,
	Text,
	Textarea,
	TextInput,
} from "@mantine/core";
import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";

export type CustomModelRow = { id: string; name: string };

export function customModelRows(
	ids: string[],
	names: Record<string, string | undefined> = {},
): CustomModelRow[] {
	return ids.map((id) => ({
		id,
		name: Object.hasOwn(names, id) ? (names[id] ?? "") : "",
	}));
}

export function customModelMetadata(rows: CustomModelRow[]) {
	const ids: string[] = [];
	const names: Record<string, string> = Object.create(null);
	for (const row of rows) {
		const id = row.id.trim();
		const name = row.name.trim();
		if (!id) throw new Error("Enter a model ID or remove the empty row.");
		if (ids.includes(id))
			throw new Error("Each model ID must be unique within its type.");
		if (
			new TextEncoder().encode(id).length > 200 ||
			Array.from(name).length > 100 ||
			Array.from(id + name).some((character) => {
				const code = character.charCodeAt(0);
				return code < 32 || (code >= 127 && code <= 159);
			})
		)
			throw new Error(
				"Use model IDs up to 200 characters and display names up to 100, without control characters.",
			);
		ids.push(id);
		if (name) names[id] = name;
	}
	if (ids.length > 100) throw new Error("Use at most 100 models per type.");
	return { ids, names };
}

export function CustomModelEditor({
	label,
	rows,
	onChange,
	disabled,
}: {
	label: string;
	rows: CustomModelRow[];
	onChange: (rows: CustomModelRow[]) => void;
	disabled: boolean;
}) {
	const [bulk, setBulk] = useState<string | null>(null);
	return (
		<Stack gap="xs">
			<Group justify="space-between">
				<Text size="sm" fw={500}>
					{label}
				</Text>
				<Button
					variant="subtle"
					size="compact-xs"
					disabled={disabled}
					onClick={() => setBulk(bulk === null ? "" : null)}
				>
					{bulk === null
						? `Bulk add ${label.toLowerCase()}`
						: "Close bulk entry"}
				</Button>
			</Group>
			{rows.map((row, index) => (
				// Rows are fully controlled; IDs are editable and may temporarily be empty or duplicate.
				// biome-ignore lint/suspicious/noArrayIndexKey: No per-row local state or uncontrolled values.
				<Group key={index} gap="xs" align="end" wrap="nowrap">
					<TextInput
						label="Model ID"
						aria-label={`${label} model ID ${index + 1}`}
						value={row.id}
						maxLength={200}
						disabled={disabled}
						onChange={(event) =>
							onChange(
								rows.map((current, i) =>
									i === index
										? { ...current, id: event.currentTarget.value }
										: current,
								),
							)
						}
						style={{ flex: 1 }}
					/>
					<TextInput
						label="Display name"
						aria-label={`${label} display name ${index + 1}`}
						placeholder="Optional"
						value={row.name}
						maxLength={100}
						disabled={disabled}
						onChange={(event) =>
							onChange(
								rows.map((current, i) =>
									i === index
										? { ...current, name: event.currentTarget.value }
										: current,
								),
							)
						}
						style={{ flex: 1 }}
					/>
					<ActionIcon
						variant="subtle"
						aria-label={`Remove ${label.toLowerCase()} model ${index + 1}`}
						disabled={disabled}
						onClick={() => onChange(rows.filter((_, i) => i !== index))}
					>
						<Trash2 size={15} />
					</ActionIcon>
				</Group>
			))}
			{bulk !== null && (
				<>
					<Textarea
						label={`${label} IDs, one per line`}
						value={bulk}
						disabled={disabled}
						onChange={(event) => setBulk(event.currentTarget.value)}
						autosize
						minRows={2}
						maxRows={5}
					/>
					<Button
						size="compact-xs"
						variant="light"
						disabled={disabled}
						onClick={() => {
							const ids = new Set(rows.map((row) => row.id.trim()));
							const added: CustomModelRow[] = [];
							for (const id of bulk
								.split("\n")
								.map((value) => value.trim())
								.filter(Boolean)) {
								if (!ids.has(id)) {
									ids.add(id);
									added.push({ id, name: "" });
								}
							}
							onChange([...rows, ...added]);
							setBulk(null);
						}}
					>
						Add IDs
					</Button>
				</>
			)}
			<Button
				size="compact-xs"
				variant="subtle"
				leftSection={<Plus size={14} />}
				disabled={disabled}
				onClick={() => onChange([...rows, { id: "", name: "" }])}
			>
				Add {label.toLowerCase()} model
			</Button>
		</Stack>
	);
}
