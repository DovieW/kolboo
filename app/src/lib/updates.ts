// Minimal semver utilities + GitHub release version lookup.
// We intentionally keep this dependency-free.

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

export async function fetchLatestGithubReleaseVersion(params: {
	owner: string;
	repo: string;
}): Promise<string | null> {
	const url = `https://api.github.com/repos/${params.owner}/${params.repo}/releases/latest`;

	const response = await fetch(url, {
		headers: {
			Accept: "application/vnd.github+json",
		},
	});

	if (!response.ok) {
		// Fail closed (no update indicator) on any network / rate-limit issue.
		return null;
	}

	const json: unknown = await response.json();
	if (!json || typeof json !== "object") return null;

	const tagName = (json as { tag_name?: unknown }).tag_name;
	if (typeof tagName !== "string") return null;

	return normalizeVersion(tagName);
}
