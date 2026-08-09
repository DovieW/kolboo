import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { licenseAPI, tauriAPI } from "../tauri";
import {
	invalidateLicenseRelatedQueries,
	invalidateLogoutRelatedQueries,
} from "./shared";

// License/auth hooks stay together because they share the same transition
// listener, query keys, and logout invalidation semantics.
export function createLicenseStateQueryFn(
	api: Pick<typeof tauriAPI, "getLicenseState"> = tauriAPI,
) {
	return () => api.getLicenseState();
}

export function createLicenseAuthContextQueryFn(
	api: Pick<typeof tauriAPI, "getLicenseAuthContext"> = tauriAPI,
) {
	return () => api.getLicenseAuthContext();
}

export function createRefreshLicenseEntitlementMutationFn(
	api: Pick<typeof licenseAPI, "refreshEntitlement"> = licenseAPI,
) {
	return (simulateFailure?: boolean) => api.refreshEntitlement(simulateFailure);
}

export function createSignUpLicenseMutationFn(
	api: Pick<typeof licenseAPI, "signUp"> = licenseAPI,
) {
	return (request: { email: string; password: string }) => api.signUp(request);
}

export function useLicenseState() {
	return useQuery({
		queryKey: ["licenseState"],
		queryFn: createLicenseStateQueryFn(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useLicenseAuthContext() {
	return useQuery({
		queryKey: ["licenseAuthContext"],
		queryFn: createLicenseAuthContextQueryFn(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function useStartLicenseLogin() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: (request?: {
			provider_hint?: string | null;
			auth_provider?: string | null;
			email?: string | null;
			password?: string | null;
		}) => licenseAPI.startLogin(request),
		onSuccess: () => {
			void invalidateLicenseRelatedQueries(queryClient);
		},
	});
}

export function useSignUpLicense() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: createSignUpLicenseMutationFn(),
		onSuccess: () => {
			void invalidateLicenseRelatedQueries(queryClient);
		},
	});
}

export function useRequestLicensePasswordReset() {
	return useMutation({
		mutationFn: (email: string) => licenseAPI.requestPasswordReset(email),
	});
}

export function useLogoutLicense() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: () => licenseAPI.logout(),
		onSuccess: () => {
			void invalidateLogoutRelatedQueries(queryClient);
		},
	});
}

export function useRefreshLicenseEntitlement() {
	const queryClient = useQueryClient();
	return useMutation({
		mutationFn: createRefreshLicenseEntitlementMutationFn(),
		onSuccess: () => {
			void invalidateLicenseRelatedQueries(queryClient);
		},
	});
}

export function useLicenseQueryBootstrap() {
	const queryClient = useQueryClient();

	useLicenseState();
	useLicenseAuthContext();

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		licenseAPI
			.onTransition(() => {
				void invalidateLicenseRelatedQueries(queryClient);
			})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((error) => {
				console.warn(
					"Failed to subscribe to license transition events:",
					error,
				);
			});

		return () => {
			try {
				unlisten?.();
			} catch {
				// ignore
			}
		};
	}, [queryClient]);
}
