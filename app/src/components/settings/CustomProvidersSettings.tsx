import {
	ActionIcon,
	Button,
	Group,
	Loader,
	Modal,
	Stack,
	Text,
	Textarea,
	TextInput,
} from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { useRef, useState } from "react";
import { type CustomProvider, configAPI } from "../../lib/tauri/commands";
import { ApiKeyField } from "./ApiKeyField";

const models = (value: string) => [
	...new Set(
		value
			.split("\n")
			.map((v) => v.trim())
			.filter(Boolean),
	),
];
export function isLocalProviderHost(host: string): boolean {
	const name = host.toLowerCase().replace(/^\[|\]$/g, "");
	if (name === "localhost" || name === "::1") return true;
	if (/^(fc|fd)[0-9a-f]{2}:/.test(name) || /^fe[89ab][0-9a-f]:/.test(name))
		return true;
	const parts = name.split(".").map(Number);
	return (
		parts.length === 4 &&
		parts.every((n) => Number.isInteger(n) && n >= 0 && n <= 255) &&
		(parts[0] === 127 ||
			parts[0] === 10 ||
			(parts[0] === 192 && parts[1] === 168) ||
			(parts[0] === 169 && parts[1] === 254) ||
			(parts[0] === 172 && (parts[1] ?? 0) >= 16 && (parts[1] ?? 0) <= 31))
	);
}
export function CustomProvidersSettings() {
	const client = useQueryClient();
	const providers = useQuery({
		queryKey: ["customProviders"],
		queryFn: configAPI.getCustomProviders,
		retry: false,
	});
	const [draft, setDraft] = useState<CustomProvider | null>(null);
	const [llm, setLlm] = useState("");
	const [stt, setStt] = useState("");
	const [removing, setRemoving] = useState<CustomProvider | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const inFlight = useRef(false);
	const edit = (provider: CustomProvider) => {
		setError(null);
		setDraft(provider);
		setLlm(provider.llm_models.join("\n"));
		setStt(provider.stt_models.join("\n"));
	};
	const save = async () => {
		if (!draft || inFlight.current) return;
		let url: URL;
		try {
			url = new URL(draft.base_url);
		} catch {
			setError("Enter a valid HTTP or HTTPS base URL.");
			return;
		}
		if (
			!["http:", "https:"].includes(url.protocol) ||
			url.username ||
			url.password ||
			url.search ||
			url.hash
		) {
			setError(
				"Use an HTTP or HTTPS URL without credentials, query or fragment.",
			);
			return;
		}
		if (url.protocol === "http:" && !isLocalProviderHost(url.hostname)) {
			setError(
				"Use HTTPS for remote providers. HTTP is only allowed on your local network.",
			);
			return;
		}
		if (!draft.name.trim() || (!models(llm).length && !models(stt).length)) {
			setError("Add a name and at least one model.");
			return;
		}
		if (
			draft.name.trim().length > 100 ||
			draft.base_url.trim().length > 2048 ||
			[models(llm), models(stt)].some(
				(list) => list.length > 100 || list.some((id) => id.length > 200),
			)
		) {
			setError(
				"Use up to 100 models per type, with model IDs under 201 characters.",
			);
			return;
		}
		inFlight.current = true;
		setBusy(true);
		setError(null);
		try {
			await configAPI.saveCustomProvider({
				...draft,
				name: draft.name.trim(),
				base_url: draft.base_url.trim().replace(/\/+$/, ""),
				llm_models: models(llm),
				stt_models: models(stt),
			});
			setDraft(null);
			for (const key of [
				"customProviders",
				"availableProviders",
				"llmProviders",
				"settings",
			])
				void client.invalidateQueries({ queryKey: [key] });
		} catch {
			setError("Could not save this provider. Your changes are still here.");
		} finally {
			inFlight.current = false;
			setBusy(false);
		}
	};
	const remove = async () => {
		if (!removing || inFlight.current) return;
		inFlight.current = true;
		setBusy(true);
		setError(null);
		try {
			await configAPI.deleteCustomProvider(removing.id);
			setRemoving(null);
			for (const key of [
				"customProviders",
				"availableProviders",
				"llmProviders",
				"settings",
			])
				void client.invalidateQueries({ queryKey: [key] });
		} catch {
			setError("Could not remove this provider. Try again.");
		} finally {
			inFlight.current = false;
			setBusy(false);
		}
	};
	return (
		<Stack gap="md" py="md">
			<Group justify="space-between">
				<Text fw={600}>Custom providers</Text>
				<Button
					variant="subtle"
					size="xs"
					leftSection={<Plus size={15} />}
					disabled={providers.isPending || providers.isError}
					onClick={() =>
						edit({
							id: `custom_${crypto.randomUUID().replaceAll("-", "_")}`,
							name: "",
							base_url: "",
							llm_models: [],
							stt_models: [],
						})
					}
				>
					Add provider
				</Button>
			</Group>
			{providers.isPending && <Loader size="xs" />}
			{providers.isError && (
				<Group>
					<Text size="sm" c="red">
						Could not load custom providers.
					</Text>
					<Button
						size="xs"
						variant="subtle"
						onClick={() => void providers.refetch()}
					>
						Retry
					</Button>
				</Group>
			)}
			{providers.data?.map((provider) => (
				<Stack key={provider.id} gap="xs">
					<Group justify="space-between">
						<Text size="sm" fw={500}>
							{provider.name}
						</Text>
						<Group gap={4}>
							<ActionIcon
								aria-label={`Edit ${provider.name}`}
								variant="subtle"
								onClick={() => edit(provider)}
							>
								<Pencil size={16} />
							</ActionIcon>
							<ActionIcon
								aria-label={`Remove ${provider.name}`}
								variant="subtle"
								color="gray"
								onClick={() => {
									setError(null);
									setRemoving(provider);
								}}
							>
								<Trash2 size={16} />
							</ActionIcon>
						</Group>
					</Group>
					<ApiKeyField
						storeKey={`${provider.id}_api_key`}
						label={`${provider.name} API key`}
					/>
				</Stack>
			))}
			<Modal
				opened={!!draft}
				onClose={() => {
					if (!inFlight.current) setDraft(null);
				}}
				title={
					providers.data?.some((p) => p.id === draft?.id)
						? "Edit provider"
						: "Add provider"
				}
				centered
				closeOnEscape={!busy}
				closeOnClickOutside={!busy}
				withCloseButton={!busy}
			>
				{draft && (
					<Stack gap="sm">
						<TextInput
							label="Name"
							maxLength={100}
							value={draft.name}
							disabled={busy}
							onChange={(e) =>
								setDraft({ ...draft, name: e.currentTarget.value })
							}
						/>
						<TextInput
							label="Base URL"
							maxLength={2048}
							placeholder="https://api.example.com/v1"
							value={draft.base_url}
							disabled={busy}
							onChange={(e) =>
								setDraft({ ...draft, base_url: e.currentTarget.value })
							}
						/>
						<Text size="xs" c="dimmed">
							OpenAI-compatible endpoint. Add model IDs, one per line.
						</Text>
						<Textarea
							label="Text models"
							value={llm}
							onChange={(e) => setLlm(e.currentTarget.value)}
							disabled={busy}
							autosize
							minRows={2}
							maxRows={5}
						/>
						<Textarea
							label="Transcription models"
							value={stt}
							onChange={(e) => setStt(e.currentTarget.value)}
							disabled={busy}
							autosize
							minRows={2}
							maxRows={5}
						/>
						{error && (
							<Text c="red" size="sm" role="alert">
								{error}
							</Text>
						)}
						<Group justify="flex-end">
							<Button
								variant="subtle"
								disabled={busy}
								onClick={() => setDraft(null)}
							>
								Cancel
							</Button>
							<Button loading={busy} onClick={() => void save()}>
								Save provider
							</Button>
						</Group>
					</Stack>
				)}
			</Modal>
			<Modal
				opened={!!removing}
				onClose={() => {
					if (!inFlight.current) setRemoving(null);
				}}
				title="Remove provider?"
				centered
				closeOnEscape={!busy}
				closeOnClickOutside={!busy}
				withCloseButton={!busy}
			>
				<Stack>
					<Text size="sm">
						Remove {removing?.name}? Existing model selections will not be
						replaced automatically.
					</Text>
					{error && (
						<Text c="red" size="sm" role="alert">
							{error}
						</Text>
					)}
					<Group justify="flex-end">
						<Button
							variant="subtle"
							disabled={busy}
							onClick={() => setRemoving(null)}
						>
							Cancel
						</Button>
						<Button color="red" loading={busy} onClick={() => void remove()}>
							Remove provider
						</Button>
					</Group>
				</Stack>
			</Modal>
		</Stack>
	);
}
