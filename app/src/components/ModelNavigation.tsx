import { createContext, useContext } from "react";
import type { ModelSelection } from "../lib/tauri";

export type ModelProviderSetup = Pick<ModelSelection, "source" | "provider_id">;

export const ModelNavigationContext = createContext<{
	setup: (selection: ModelProviderSetup) => void;
	account: () => void;
} | null>(null);

export function useModelNavigation() {
	return useContext(ModelNavigationContext);
}
