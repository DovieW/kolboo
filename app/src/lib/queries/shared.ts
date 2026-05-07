import {
	type QueryClient,
	useMutation,
	useQueryClient,
} from "@tanstack/react-query";

import {
	configAPI,
	dataAPI,
	llmAPI,
	logsAPI,
	recordingsAPI,
	sttAPI,
	tauriAPI,
} from "../tauri";
import {
	authReasonCodeToMessage,
	normalizeAuthReasonCode,
} from "../tauri/license";
import {
	classifySettingsRuntimeEffects,
	type SettingsQueryInvalidation,
} from "../tauri/settingsSync";

// Shared query-layer utilities live here so domain hook modules can stay small
// without each re-declaring the same runtime-sync and invalidation policy.
export function toManagedInferenceMessage(error: unknown): string {
	if (!(error && typeof error === "object")) {
		return "Managed inference is temporarily unavailable right now. You can retry, or switch to BYOK providers in Settings.";
	}

	const reasonCode = normalizeAuthReasonCode(
		(error as { reason_code?: unknown }).reason_code,
	);
	const reasonCodeMessage = authReasonCodeToMessage(reasonCode);
	if (reasonCodeMessage) {
		if (reasonCode === "insufficient_tier") {
			return `${reasonCodeMessage} You can continue with BYOK providers in Settings.`;
		}
		return reasonCodeMessage;
	}

	const category = (error as { category?: string }).category;
	if (category === "unauthorized") {
		return "Your session expired. Please sign in again to continue.";
	}
	if (category === "ineligible") {
		return "Your account or org is not eligible for managed inference right now.";
	}
	if (category === "over_quota") {
		return "You've reached your managed usage limit. Please wait for reset or switch to BYOK.";
	}

	return "Managed inference is temporarily unavailable right now. You can retry, or switch to BYOK providers in Settings.";
}

// Keep one shared dependency bag for queryFn factories so hook modules and
// tests always talk to the same thin adapter boundary.
export const queryFnDeps = {
	tauriAPI,
	sttAPI,
	recordingsAPI,
	dataAPI,
	configAPI,
	llmAPI,
	logsAPI,
} as const;

export async function applySettingsQueryInvalidations(
	queryClient: Pick<QueryClient, "invalidateQueries">,
	invalidations: readonly SettingsQueryInvalidation[],
): Promise<void> {
	await Promise.all(
		invalidations.map((invalidation) =>
			queryClient.invalidateQueries({ queryKey: invalidation.queryKey }),
		),
	);
}

export async function invalidatePolicyRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await applySettingsQueryInvalidations(
		queryClient,
		classifySettingsRuntimeEffects({ policyNormalized: true })
			.queryInvalidations,
	);
}

export async function invalidateLicenseRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await applySettingsQueryInvalidations(
		queryClient,
		classifySettingsRuntimeEffects({
			patch: { license_state: true },
		}).queryInvalidations.filter(
			(invalidation) => invalidation.reason === "license",
		),
	);
}

export async function invalidateLogoutRelatedQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
): Promise<void> {
	await Promise.all([
		invalidateLicenseRelatedQueries(queryClient),
		invalidatePolicyRelatedQueries(queryClient),
	]);
}

function buildSettingsMutationInvalidations(
	extraInvalidations: readonly SettingsQueryInvalidation[] = [],
): readonly SettingsQueryInvalidation[] {
	return [
		{ queryKey: ["settings"], reason: "settings" },
		...extraInvalidations,
	];
}

export async function invalidateSettingsQueries(
	queryClient: Pick<QueryClient, "invalidateQueries">,
	extraInvalidations: readonly SettingsQueryInvalidation[] = [],
): Promise<void> {
	await applySettingsQueryInvalidations(
		queryClient,
		buildSettingsMutationInvalidations(extraInvalidations),
	);
}

// Keep the common "write setting -> invalidate settings-derived queries" path
// in one place so simple settings hooks stay boring on purpose.
export function useSettingsInvalidatingMutation<TVariables, TData = unknown>(
	mutationFn: (variables: TVariables) => Promise<TData>,
	options?: {
		extraInvalidations?: readonly SettingsQueryInvalidation[];
		onError?: (error: unknown) => void;
	},
) {
	const queryClient = useQueryClient();

	return useMutation({
		mutationFn,
		onSuccess: () => {
			void invalidateSettingsQueries(queryClient, options?.extraInvalidations);
		},
		onError: options?.onError,
	});
}
