import { ActionIcon, Group, Switch, Tooltip } from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";
import { autostartAPI } from "../../lib/tauri/autostart";
import { SettingsRow } from "./SettingsRow";

const QUERY_KEY = ["start-at-login"];

export function StartAtLoginSetting({
	profileScope,
}: {
	profileScope: boolean;
}) {
	const client = useQueryClient();
	const status = useQuery({
		queryKey: QUERY_KEY,
		queryFn: autostartAPI.getEnabled,
		refetchOnMount: "always",
		retry: false,
	});
	const update = useMutation({
		mutationFn: autostartAPI.setEnabled,
		onSuccess: (enabled) => client.setQueryData(QUERY_KEY, enabled),
		// Also reread after an error: the OS write may have succeeded even if
		// checking the result failed. Never display an optimistic saved state.
		onSettled: () => client.invalidateQueries({ queryKey: QUERY_KEY }),
	});
	const description = status.isError
		? "Couldn’t read startup setting"
		: update.isError
			? "Couldn’t update startup setting"
			: "Launch Kolboo when you sign in to this computer";

	return (
		<SettingsRow
			label="Start at login"
			description={description}
			right={
				<Group gap="xs">
					{status.isError && (
						<Tooltip label="Retry" withArrow>
							<ActionIcon
								variant="subtle"
								aria-label="Retry startup setting"
								disabled={status.isFetching}
								onClick={() => void status.refetch()}
							>
								<RotateCcw size={16} />
							</ActionIcon>
						</Tooltip>
					)}
					<Tooltip
						label="Change this in the Default profile"
						disabled={!profileScope}
						withArrow
					>
						<span>
							<Switch
								aria-label="Start at login"
								checked={status.data === true}
								disabled={
									profileScope ||
									status.isPending ||
									status.isError ||
									update.isPending
								}
								onChange={(event) => update.mutate(event.currentTarget.checked)}
							/>
						</span>
					</Tooltip>
				</Group>
			}
		/>
	);
}
