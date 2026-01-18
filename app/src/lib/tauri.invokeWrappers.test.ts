import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { configAPI, llmAPI, sttAPI, tauriAPI } from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (x: string) => x,
  invoke: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => undefined),
  listen: vi.fn(async () => () => undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    startDragging: vi.fn(async () => undefined),
  })),
}));

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => ({
      get: vi.fn(async () => undefined),
      set: vi.fn(async () => undefined),
      delete: vi.fn(async () => undefined),
      save: vi.fn(async () => undefined),
    })),
  },
}));

const invokeMock = vi.mocked(invoke);
const emitMock = vi.mocked(emit);

describe("tauri invoke wrappers", () => {
  beforeEach(() => {
    invokeMock.mockClear();
    emitMock.mockClear();
  });

  it("tauriAPI.getCostSummary uses get_cost_summary_v2", async () => {
    await tauriAPI.getCostSummary({
      timeframe: "24h",
      kind: "all",
    });

    expect(invokeMock).toHaveBeenCalledWith("get_cost_summary_v2", {
      params: {
        timeframe: "24h",
        kind: undefined,
        sttModelKeys: undefined,
        llmModelKeys: undefined,
        excludeFreeTier: undefined,
      },
    });
  });

  it("configAPI.syncPipelineConfig uses sync_pipeline_config", async () => {
    await configAPI.syncPipelineConfig();
    expect(invokeMock).toHaveBeenCalledWith("sync_pipeline_config");
  });

  it("llmAPI.complete uses llm_complete with args payload", async () => {
    await llmAPI.complete({
      provider: "openai",
      systemPrompt: "sys",
      userPrompt: "user",
    });

    expect(invokeMock).toHaveBeenCalledWith("llm_complete", {
      args: {
        provider: "openai",
        model: null,
        openAiReasoningEffort: null,
        geminiThinkingBudget: null,
        geminiThinkingLevel: null,
        anthropicThinkingBudget: null,
        systemPrompt: "sys",
        userPrompt: "user",
      },
    });
  });

  it("sttAPI.retryTranscription uses pipeline_retry_transcription", async () => {
    await sttAPI.retryTranscription({ requestId: "req_123" });

    expect(invokeMock).toHaveBeenCalledWith("pipeline_retry_transcription", {
      requestId: "req_123",
    });
  });

  it("tauriAPI.updateOverlayMode invokes set_overlay_mode and emits settings-changed", async () => {
    await tauriAPI.updateOverlayMode("always");

    expect(invokeMock).toHaveBeenCalledWith("set_overlay_mode", {
      mode: "always",
    });
    expect(emitMock).toHaveBeenCalledWith("settings-changed", {});
  });

  it("configAPI.getAvailableProviders uses get_available_providers", async () => {
    await configAPI.getAvailableProviders();
    expect(invokeMock).toHaveBeenCalledWith("get_available_providers");
  });

  it("backupAPI.githubBackupPushToGist uses github_backup_push_to_gist", async () => {
    const { backupAPI } = await import("./tauri");

    await backupAPI.githubBackupPushToGist({ gistId: "abc" });

    expect(invokeMock).toHaveBeenCalledWith("github_backup_push_to_gist", {
      gistId: "abc",
    });
  });

  it("tauriAPI.getHistoryPage uses get_history_page", async () => {
    await tauriAPI.getHistoryPage({ page: 2, pageSize: 10 });

    expect(invokeMock).toHaveBeenCalledWith("get_history_page", {
      params: { page: 2, pageSize: 10 },
    });
  });

  it("tauriAPI.cacheRouterEmbeddings uses cache_router_embeddings", async () => {
    await tauriAPI.cacheRouterEmbeddings({ profileId: "profile-1" });

    expect(invokeMock).toHaveBeenCalledWith("cache_router_embeddings", {
      profileId: "profile-1",
      forceRefresh: null,
    });
  });

  it("tauriAPI.emitSettingsChanged forwards to settings-changed", async () => {
    await tauriAPI.emitSettingsChanged({ theme: "dark" });

    expect(emitMock).toHaveBeenCalledWith("settings-changed", {
      theme: "dark",
    });
  });

  it("recordingsAPI.getRecordingAssetUrl uses recording_get_wav_path", async () => {
    const { recordingsAPI } = await import("./tauri");
    invokeMock.mockResolvedValueOnce("C:/tmp/file.wav");

    const url = await recordingsAPI.getRecordingAssetUrl({
      requestId: "req-1",
    });

    expect(invokeMock).toHaveBeenCalledWith("recording_get_wav_path", {
      requestId: "req-1",
    });
    expect(url).toBe("C:/tmp/file.wav");
  });
});
