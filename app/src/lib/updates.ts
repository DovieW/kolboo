// Minimal semver utilities plus the opt-in signed Tauri updater path.

export const signedUpdaterEnabled =
	import.meta.env.VITE_SIGNED_UPDATER_ENABLED === "true";

export async function checkSignedUpdateVersion(): Promise<string | null> {
	if (!signedUpdaterEnabled) return null;

	const { check } = await import("@tauri-apps/plugin-updater");
	const update = await check();
	return update?.version ?? null;
}

export async function installSignedUpdate(): Promise<boolean> {
	if (!signedUpdaterEnabled) return false;

	const { check } = await import("@tauri-apps/plugin-updater");
	const update = await check();
	if (!update) return false;

	await update.downloadAndInstall();
	return true;
}

export function normalizeVersion(input: string): string | null {
	const trimmed = input.trim();
	if (!trimmed) return null;

	// Accept either "v1.2.3" or "1.2.3"; strip leading v/V.
	const withoutV = trimmed.replace(/^[vV]/, "");

	// Keep only the core semver part (major.minor.patch) and ignore pre-release/build.
	// Examples accepted:
	// - 1.2.3
	// - 1.2.3-beta.1 -> 1.2.3
	// - 1.2.3+build.7 -> 1.2.3
	const match = withoutV.match(/^(\d+)\.(\d+)\.(\d+)/);
	if (!match) return null;

	return `${Number(match[1])}.${Number(match[2])}.${Number(match[3])}`;
}

export function compareSemver(a: string, b: string): number {
	const parse = (value: string): [number, number, number] | null => {
		const normalized = normalizeVersion(value);
		if (!normalized) return null;

		const parts = normalized.split(".");
		if (parts.length !== 3) return null;

		const major = Number(parts[0]);
		const minor = Number(parts[1]);
		const patch = Number(parts[2]);

		if (
			!Number.isFinite(major) ||
			!Number.isFinite(minor) ||
			!Number.isFinite(patch)
		) {
			return null;
		}

		return [major, minor, patch];
	};

	const pa = parse(a);
	const pb = parse(b);

	// "Unknown" versions are treated as equal so we fail closed.
	if (!pa || !pb) return 0;

	const [aMaj, aMin, aPatch] = pa;
	const [bMaj, bMin, bPatch] = pb;

	if (aMaj !== bMaj) return aMaj > bMaj ? 1 : -1;
	if (aMin !== bMin) return aMin > bMin ? 1 : -1;
	if (aPatch !== bPatch) return aPatch > bPatch ? 1 : -1;
	return 0;
}
