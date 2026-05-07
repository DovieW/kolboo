// Keep `queries.ts` as a shallow compatibility barrel while the real query
// modules live by domain under `app/src/lib/queries/**`.

export * from "./queries/costs";
export * from "./queries/history";
export * from "./queries/license";
export * from "./queries/logs";
export * from "./queries/policy";
export * from "./queries/providers";
export * from "./queries/recordings";
export * from "./queries/settings";
export {
	applySettingsQueryInvalidations,
	invalidateLicenseRelatedQueries,
	invalidateLogoutRelatedQueries,
	invalidatePolicyRelatedQueries,
	invalidateSettingsQueries,
	toManagedInferenceMessage,
} from "./queries/shared";
export * from "./queries/transcription";
