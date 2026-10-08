import { Anchor, Button, Group, Stack, Text } from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UpdateStatus } from "../../lib/tauri/types.generated";
import { updatesAPI } from "../../lib/tauri/updates";
import { SettingsRow } from "./SettingsRow";

const phases = {
	idle: "No update is ready.",
	checking: "Checking for updates…",
	downloading: "Downloading and verifying update…",
	ready: "Update ready to install.",
	installing: "Installing update. Kolboo will close…",
	error: "Update check or download failed. Try again.",
} satisfies Record<UpdateStatus["phase"], string>;

export function UpdatesSetting() {
	const client = useQueryClient();
	const query = useQuery({
		queryKey: ["updates"],
		queryFn: updatesAPI.status,
		retry: false,
		refetchInterval: 1000,
	});
	const check = useMutation({
		mutationFn: updatesAPI.check,
		onSuccess: (status) => client.setQueryData(["updates"], status),
	});
	const install = useMutation({
		mutationFn: updatesAPI.install,
		onSuccess: () => client.invalidateQueries({ queryKey: ["updates"] }),
	});
	const status = query.data;
	const busy =
		check.isPending ||
		install.isPending ||
		status?.phase === "checking" ||
		status?.phase === "downloading" ||
		status?.phase === "installing";
	return (
		<SettingsRow
			label="Updates"
			description={status?.hint}
			right={
				<Stack gap="xs" align="flex-end">
					<Text size="sm" role="status">
						{query.isError
							? "Couldn’t read update status. Try checking again."
							: !status
								? "Loading update status…"
								: !status.enabled
									? "This build uses manual downloads."
									: status.phase === "idle" && status.checked
										? "You’re up to date."
										: phases[status.phase]}
						{status?.version && ` Version ${status.version}.`}
					</Text>
					{(check.isError || install.isError || status?.error) && (
						<Text size="sm" c="red" role="alert">
							{install.isError
								? "Couldn’t install. Finish active work and check installation permissions, then try again."
								: "Couldn’t check or install the update. Please try again."}
						</Text>
					)}
					<Group gap="xs">
						<Button
							variant="default"
							disabled={busy || (!query.isError && !status?.enabled)}
							onClick={() => {
								install.reset();
								check.mutate();
							}}
						>
							Check for updates
						</Button>
						<Button
							disabled={busy || !status?.can_install}
							onClick={() => {
								check.reset();
								install.mutate();
							}}
						>
							Install update
						</Button>
					</Group>
					<Anchor
						href="https://github.com/DovieW/kolboo/releases/latest"
						target="_blank"
						rel="noopener noreferrer"
						size="sm"
					>
						Download latest release
					</Anchor>
				</Stack>
			}
		/>
	);
}
