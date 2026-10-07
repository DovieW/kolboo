import { pathToFileURL } from "node:url";

export function validateDesktopCloudConfig(env) {
	for (const key of ["TAURI_CLOUDFLARE_ACCESS_CLIENT_ID", "TAURI_CLOUDFLARE_ACCESS_CLIENT_SECRET"]) {
		if (env[key]?.trim()) throw new Error(`Forbidden release setting: ${key}`);
	}
	if (env.TAURI_CLOUD_ENV === "community" && env.VITE_CLOUD_SERVICE_ENABLED === "false") {
		for (const key of ["TAURI_API_BASE_URL", "TAURI_MANAGED_INFERENCE_GATEWAY_URL", "TAURI_SUPABASE_URL", "TAURI_SUPABASE_PUBLISHABLE_KEY", "TAURI_PUBLIC_AUTH_PAGE_URL"]) {
			if (env[key]?.trim()) throw new Error(`Community package must omit public cloud configuration: ${key}`);
		}
		return;
	}
	if (env.TAURI_CLOUD_ENV !== "production" || env.VITE_CLOUD_SERVICE_ENABLED !== "true") {
		throw new Error("Explicit matching TAURI_CLOUD_ENV and VITE_CLOUD_SERVICE_ENABLED are required");
	}
	for (const key of [
		"TAURI_API_BASE_URL",
		"TAURI_MANAGED_INFERENCE_GATEWAY_URL",
		"TAURI_SUPABASE_URL",
		"TAURI_PUBLIC_AUTH_PAGE_URL",
	]) {
		try {
			const value = env[key];
			if (!value || /[\r\n]/.test(value)) throw new Error();
			const url = new URL(value);
			if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash ||
				url.hostname === "localhost" || url.hostname === "kolboo.dovie.dev" ||
				url.hostname.endsWith(".test") || /^dev[.-]/.test(url.hostname)) throw new Error();
		} catch {
			throw new Error(`Invalid production public configuration: ${key}`);
		}
	}
	const key = env.TAURI_SUPABASE_PUBLISHABLE_KEY ?? "";
	let anon = false;
	try {
		anon = JSON.parse(Buffer.from(key.split(".")[1] ?? "", "base64url").toString()).role === "anon";
	} catch { /* Non-JWT public keys use the publishable prefix. */ }
	if (/[\r\n]/.test(key) || !(key.startsWith("sb_publishable_") && key.length > 20 || anon)) {
		throw new Error("Only a Supabase publishable/anon key may enter desktop packages");
	}
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	validateDesktopCloudConfig(process.env);
	console.log("Desktop package cloud configuration validated; values not printed.");
}
