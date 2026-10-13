import { invoke } from "@tauri-apps/api/core";
import type { UpdateStatus } from "./types.generated";

export const updatesAPI = {
	status: () => invoke<UpdateStatus>("get_update_status"),
	check: () => invoke<UpdateStatus>("check_for_updates"),
	install: () => invoke<void>("install_update"),
};
