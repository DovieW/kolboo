import { beforeEach, describe, expect, it, vi } from "vitest";

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
	beforeEach(() => {
		invokeMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		convertFileSrcMock.mockClear();
		startDraggingMock.mockClear();
		getCurrentWindowMock.mockClear();
		emitTypedMock.mockClear();
		listenTypedMock.mockClear();
	});

	it("typeText returns success false when invoke fails", async () => {
		invokeMock.mockRejectedValueOnce(new Error("boom"));
		const { tauriAPI } = await import("./commands");

		const result = await tauriAPI.typeText("hi");

		expect(result.success).toBe(false);
		expect(result.error).toContain("boom");
		expect(invokeMock).toHaveBeenCalledWith("type_text", { text: "hi" });
	});

	it("getCostSummary maps kind=all to undefined", async () => {
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

	it("getCostByProvider maps kind=all to undefined", async () => {
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
	});

	it("listOpenWindows includes includeTitles when requested", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.listOpenWindows({ includeTitles: true });
		await tauriAPI.listOpenWindows();

		expect(invokeMock).toHaveBeenNthCalledWith(1, "list_open_windows", {
			includeTitles: true,
		});
		expect(invokeMock).toHaveBeenNthCalledWith(2, "list_open_windows");
	});

	it("startDragging uses the current window API", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.startDragging();

		expect(getCurrentWindowMock).toHaveBeenCalled();
		expect(startDraggingMock).toHaveBeenCalled();
	});

	it("emitSettingsChanged forwards payload to emitTyped", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.emitSettingsChanged({ accent_color: "#ffffff" });

		expect(emitTypedMock).toHaveBeenCalledWith("settings-changed", {
			accent_color: "#ffffff",
		});
	});

	it("onSettingsChanged normalizes null payloads", async () => {
		const { tauriAPI } = await import("./commands");
		const handler = vi.fn();

		await tauriAPI.onSettingsChanged(handler);

		expect(listenTypedMock).toHaveBeenCalledWith(
			"settings-changed",
			expect.any(Function),
		);
		expect(handler).toHaveBeenCalledWith({});
	});

	it("cacheRouterEmbeddings defaults forceRefresh to null", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.cacheRouterEmbeddings({ profileId: "profile-1" });

		expect(invokeMock).toHaveBeenCalledWith("cache_router_embeddings", {
			profileId: "profile-1",
			forceRefresh: null,
		});
	});

	it("llmAPI.complete maps optional args and nulls", async () => {
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
	});

	it("sttAPI.testTranscribeLastAudio passes profileId mapping", async () => {
		const { sttAPI } = await import("./commands");

		await sttAPI.testTranscribeLastAudio({ profileId: null });

		expect(invokeMock).toHaveBeenCalledWith(
			"pipeline_test_transcribe_last_audio",
			{ profileId: null },
		);
	});

	it("getRecordingAssetUrl converts the file path", async () => {
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
	});

	it("getRecordingAssetUrl returns null when no path", async () => {
		invokeMock.mockResolvedValueOnce(null);
		const { recordingsAPI } = await import("./commands");

		const url = await recordingsAPI.getRecordingAssetUrl({
			requestId: "req-2",
		});

		expect(url).toBeNull();
		expect(convertFileSrcMock).not.toHaveBeenCalled();
	});

	it("configAPI.syncPipelineConfig invokes the backend", async () => {
		const { configAPI } = await import("./commands");

		await configAPI.syncPipelineConfig();

		expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
	});

	it("getPolicyState invokes policy command", async () => {
		const { tauriAPI } = await import("./commands");

		await tauriAPI.getPolicyState();

		expect(invokeMock).toHaveBeenCalledWith("policy_get_state");
	});

	it("syncPolicy invokes policy sync command", async () => {
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

	it("policyAPI.syncPolicy invokes policy sync command", async () => {
		const { policyAPI } = await import("./commands");

		await policyAPI.syncPolicy();

		expect(invokeMock).toHaveBeenCalledWith("policy_sync", {
			request: null,
		});
	});

	it("exportPolicyDiagnostics invokes policy export command", async () => {
		const { policyAPI } = await import("./commands");

		await policyAPI.exportPolicyDiagnostics();

		expect(invokeMock).toHaveBeenCalledWith("policy_export_diagnostics");
	});

	it("logsAPI.sentryBackendSmokeTest invokes backend smoke command", async () => {
		const { logsAPI } = await import("./commands");

		await logsAPI.sentryBackendSmokeTest("settings-panel");

		expect(invokeMock).toHaveBeenCalledWith("sentry_backend_smoke_test", {
			surface: "settings-panel",
		});
	});

	it("license wrappers invoke backend commands", async () => {
		const { tauriAPI, licenseAPI } = await import("./commands");
		const transitionHandler = vi.fn();

		await tauriAPI.getLicenseState();
		await tauriAPI.startLicenseLogin({
      provider_hint: "enterprise",
      auth_provider: "google",
      email: "user@example.com",
      password: "password123",
    });
		await tauriAPI.exchangeLicenseSession("upstream-token-123");
		await tauriAPI.logoutLicense();
		await tauriAPI.refreshLicenseEntitlement(true);
		await tauriAPI.getLicenseManagementUrl();

		await licenseAPI.getState();
		await licenseAPI.startLogin();
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
