import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { listenTyped } from "../tauri/events";
import type {
	EnterprisePersonaEnvironment,
	EnterprisePersonaState,
	EnterprisePersonaType,
	SettingsChangedPayload,
} from "../tauri/types";

const STORAGE_CONTEXT_KEY = "kolboo_fixture_context";
const STORAGE_PERSONA_KEY = "kolboo_test_persona";
const STORAGE_TEST_ACCESS_EXPIRES_AT_KEY = "kolboo_test_access_expires_at";

function normalizePersonaType(value: unknown): EnterprisePersonaType | null {
	if (value === "byok" || value === "managed" || value === "mixed-policy") {
		return value;
	}
	return null;
}

function normalizeIsoTimestamp(value: unknown): string | null {
	if (typeof value !== "string") return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	const parsed = Date.parse(trimmed);
	if (Number.isNaN(parsed)) return null;
	return new Date(parsed).toISOString();
}

function inferEnvironmentFromContext(
	contextKey: string | null,
): EnterprisePersonaEnvironment {
	const normalized = contextKey?.trim().toLowerCase() ?? "";
	if (normalized.startsWith("pr-")) return "preview";
	if (normalized.includes("staging")) return "staging";
	if (import.meta.env.PROD) return "production";
	return "local";
}

function getLocalStorageValue(key: string): string | null {
	try {
		if (typeof window === "undefined") return null;
		return window.localStorage.getItem(key);
	} catch {
		return null;
	}
}

function setLocalStorageValue(key: string, value: string | null): void {
	try {
		if (typeof window === "undefined") return;
		if (value == null || value.trim().length === 0) {
			window.localStorage.removeItem(key);
			return;
		}
		window.localStorage.setItem(key, value);
	} catch {
		// no-op
	}
}

function readEnterprisePersonaStateFromStorage(): EnterprisePersonaState {
	const contextKey = getLocalStorageValue(STORAGE_CONTEXT_KEY)?.trim() || null;
	const personaType = normalizePersonaType(
		getLocalStorageValue(STORAGE_PERSONA_KEY),
	);
	const testAccessExpiresAt = normalizeIsoTimestamp(
		getLocalStorageValue(STORAGE_TEST_ACCESS_EXPIRES_AT_KEY),
	);
	const testAccessActive =
		testAccessExpiresAt != null && Date.parse(testAccessExpiresAt) > Date.now();

	return {
		context_key: contextKey,
		persona_type: personaType,
		test_access_active: testAccessActive,
		test_access_expires_at: testAccessExpiresAt,
		environment: inferEnvironmentFromContext(contextKey),
		source:
			contextKey || personaType || testAccessExpiresAt ? "storage" : "none",
		updated_at: testAccessExpiresAt,
	};
}

function extractString(
	payload: SettingsChangedPayload,
	keys: string[],
): string | null {
	for (const key of keys) {
		const raw = payload[key];
		if (typeof raw === "string" && raw.trim().length > 0) {
			return raw.trim();
		}
	}
	return null;
}

function extractBoolean(
	payload: SettingsChangedPayload,
	keys: string[],
): boolean | null {
	for (const key of keys) {
		const raw = payload[key];
		if (typeof raw === "boolean") return raw;
	}
	return null;
}

export function applyPersonaEventPayload(
	current: EnterprisePersonaState,
	payload: SettingsChangedPayload,
): EnterprisePersonaState {
	const contextKey =
		extractString(payload, [
			"enterprise_test_persona_context_key",
			"test_persona_context_key",
			"persona_context_key",
		]) ?? current.context_key;

	const personaType =
		normalizePersonaType(
			extractString(payload, [
				"enterprise_test_persona_type",
				"test_persona_type",
				"persona_type",
			]),
		) ?? current.persona_type;

	const expiresAt =
		normalizeIsoTimestamp(
			extractString(payload, [
				"enterprise_test_access_expires_at",
				"test_access_expires_at",
			]),
		) ?? current.test_access_expires_at;

	const explicitActive = extractBoolean(payload, [
		"enterprise_test_access_active",
		"test_access_active",
	]);
	const computedActive =
		expiresAt != null && Date.parse(expiresAt) > Date.now();

	const next: EnterprisePersonaState = {
		context_key: contextKey,
		persona_type: personaType,
		test_access_active: explicitActive ?? computedActive,
		test_access_expires_at: expiresAt,
		environment: inferEnvironmentFromContext(contextKey),
		source:
			contextKey || personaType || expiresAt || explicitActive != null
				? "event"
				: current.source,
		updated_at: new Date().toISOString(),
	};

	setLocalStorageValue(STORAGE_CONTEXT_KEY, next.context_key);
	setLocalStorageValue(STORAGE_PERSONA_KEY, next.persona_type);
	setLocalStorageValue(
		STORAGE_TEST_ACCESS_EXPIRES_AT_KEY,
		next.test_access_expires_at,
	);

	return next;
}

export function useEnterprisePersonaState() {
	const queryClient = useQueryClient();

	useEffect(() => {
		let unlisten: (() => void) | null = null;

		listenTyped("settings-changed", (payload) => {
			queryClient.setQueryData<EnterprisePersonaState>(
				["enterprisePersona"],
				(previous) => {
					const base = previous ?? readEnterprisePersonaStateFromStorage();
					return applyPersonaEventPayload(base, payload);
				},
			);
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch((error) => {
				console.warn(
					"Failed to subscribe to settings-changed for persona state:",
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

	return useQuery({
		queryKey: ["enterprisePersona"],
		queryFn: async () => readEnterprisePersonaStateFromStorage(),
		staleTime: 0,
		refetchOnWindowFocus: true,
	});
}

export function formatEnterprisePersonaLabel(
	personaType: EnterprisePersonaType | null,
): string {
	if (personaType === "byok") return "BYOK";
	if (personaType === "managed") return "Managed";
	if (personaType === "mixed-policy") return "Mixed policy";
	return "Not set";
}
