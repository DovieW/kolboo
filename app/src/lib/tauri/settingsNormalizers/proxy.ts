import { DEFAULT_PROXY_SETTINGS } from "../settingsDefaults";
import type { ProxySettings } from "../types";
import { isRecord } from "./shared";

export function normalizeProxyMode(value: unknown): ProxySettings["mode"] {
	if (value === "no_proxy" || value === "system" || value === "manual") {
		return value;
	}
	return DEFAULT_PROXY_SETTINGS.mode;
}

export function normalizeManualProxySettings(
	value: unknown,
): ProxySettings["manual"] {
	const v = isRecord(value) ? value : {};

	const proxy_url =
		typeof v.proxy_url === "string"
			? v.proxy_url
			: DEFAULT_PROXY_SETTINGS.manual.proxy_url;
	const no_proxy =
		typeof v.no_proxy === "string"
			? v.no_proxy
			: DEFAULT_PROXY_SETTINGS.manual.no_proxy;
	const username =
		typeof v.username === "string"
			? v.username
			: DEFAULT_PROXY_SETTINGS.manual.username;
	const password =
		typeof v.password === "string"
			? v.password
			: DEFAULT_PROXY_SETTINGS.manual.password;

	return { proxy_url, no_proxy, username, password };
}

function normalizeTrustedCaCertFormat(value: unknown): "pem" | "der" {
	return value === "der" ? "der" : "pem";
}

function normalizeTrustedCaCertificate(
	value: unknown,
): ProxySettings["trusted_ca_certificates"][number] | null {
	if (!isRecord(value)) return null;
	const x = value;
	const id = typeof x.id === "string" ? x.id : "";
	const file_name = typeof x.file_name === "string" ? x.file_name : "";
	const format = normalizeTrustedCaCertFormat(x.format);
	const data_base64 = typeof x.data_base64 === "string" ? x.data_base64 : "";
	if (!id || !data_base64) return null;
	return { id, file_name, format, data_base64 };
}

export function normalizeProxySettings(value: unknown): ProxySettings {
	const v = isRecord(value) ? value : {};
	const mode = normalizeProxyMode(v.mode);
	const manual = normalizeManualProxySettings(v.manual);

	const trusted_ca_certificates: ProxySettings["trusted_ca_certificates"] =
		Array.isArray(v.trusted_ca_certificates)
			? (v.trusted_ca_certificates as unknown[])
					.map(normalizeTrustedCaCertificate)
					.filter(
						(c): c is ProxySettings["trusted_ca_certificates"][number] =>
							c !== null,
					)
			: [];

	const danger_accept_invalid_certs =
		typeof v.danger_accept_invalid_certs === "boolean"
			? v.danger_accept_invalid_certs
			: DEFAULT_PROXY_SETTINGS.danger_accept_invalid_certs;

	return {
		mode,
		manual,
		trusted_ca_certificates,
		danger_accept_invalid_certs,
	};
}
