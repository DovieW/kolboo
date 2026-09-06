import { beforeEach, describe, expect, vi } from "vitest";
import { itWithImportTimeout } from "../testTimeouts";

const invokeMock = vi.fn();
const convertFileSrcMock = vi.fn((path: string) => `converted:${path}`);
const startDraggingMock = vi.fn();
const getCurrentWindowMock = vi.fn(() => ({
	startDragging: startDraggingMock,
}));

const emitTypedMock = vi.fn();
const listenTypedMock = vi.fn(
	async (_name: string, handler: (payload: unknown) => void) => {
		handler(null);
		return () => {
			// no-op
		};
	},
);

vi.mock("@tauri-apps/api/core", () => ({
	invoke: invokeMock,
	convertFileSrc: convertFileSrcMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
	getCurrentWindow: getCurrentWindowMock,
}));

vi.mock("./events", () => ({
	emitTyped: emitTypedMock,
	listenTyped: listenTypedMock,
}));

describe("tauri command wrappers", () => {
	itWithImportTimeout(
		"home controls use the existing recording pipeline without text injection",
		async () => {
			const { recordingControlsAPI } = await import("./commands");
			await recordingControlsAPI.getState();
			await recordingControlsAPI.start();
			await recordingControlsAPI.stop();
			await recordingControlsAPI.cancel();
			await recordingControlsAPI.setPaused(true);
			await recordingControlsAPI.setPaused(false);
			await recordingControlsAPI.getPaused();
			expect(invokeMock.mock.calls).toEqual([
				["pipeline_get_state"],
				[
					"pipeline_start_recording",
					{ historyOnly: true, computerAudio: false },
				],
				["pipeline_stop_and_transcribe"],
				["pipeline_cancel"],
				["pipeline_set_recording_paused", { paused: true }],
				["pipeline_set_recording_paused", { paused: false }],
				["pipeline_get_recording_paused"],
			]);
		},
	);
	beforeEach(() => {
		invokeMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		convertFileSrcMock.mockClear();
		startDraggingMock.mockClear();
		getCurrentWindowMock.mockClear();
		emitTypedMock.mockClear();
		listenTypedMock.mockClear();
	});

	itWithImportTimeout(
		"audio device wrappers call the backend commands with typed params",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.listAudioInputDevicesV2();
			await tauriAPI.getDefaultAudioInputDeviceName();
			await tauriAPI.startMicTestMeter("mic:v1:abc:0");
			await tauriAPI.stopMicTestMeter();

			expect(invokeMock).toHaveBeenNthCalledWith(
				1,
				"list_audio_input_devices_v2",
			);
			expect(invokeMock).toHaveBeenNthCalledWith(
				2,
				"get_default_audio_input_device_name",
			);
			expect(invokeMock).toHaveBeenNthCalledWith(3, "mic_test_start_meter", {
				args: { inputDeviceId: "mic:v1:abc:0" },
			});
			expect(invokeMock).toHaveBeenNthCalledWith(4, "mic_test_stop_meter");
		},
	);

	itWithImportTimeout(
		"typeText returns success false when invoke fails",
		async () => {
			invokeMock.mockRejectedValueOnce(new Error("boom"));
			const { tauriAPI } = await import("./commands");

			const result = await tauriAPI.typeText("hi");

			expect(result.success).toBe(false);
			expect(result.error).toContain("boom");
			expect(invokeMock).toHaveBeenCalledWith("type_text", { text: "hi" });
		},
	);

	itWithImportTimeout("getCostSummary maps kind=all to undefined", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.getCostSummary({ timeframe: "7d", kind: "all" });

		expect(invokeMock).toHaveBeenCalledWith("get_cost_summary_v2", {
			params: {
				timeframe: "7d",
				kind: undefined,
				sttModelKeys: undefined,
				llmModelKeys: undefined,
				excludeFreeTier: undefined,
			},
		});
	});

	itWithImportTimeout(
		"normalizes missing secret values and optional microphone identifiers",
		async () => {
			invokeMock
				.mockResolvedValueOnce(undefined)
				.mockResolvedValueOnce(undefined);
			const { tauriAPI } = await import("./commands");

			await expect(tauriAPI.getApiKey("openai_api_key")).resolves.toBeNull();
			await tauriAPI.startMicTestMeter();

			expect(invokeMock).toHaveBeenNthCalledWith(1, "secrets_get_api_key", {
				storeKey: "openai_api_key",
			});
			expect(invokeMock).toHaveBeenNthCalledWith(2, "mic_test_start_meter", {
				args: { inputDeviceId: null },
			});
		},
	);

	itWithImportTimeout(
		"getCostByProvider maps kind=all to undefined",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.getCostByProvider({ timeframe: "24h", kind: "all" });

			expect(invokeMock).toHaveBeenCalledWith("get_cost_by_provider_v2", {
				params: {
					timeframe: "24h",
					kind: undefined,
					sttModelKeys: undefined,
					llmModelKeys: undefined,
					excludeFreeTier: undefined,
				},
			});
		},
	);

	itWithImportTimeout(
		"listOpenWindows includes includeTitles when requested",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.listOpenWindows({ includeTitles: true });
			await tauriAPI.listOpenWindows();

			expect(invokeMock).toHaveBeenNthCalledWith(1, "list_open_windows", {
				includeTitles: true,
			});
			expect(invokeMock).toHaveBeenNthCalledWith(2, "list_open_windows");
		},
	);

	itWithImportTimeout("startDragging uses the current window API", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.startDragging();

		expect(getCurrentWindowMock).toHaveBeenCalled();
		expect(startDraggingMock).toHaveBeenCalled();
	});

	itWithImportTimeout(
		"emitSettingsChanged forwards payload to emitTyped",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.emitSettingsChanged({ accent_color: "#ffffff" });

			expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
				accent_color: "#ffffff",
			});
		},
	);

	itWithImportTimeout(
		"setApiKey syncs runtime config and emits one settings-changed event",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.setApiKey("groq_api_key", "secret");

			expect(invokeMock).toHaveBeenCalledWith("secrets_set_api_key", {
				storeKey: "groq_api_key",
				apiKey: "secret",
			});
			expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
			expect(emitTypedMock).toHaveBeenCalledTimes(1);
			expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
				api_keys_changed: true,
			});
		},
	);

	itWithImportTimeout(
		"clearApiKey uses the same runtime sync policy as setApiKey",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.clearApiKey("ocr_api_key");

			expect(invokeMock).toHaveBeenCalledWith("secrets_clear_api_key", {
				storeKey: "ocr_api_key",
			});
			expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
			expect(emitTypedMock).toHaveBeenCalledTimes(1);
			expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
				api_keys_changed: true,
			});
		},
	);

	itWithImportTimeout(
		"onSettingsChanged normalizes null payloads",
		async () => {
			const { tauriAPI } = await import("./commands");
			const handler = vi.fn();

			await tauriAPI.onSettingsChanged(handler);

			expect(listenTypedMock).toHaveBeenCalledWith(
				"settings-changed",
				expect.any(Function),
			);
			expect(handler).toHaveBeenCalledWith({});
		},
	);

	itWithImportTimeout(
		"cacheRouterEmbeddings defaults forceRefresh to null",
		async () => {
			const { tauriAPI } = await import("./commands");

			await tauriAPI.cacheRouterEmbeddings({ profileId: "profile-1" });

			expect(invokeMock).toHaveBeenCalledWith("cache_router_embeddings", {
				profileId: "profile-1",
				forceRefresh: null,
			});
		},
	);

	itWithImportTimeout(
		"llmAPI.complete maps optional args and nulls",
		async () => {
			const { llmAPI } = await import("./commands");

			await llmAPI.complete({
				provider: "openai",
				model: null,
				systemPrompt: "sys",
				userPrompt: "user",
				openAiReasoningEffort: "low",
				geminiThinkingBudget: null,
				geminiThinkingLevel: null,
				anthropicThinkingBudget: null,
			});

			expect(invokeMock).toHaveBeenCalledWith("llm_complete", {
				args: {
					provider: "openai",
					model: null,
					openAiReasoningEffort: "low",
					geminiThinkingBudget: null,
					geminiThinkingLevel: null,
					anthropicThinkingBudget: null,
					systemPrompt: "sys",
					userPrompt: "user",
				},
			});
		},
	);

	itWithImportTimeout(
		"sttAPI.testTranscribeLastAudio passes profileId mapping",
		async () => {
			const { sttAPI } = await import("./commands");

			await sttAPI.testTranscribeLastAudio({ profileId: null });

			expect(invokeMock).toHaveBeenCalledWith(
				"pipeline_test_transcribe_last_audio",
				{ profileId: null },
			);
		},
	);

	itWithImportTimeout(
		"getRecordingAssetUrl converts the file path",
		async () => {
			invokeMock.mockResolvedValueOnce("C:\\temp\\audio.wav");
			const { recordingsAPI } = await import("./commands");

			const url = await recordingsAPI.getRecordingAssetUrl({
				requestId: "req-1",
			});

			expect(invokeMock).toHaveBeenCalledWith("recording_get_wav_path", {
				requestId: "req-1",
			});
			expect(convertFileSrcMock).toHaveBeenCalledWith("C:\\temp\\audio.wav");
			expect(url).toBe("converted:C:\\temp\\audio.wav");
		},
	);

	itWithImportTimeout(
		"getRecordingAssetUrl returns null when no path",
		async () => {
			invokeMock.mockResolvedValueOnce(null);
			const { recordingsAPI } = await import("./commands");

			const url = await recordingsAPI.getRecordingAssetUrl({
				requestId: "req-2",
			});

			expect(url).toBeNull();
			expect(convertFileSrcMock).not.toHaveBeenCalled();
		},
	);

	itWithImportTimeout(
		"configAPI.syncPipelineConfig invokes the backend",
		async () => {
			const { configAPI } = await import("./commands");

			await configAPI.syncPipelineConfig();

			expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
		},
	);

	itWithImportTimeout("getPolicyState invokes policy command", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.getPolicyState();

		expect(invokeMock).toHaveBeenCalledWith("policy_get_state");
	});

	itWithImportTimeout("syncPolicy invokes policy sync command", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.syncPolicy({
			policyPack: { version: 1, constraints: { rewrite_llm_enabled: true } },
		});

		expect(invokeMock).toHaveBeenCalledWith("policy_sync", {
			request: {
				policyPack: { version: 1, constraints: { rewrite_llm_enabled: true } },
			},
		});
	});

	itWithImportTimeout(
		"policyAPI.syncPolicy invokes policy sync command",
		async () => {
			const { policyAPI } = await import("./commands");

			await policyAPI.syncPolicy();

			expect(invokeMock).toHaveBeenCalledWith("policy_sync", {
				request: null,
			});
		},
	);

	itWithImportTimeout(
		"exportPolicyDiagnostics invokes policy export command",
		async () => {
			const { policyAPI } = await import("./commands");

			await policyAPI.exportPolicyDiagnostics();

			expect(invokeMock).toHaveBeenCalledWith("policy_export_diagnostics");
		},
	);

	itWithImportTimeout(
		"logsAPI.sentryBackendSmokeTest invokes backend smoke command",
		async () => {
			const { logsAPI } = await import("./commands");

			await logsAPI.sentryBackendSmokeTest("settings-panel");

			expect(invokeMock).toHaveBeenCalledWith("sentry_backend_smoke_test", {
				surface: "settings-panel",
			});
		},
	);

	itWithImportTimeout("license wrappers invoke backend commands", async () => {
		invokeMock.mockImplementation(async (command: string) => {
			if (command === "license_sign_up") {
				return {
					confirmation_required: false,
					email: "new@example.com",
					state: {
						tier: "community",
						status: "active",
						user_id: "user_123",
						email: "new@example.com",
						org: null,
						expires_at: null,
						cached_at: "2026-01-01T00:00:00Z",
						last_validated_at: "2026-01-01T00:00:00Z",
						usage: {
							stt_seconds_used: 0,
							llm_tokens_used: 0,
							requests_today: 0,
						},
						limits: {
							stt_seconds_monthly: 0,
							llm_tokens_monthly: 0,
							requests_per_day: 0,
						},
						portal_available: false,
					},
				};
			}

			return undefined;
		});

		const { tauriAPI, licenseAPI } = await import("./commands");
		const transitionHandler = vi.fn();

		await tauriAPI.getLicenseState();
		await tauriAPI.startLicenseLogin({
			provider_hint: "enterprise",
			auth_provider: "google",
			email: "user@example.com",
			password: "password123",
		});
		await tauriAPI.signUpLicense({
			email: "new@example.com",
			password: "password123",
		});
		await tauriAPI.requestLicensePasswordReset("user@example.com");
		await tauriAPI.exchangeLicenseSession("upstream-token-123");
		await tauriAPI.logoutLicense();
		await tauriAPI.refreshLicenseEntitlement(true);
		await tauriAPI.getLicenseManagementUrl();

		await licenseAPI.getState();
		await licenseAPI.startLogin();
		await licenseAPI.signUp({
			email: "newer@example.com",
			password: "password456",
		});
		await licenseAPI.requestPasswordReset("newer@example.com");
		await licenseAPI.exchangeSession("upstream-token-456");
		await licenseAPI.onTransition(transitionHandler);

		expect(invokeMock).toHaveBeenCalledWith("license_get_state");
		expect(invokeMock).toHaveBeenCalledWith("license_start_login", {
			request: {
				provider_hint: "enterprise",
				auth_provider: "google",
				email: "user@example.com",
				password: "password123",
			},
		});
		expect(invokeMock).toHaveBeenCalledWith("license_sign_up", {
			request: {
				email: "new@example.com",
				password: "password123",
			},
		});
		expect(invokeMock).toHaveBeenCalledWith("license_request_password_reset", {
			email: "user@example.com",
		});
		expect(invokeMock).toHaveBeenCalledWith("license_logout");
		expect(invokeMock).toHaveBeenCalledWith("license_exchange_session", {
			request: {
				upstream_access_token: "upstream-token-123",
			},
		});
		expect(invokeMock).toHaveBeenCalledWith("license_refresh_entitlement", {
			simulateFailure: true,
		});
		expect(invokeMock).toHaveBeenCalledWith("license_get_management_url");
		expect(invokeMock).toHaveBeenCalledWith("license_start_login", {
			request: {
				provider_hint: null,
				auth_provider: null,
				email: null,
				password: null,
			},
		});
		expect(invokeMock).toHaveBeenCalledWith("license_sign_up", {
			request: {
				email: "newer@example.com",
				password: "password456",
			},
		});
		expect(invokeMock).toHaveBeenCalledWith("license_request_password_reset", {
			email: "newer@example.com",
		});
		expect(invokeMock).toHaveBeenCalledWith("license_exchange_session", {
			request: {
				upstream_access_token: "upstream-token-456",
			},
		});
		expect(listenTypedMock).toHaveBeenCalledWith(
			"settings-changed",
			expect.any(Function),
		);
		expect(transitionHandler).not.toHaveBeenCalled();
	});
});
