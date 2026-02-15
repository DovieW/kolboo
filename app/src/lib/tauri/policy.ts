import { invoke } from "@tauri-apps/api/core";
import type { PolicyDiagnosticExport, PolicyState } from "./types";

export const tauriPolicyAPI = {
	syncPolicy: (request?: { policyPack?: unknown }): Promise<PolicyState> =>
		invoke("policy_sync", {
			request: request
				? {
						policyPack: request.policyPack ?? null,
					}
				: null,
		}),

	getPolicyState: (): Promise<PolicyState> => invoke("policy_get_state"),

	exportPolicyDiagnostics: (): Promise<PolicyDiagnosticExport> =>
		invoke("policy_export_diagnostics"),
};
