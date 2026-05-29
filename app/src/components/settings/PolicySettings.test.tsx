import { describe, expect, it } from "vitest";
import {
	buildSupportDiagnosticsBundle,
	diagnosticsToJson,
	formatPolicySourceLabel,
	formatPolicyTimestampLabel,
	policyStatusSummary,
} from "./policyDiagnostics";

describe("PolicySettings helpers", () => {
	it("renders policy source labels", () => {
		expect(formatPolicySourceLabel("none")).toBe("Unmanaged");
		expect(formatPolicySourceLabel("file")).toBe("Local file");
		expect(formatPolicySourceLabel("cloud")).toBe("Cloud");
	});

	it("formats timestamps safely", () => {
		expect(formatPolicyTimestampLabel(null)).toBe("—");
		expect(formatPolicyTimestampLabel("not-a-date")).toBe("—");
		expect(formatPolicyTimestampLabel("2026-02-13T12:00:00Z")).toContain(
			"2026",
		);
	});

	it("summarizes status from policy state", () => {
		expect(
			policyStatusSummary({
				source: "none",
				is_valid: true,
				last_updated: null,
				expires_at: null,
				version: null,
				enforced_fields: [],
			}),
		).toBe("No active policy");

		expect(
			policyStatusSummary({
				source: "cloud",
				is_valid: false,
				last_updated: null,
				expires_at: null,
				version: null,
				enforced_fields: [],
			}),
		).toBe("Policy invalid");
	});

	it("serializes diagnostics JSON", () => {
		const json = diagnosticsToJson({ redaction_applied: true });
		expect(json).toContain('"redaction_applied": true');
	});

	it("builds a support-safe diagnostics bundle with hashed targets and request ids", async () => {
		const bundle = await buildSupportDiagnosticsBundle(
			{
				policyExport: {
					generated_at: "2026-05-14T12:00:00.000Z",
					policy_state: {
						source: "cloud",
						is_valid: true,
						last_updated: null,
						expires_at: null,
						version: "3",
						enforced_fields: [],
					},
					enforced_fields: [],
					redaction_applied: true,
				},
				runtimeConfig: {
					app_version: "0.2.4-test",
					api_base_url: null,
					managed_inference_gateway_url: null,
					cloudflare_access_client_id: null,
					cloudflare_access_client_secret: null,
					sentry_dsn: null,
					sentry_env: "preview",
					sentry_release: "kolboo-desktop@0.2.4-test",
					posthog_api_key: null,
					posthog_host: null,
				},
				licenseState: {
					tier: "enterprise",
					status: "active",
					user_id: "user-123",
					email: "sensitive@example.com",
					org: {
						org_id: "org-123",
						org_name: "Acme Enterprise",
						inference_mode: "managed",
					},
					expires_at: null,
					cached_at: "2026-05-14T11:50:00.000Z",
					last_validated_at: "2026-05-14T11:45:00.000Z",
					usage: {
						stt_seconds_used: 11,
						llm_tokens_used: 22,
						requests_today: 3,
					},
					limits: {
						stt_seconds_monthly: 100,
						llm_tokens_monthly: 200,
						requests_per_day: 10,
					},
					portal_available: false,
				},
				authContext: {
					authenticated: true,
					secure_session_present: true,
					subject_id: "user-123",
					issuer: "https://issuer.test",
					mode: "enterprise",
					org_id: "org-123",
					entitlements: ["enterprise"],
					policy_status: "allow",
					reason_code: null,
				},
				requestLogs: [
					{
						id: "req-1",
						started_at: "2026-05-14T11:55:00.000Z",
						ended_at: "2026-05-14T11:55:05.000Z",
						stt_provider: "deepgram",
						stt_model: null,
						llm_provider: null,
						llm_model: null,
						raw_transcript: "this should never leave the device",
						final_text: "also not for export",
						total_duration_ms: 5000,
						stt_duration_ms: null,
						llm_duration_ms: null,
						status: "error",
						error_message: "provider failed",
						entries: [],
						stt_is_free_tier: false,
						llm_is_free_tier: false,
						stt_estimated_cost_usd_micros: null,
						llm_estimated_cost_usd_micros: null,
					},
				],
			},
			async (targetScope) => {
				if (targetScope.startsWith("user:")) {
					return "a".repeat(64);
				}
				return "b".repeat(64);
			},
		);

		expect(bundle.operator_handoff.request_ids).toEqual(["req-1"]);
		expect(bundle.operator_handoff.user_target).toEqual({
			target_scope: "user:aaaaaaaaaaaa",
			target_hash: "a".repeat(64),
		});
		expect(bundle.operator_handoff.org_target).toEqual({
			target_scope: "org:bbbbbbbbbbbb",
			target_hash: "b".repeat(64),
		});
		expect(bundle.recent_request_logs[0]).toMatchObject({
			request_id: "req-1",
			status: "error",
			error_present: true,
		});

		const serialized = JSON.stringify(bundle);
		expect(serialized).not.toContain("sensitive@example.com");
		expect(serialized).not.toContain("Acme Enterprise");
		expect(serialized).not.toContain("user-123");
		expect(serialized).not.toContain("org-123");
		expect(serialized).not.toContain("this should never leave the device");
		expect(serialized).not.toContain("also not for export");
	});
});
