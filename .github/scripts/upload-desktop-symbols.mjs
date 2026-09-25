import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { appendFileSync, chmodSync, mkdtempSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

// Official getsentry/cli 0.45.0 release assets. Pin AND verify before execution.
const ASSETS = {
	"linux-x64": ["sentry-linux-x64", "658363afd0c6d581b887de41160c40adc7e29c79c015569a57aaf01532ed47be"],
	"darwin-arm64": ["sentry-darwin-arm64", "68a5f9a7fd24d5be67a40afbada46bce30f9855926f948e06b1e5d2408b94cc4"],
	"darwin-x64": ["sentry-darwin-x64", "a9e6532200d5ba17757625edd57b943dc240d78a2afc5d62b770d7bd69384950"],
	"win32-x64": ["sentry-windows-x64.exe", "e4193c136dfad3415e4251d77d7653f9317b2d37bbff7298baca1057baf37dfa"],
};

export function cliAsset(platform, arch) {
	const asset = ASSETS[`${platform}-${arch}`];
	if (!asset) throw new Error("Unsupported native-symbol upload runner");
	return { name: asset[0], sha256: asset[1], url: `https://github.com/getsentry/cli/releases/download/0.45.0/${asset[0]}` };
}

export function verifyCli(bytes, sha256) {
	if (createHash("sha256").update(bytes).digest("hex") !== sha256) throw new Error("Sentry CLI checksum mismatch; refusing to execute");
}

export function requireDebugInfo(reports) {
	if (!reports.some((report) => report.usable && report.objects?.some((object) => object.hasDebugInfo))) {
		throw new Error("Native artifact has no line/debug information; refusing an unreadable release");
	}
}

export async function uploadDesktopSymbols(paths, env = process.env) {
	if (!env.SENTRY_AUTH_TOKEN?.trim() || env.SENTRY_AUTH_TOKEN === "replace_me") {
		console.log("Native Sentry symbol upload disabled (no upload credential; development build only)");
		return;
	}
	if (!paths.length) throw new Error("Pass the exact desktop binary/debug files, not the target directory");
	for (const path of paths) {
		if (!statSync(path).isFile()) throw new Error("Native symbol input must be a file");
	}
	const asset = cliAsset(process.platform, process.arch);
	const response = await fetch(asset.url, { signal: AbortSignal.timeout(120_000) });
	if (!response.ok) throw new Error(`Sentry CLI download failed (${response.status})`);
	const bytes = Buffer.from(await response.arrayBuffer());
	verifyCli(bytes, asset.sha256);
	const binary = join(mkdtempSync(join(tmpdir(), "kolboo-sentry-cli-")), asset.name);
	writeFileSync(binary, bytes, { mode: 0o700 });
	chmodSync(binary, 0o700);
	const options = { env, timeout: 300_000, maxBuffer: 16 * 1024 * 1024 };
	const reports = paths.map((path) => JSON.parse(execFileSync(binary, ["debug-files", "check", path, "--json"], { ...options, encoding: "utf8" })));
	requireDebugInfo(reports);
	execFileSync(binary, ["debug-files", "upload", "--wait", "--no-sources", ...paths], { ...options, stdio: "inherit" });
	if (env.GITHUB_ENV) appendFileSync(env.GITHUB_ENV, "SENTRY_NATIVE_SYMBOLS_UPLOADED=true\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	await uploadDesktopSymbols(process.argv.slice(2));
}
