import {
	Button,
	Group,
	PasswordInput,
	Popover,
	Stack,
	Text,
} from "@mantine/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState } from "react";
import { tauriAPI } from "../../lib/tauri";

/** A replacement-only editor: saved secrets never need to enter the webview. */
export function ApiKeyField({
	storeKey,
	label,
	disabled = false,
}: {
	storeKey: string;
	label: string;
	disabled?: boolean;
}) {
	const queryClient = useQueryClient();
	const statusId = useId();
	const saved = useQuery({
		queryKey: ["apiKey", storeKey],
		queryFn: () => tauriAPI.hasApiKey(storeKey),
		retry: false,
	});
	const [draft, setDraft] = useState("");
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<{
		action: "save" | "remove";
		message: string;
	} | null>(null);
	const [confirmRemoval, setConfirmRemoval] = useState(false);
	const inFlight = useRef(false);
	const mounted = useRef(true);
	const field = useRef<HTMLDivElement>(null);
	useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
		};
	}, []);

	const commit = async (remove = false) => {
		const value = draft.trim();
		// Empty replacement drafts never remove an existing key. Removal is explicit.
		if (disabled || inFlight.current || (!remove && !value)) return;
		inFlight.current = true;
		setBusy(true);
		setError(null);
		try {
			await queryClient.cancelQueries({
				queryKey: ["apiKey", storeKey],
				exact: true,
			});
			if (remove) await tauriAPI.clearApiKey(storeKey);
			else await tauriAPI.setApiKey(storeKey, value);
			queryClient.setQueryData(["apiKey", storeKey], !remove);
			// Retire any old eager-secret query without fetching it again.
			queryClient.removeQueries({
				queryKey: ["apiKeyValue", storeKey],
				exact: true,
			});
			void queryClient.invalidateQueries({ queryKey: ["availableProviders"] });
			if (mounted.current) {
				setDraft("");
				setConfirmRemoval(false);
			}
		} catch {
			// Credential-store errors can include user input; never render or log them.
			if (mounted.current) {
				setError({
					action: remove ? "remove" : "save",
					message: remove
						? "Couldn’t remove the key. Try again."
						: "Couldn’t save. Your draft is still here.",
				});
			}
		} finally {
			inFlight.current = false;
			if (mounted.current) setBusy(false);
		}
	};

	const status =
		error?.message ??
		(saved.isError
			? "Can’t check the saved key."
			: busy
				? "Saving…"
				: draft.trim()
					? "Press Enter or leave the field to save"
					: saved.isPending
						? "Checking…"
						: saved.data
							? "Saved securely"
							: "No key saved");

	return (
		<Stack
			ref={field}
			gap={4}
			w={260}
			maw="100%"
			onBlur={(event) => {
				// Revealing a draft is not a request to save and erase it. Save
				// when focus leaves the entire editor, including its reveal button.
				if (
					event.relatedTarget instanceof Node &&
					field.current?.contains(event.relatedTarget)
				)
					return;
				void commit();
			}}
		>
			<PasswordInput
				aria-label={`${label} API key`}
				aria-describedby={statusId}
				value={draft}
				onChange={(event) => {
					setDraft(event.currentTarget.value);
					setError(null);
				}}
				onKeyDown={(event) => {
					if (event.key === "Enter" && !event.nativeEvent.isComposing) {
						event.preventDefault();
						void commit();
					}
				}}
				placeholder={
					saved.data ? "Paste a key to replace" : "Paste your API key"
				}
				disabled={disabled || busy || saved.isPending || saved.isError}
				autoComplete="off"
				data-private="true"
				size="sm"
			/>
			<Group justify="space-between" gap={6} align="flex-start" wrap="wrap">
				<Text
					id={statusId}
					size="xs"
					c={error || saved.isError ? "red" : "dimmed"}
					role={error ? "alert" : "status"}
					style={{ flex: 1 }}
				>
					{status}
				</Text>
				{saved.isError ? (
					<Button
						variant="subtle"
						size="compact-xs"
						disabled={disabled}
						onClick={() => {
							void saved.refetch();
						}}
					>
						Retry
					</Button>
				) : error && !confirmRemoval ? (
					<Button
						variant="subtle"
						size="compact-xs"
						disabled={disabled}
						onClick={() => {
							if (error.action === "remove") setConfirmRemoval(true);
							else void commit();
						}}
					>
						Retry
					</Button>
				) : saved.data ? (
					<Popover
						opened={!disabled && confirmRemoval}
						onChange={setConfirmRemoval}
						position="bottom-end"
						withinPortal
					>
						<Popover.Target>
							<Button
								variant="subtle"
								color="gray"
								size="compact-xs"
								disabled={disabled || busy || Boolean(draft.trim())}
								onClick={() => setConfirmRemoval((opened) => !opened)}
							>
								Remove
							</Button>
						</Popover.Target>
						<Popover.Dropdown>
							<Stack gap="xs">
								<Text size="sm">Remove the saved {label} key?</Text>
								<Group justify="flex-end" gap="xs">
									<Button
										size="compact-sm"
										variant="default"
										disabled={busy}
										onClick={() => setConfirmRemoval(false)}
									>
										Cancel
									</Button>
									<Button
										size="compact-sm"
										color="red"
										loading={busy}
										onClick={() => {
											void commit(true);
										}}
									>
										Remove key
									</Button>
								</Group>
							</Stack>
						</Popover.Dropdown>
					</Popover>
				) : null}
			</Group>
		</Stack>
	);
}
