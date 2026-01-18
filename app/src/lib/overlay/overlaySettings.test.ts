import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { createOverlaySettingsChangedHandler } from "./overlaySettings";

describe("createOverlaySettingsChangedHandler", () => {
  it("applies accent, reloads settings, invalidates, and syncs pipeline", async () => {
    const applyAccentColor = vi.fn();
    const reloadSettingsFromDisk = vi.fn(async () => undefined);
    const invoke = vi.fn(async () => undefined);
    const queryClient = new QueryClient();
    const invalidateSpy = vi
      .spyOn(queryClient, "invalidateQueries")
      .mockResolvedValue(undefined as never);

    const handler = createOverlaySettingsChangedHandler({
      applyAccentColor,
      reloadSettingsFromDisk,
      queryClient,
      invoke,
    });

    await handler({ accent_color: "#ff0000" });

    expect(applyAccentColor).toHaveBeenCalledWith("#ff0000");
    expect(reloadSettingsFromDisk).toHaveBeenCalled();
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["settings"] });
    expect(invoke).toHaveBeenCalledWith("sync_pipeline_config");
  });

  it("still reloads and syncs without accent payload", async () => {
    const applyAccentColor = vi.fn();
    const reloadSettingsFromDisk = vi.fn(async () => undefined);
    const invoke = vi.fn(async () => undefined);
    const queryClient = new QueryClient();
    vi.spyOn(queryClient, "invalidateQueries").mockResolvedValue(
      undefined as never,
    );

    const handler = createOverlaySettingsChangedHandler({
      applyAccentColor,
      reloadSettingsFromDisk,
      queryClient,
      invoke,
    });

    await handler({});

    expect(applyAccentColor).not.toHaveBeenCalled();
    expect(reloadSettingsFromDisk).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("sync_pipeline_config");
  });
});
