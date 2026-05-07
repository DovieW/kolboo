import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";

import { policyAPI, tauriAPI } from "../tauri";
import { listenTyped } from "../tauri/events";
import { applySettingsRuntimeSyncPolicy } from "../tauri/settingsSync";
import { invalidatePolicyRelatedQueries } from "./shared";

// Policy hooks are event-driven and share a very specific runtime-sync story,
// so we keep them together instead of mixing them with generic settings hooks.
export function usePolicyState() {
	const queryClient = useQueryClient();

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		listenTyped("policy-state-changed", () => {
			void invalidatePolicyRelatedQueries(queryClient);
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((error) => {
				console.warn("Failed to subscribe to policy-state-changed:", error);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);

	return useQuery({
		queryKey: ["policyState"],
		queryFn: () => tauriAPI.getPolicyState(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function createPolicySyncMutationFn(deps: {
	syncPolicy: (request?: { policyPack?: unknown }) => Promise<unknown>;
	invoke: (command: string) => Promise<unknown>;
	emitSettingsChanged?: () => Promise<unknown>;
}) {
	return async (request?: { policyPack?: unknown }) => {
		const state = await deps.syncPolicy(request);
		await applySettingsRuntimeSyncPolicy({
			policyNormalized: true,
			backendEventEmitted: true,
			invoke: deps.invoke,
			emitSettingsChanged: deps.emitSettingsChanged ?? (async () => undefined),
		});
		return state;
	};
}

export function usePolicySync() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: createPolicySyncMutationFn({
			syncPolicy: tauriAPI.syncPolicy,
			invoke,
		}),
		onSuccess: () => {
			void invalidatePolicyRelatedQueries(queryClient);
		},
	});
}

export function usePolicyDiagnosticsExport() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => policyAPI.exportPolicyDiagnostics(),
		onSuccess: () => {
			void invalidatePolicyRelatedQueries(queryClient);
		},
	});
}
