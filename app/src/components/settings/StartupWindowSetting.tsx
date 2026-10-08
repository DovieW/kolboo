import { Switch, Tooltip } from "@mantine/core";
import {
	useSettings,
	useUpdateMainWindowShowOnLaunch,
} from "../../lib/queries";
import { SettingsRow } from "./SettingsRow";

export function StartupWindowSetting({
	profileScope,
}: {
	profileScope: boolean;
}) {
	const settings = useSettings();
	const update = useUpdateMainWindowShowOnLaunch();
	return (
		<SettingsRow
			label="Show window on launch"
			description={
				update.isError
					? "Couldn’t save startup preference. Try again."
					: "Open the main window when Kolboo launches, including at login"
			}
			right={
				<Tooltip
					label="Change this in the Default profile"
					disabled={!profileScope}
					withArrow
				>
					<span>
						<Switch
							aria-label="Show window on launch"
							checked={settings.data?.main_window_show_on_launch === true}
							disabled={
								profileScope ||
								settings.isPending ||
								settings.isError ||
								update.isPending
							}
							onChange={(event) => update.mutate(event.currentTarget.checked)}
						/>
					</span>
				</Tooltip>
			}
		/>
	);
}
