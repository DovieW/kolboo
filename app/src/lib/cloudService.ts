// The Rust build and CI preflight enforce this flag against TAURI_CLOUD_ENV.
// Community packages contain no account-service endpoints; cached entitlements
// must not make unavailable signup, managed models or sync appear usable.
export function isCloudServiceAvailable(): boolean {
	return import.meta.env.VITE_CLOUD_SERVICE_ENABLED !== "false";
}
