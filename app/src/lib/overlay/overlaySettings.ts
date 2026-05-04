import type { QueryClient } from "@tanstack/react-query";
import type { SettingsChangedPayload } from "../tauri";

type OverlaySettingsDeps = {
  applyAccentColor: (color: string | null) => void;
  reloadSettingsFromDisk: () => Promise<void>;
  queryClient: QueryClient;
};

export function createOverlaySettingsChangedHandler(
  deps: OverlaySettingsDeps,
): (payload: SettingsChangedPayload) => Promise<void> {
  const { applyAccentColor, reloadSettingsFromDisk, queryClient } = deps;

  let latestRevisionSeen = 0;

  return async (payload: SettingsChangedPayload) => {
    // Ignore stale events (can happen if multiple saves/reloads race).
    try {
      const maybeObj = payload as unknown;
      if (maybeObj && typeof maybeObj === "object") {
        const rawRev = (maybeObj as Record<string, unknown>).settings_revision;
        const rev =
          typeof rawRev === "number" && Number.isFinite(rawRev)
            ? Math.trunc(rawRev)
            : null;
        if (typeof rev === "number") {
          if (rev <= latestRevisionSeen) return;
          latestRevisionSeen = rev;
        }
      }
    } catch {
      // ignore (non-critical)
    }

    // Apply accent immediately (without waiting on any disk reload).
    try {
      const maybeObj = payload as unknown;
      if (maybeObj && typeof maybeObj === "object") {
        const accent = (maybeObj as Record<string, unknown>).accent_color;
        if (accent === null || typeof accent === "string") {
          applyAccentColor(accent);
        }
      }
    } catch {
      // ignore (non-critical)
    }

    // In the overlay window, force a disk reload so *all* settings fields reflect
    // the latest changes made by the main window.
    let revAtStart: number | null = null;
    try {
      const maybeObj = payload as unknown;
      if (maybeObj && typeof maybeObj === "object") {
        const rawRev = (maybeObj as Record<string, unknown>).settings_revision;
        if (typeof rawRev === "number" && Number.isFinite(rawRev)) {
          revAtStart = Math.trunc(rawRev);
        }
      }
    } catch {
      // ignore
    }

    try {
      await reloadSettingsFromDisk();
    } catch {
      // ignore
    }

    // If a newer revision arrived while we were reloading, don't do extra work.
    if (typeof revAtStart === "number" && revAtStart !== latestRevisionSeen) {
      return;
    }

    queryClient.invalidateQueries({ queryKey: ["settings"] });
  };
}
