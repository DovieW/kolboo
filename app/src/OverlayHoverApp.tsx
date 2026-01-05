import "./app.css";

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { tauriAPI, type IntentRouterSettings } from "./lib/tauri";
import { useSettings } from "./lib/queries";

type ActiveProfileInfo = {
  profile_id: string | null;
  profile_name: string | null;
};

type SessionPresetLockInfo = {
  profile_id: string | null;
  preset_id: string | null;
};

export default function OverlayHoverApp() {
  const { data: settings } = useSettings();
  const [activeProfile, setActiveProfile] = useState<ActiveProfileInfo | null>(
    null
  );
  const activeProfileId = activeProfile?.profile_id ?? null;

  const [sessionLock, setSessionLock] = useState<SessionPresetLockInfo>({
    profile_id: null,
    preset_id: null,
  });

  const hideTimerRef = useRef<number | null>(null);

  const scheduleHide = useCallback((delayMs: number) => {
    if (hideTimerRef.current) {
      window.clearTimeout(hideTimerRef.current);
    }
    hideTimerRef.current = window.setTimeout(() => {
      tauriAPI.scheduleHideOverlayHover(0);
      hideTimerRef.current = null;
    }, delayMs);
  }, []);

  const keepAlive = useCallback(() => {
    // Re-showing is idempotent and cancels any pending backend hide.
    tauriAPI.showOverlayHover();
  }, []);

  // IMPORTANT: do not auto-show the hover window on mount. The backend controls
  // visibility and positioning; we only refresh while the pointer is over the panel.

  // Resolve the active program profile periodically.
  useEffect(() => {
    let cancelled = false;
    let interval: number | null = null;

    const sync = async () => {
      try {
        const result = await invoke<ActiveProfileInfo>(
          "pipeline_get_active_profile_for_foreground_app"
        );
        if (cancelled) return;
        setActiveProfile({
          profile_id: result?.profile_id ?? null,
          profile_name: result?.profile_name ?? null,
        });
      } catch {
        // ignore
      }
    };

    sync();
    interval = window.setInterval(sync, 1500);
    return () => {
      cancelled = true;
      if (interval) window.clearInterval(interval);
    };
  }, []);

  // Poll the current session preset lock (so both windows stay in sync).
  useEffect(() => {
    let cancelled = false;
    let interval: number | null = null;

    const sync = async () => {
      try {
        const result = await invoke<SessionPresetLockInfo>(
          "pipeline_get_session_preset_lock"
        );
        if (cancelled) return;
        setSessionLock({
          profile_id: result?.profile_id ?? null,
          preset_id: result?.preset_id ?? null,
        });
      } catch {
        // ignore
      }
    };

    sync();
    interval = window.setInterval(sync, 600);
    return () => {
      cancelled = true;
      if (interval) window.clearInterval(interval);
    };
  }, []);

  const activeProfilePresets = useMemo(() => {
    if (!settings) return [];
    if (!activeProfileId) return [];
    if (activeProfileId === "default") return [];
    const profile = settings.rewrite_program_prompt_profiles.find(
      (p) => p.id === activeProfileId
    );
    return profile?.presets ?? [];
  }, [settings, activeProfileId]);

  const activeProfileRouter = useMemo(() => {
    if (!settings) return null;
    if (!activeProfileId) return null;
    if (activeProfileId === "default") return null;
    const profile = settings.rewrite_program_prompt_profiles.find(
      (p) => p.id === activeProfileId
    );
    return profile?.router ?? null;
  }, [settings, activeProfileId]);

  const routerIsEffectivelyOn =
    !!activeProfileRouter &&
    activeProfileRouter.enabled &&
    activeProfileRouter.strategy !== "off";

  const toggleRouterEnabled = useCallback(async () => {
    if (!settings) return;
    if (!activeProfileId || activeProfileId === "default") return;

    const profiles = settings.rewrite_program_prompt_profiles;
    const idx = profiles.findIndex((p) => p.id === activeProfileId);
    if (idx < 0) return;

    const profile = profiles[idx];
    if (!profile) return;
    const current = profile.router ?? null;

    const nextRouter: IntentRouterSettings = (() => {
      if (routerIsEffectivelyOn) {
        if (!current) return { enabled: false, strategy: "off" };
        return { ...current, enabled: false };
      }

      if (current && current.strategy !== "off") {
        return { ...current, enabled: true };
      }

      return {
        enabled: true,
        strategy: "embeddings",
        embedding_provider: "openai",
        embedding_model: "text-embedding-3-small",
        pick_highest_score: false,
        similarity_threshold: null,
        similarity_margin: null,
      };
    })();

    const nextProfiles = profiles.map((p) =>
      p.id === activeProfileId ? { ...p, router: nextRouter } : p
    );

    try {
      await tauriAPI.updateRewriteProgramPromptProfiles(nextProfiles);
      await tauriAPI.emitSettingsChanged({});
    } catch (error) {
      console.error("[OverlayHover] Failed to toggle router:", error);
    }
  }, [activeProfileId, routerIsEffectivelyOn, settings]);

  const setSessionPresetLock = useCallback(
    async (nextPresetId: string | null) => {
      try {
        const profileIdForLock =
          activeProfileId && activeProfileId !== "default"
            ? activeProfileId
            : null;
        await invoke("pipeline_set_session_preset_lock", {
          profileId: profileIdForLock,
          presetId: nextPresetId ?? null,
        });
        // Refresh quickly.
        keepAlive();
      } catch {
        // ignore
      }
    },
    [activeProfileId, keepAlive]
  );

  return (
    <div
      className="overlay-hover-panel"
      role="dialog"
      aria-label="Preset controls"
      onMouseEnter={() => {
        keepAlive();
        if (hideTimerRef.current) {
          window.clearTimeout(hideTimerRef.current);
          hideTimerRef.current = null;
        }
      }}
      onMouseMove={() => {
        keepAlive();
      }}
      onMouseLeave={() => {
        scheduleHide(200);
      }}
      style={{
        position: "fixed",
        left: 0,
        top: 0,
        right: "auto",
        bottom: "auto",
        transform: "none",
      }}
    >
      <div className="overlay-hover-title">Presets</div>
      <div className="overlay-hover-subtitle">
        {(activeProfile?.profile_name ?? activeProfileId ?? "Detecting app…")
          .trim()
          .slice(0, 64)}
      </div>

      <div className="overlay-hover-row">
        <button
          type="button"
          className="overlay-hover-btn"
          data-kind="toggle"
          data-active={routerIsEffectivelyOn ? "true" : "false"}
          disabled={!activeProfileId || activeProfileId === "default"}
          onClick={() => {
            toggleRouterEnabled();
          }}
          title={
            !activeProfileId
              ? "Detecting the foreground app…"
              : activeProfileId === "default"
              ? "No program profile is active (Default)"
              : undefined
          }
        >
          Intent router: {routerIsEffectivelyOn ? "On" : "Off"}
        </button>
      </div>

      <div className="overlay-hover-row">
        <button
          type="button"
          className="overlay-hover-btn"
          data-active={!sessionLock.preset_id ? "true" : "false"}
          disabled={!activeProfileId}
          onClick={() => {
            setSessionPresetLock(null);
          }}
        >
          Auto
        </button>
        {activeProfilePresets.length > 0 ? (
          activeProfilePresets.map((p) => (
            <button
              key={p.id}
              type="button"
              className="overlay-hover-btn"
              data-active={sessionLock.preset_id === p.id ? "true" : "false"}
              disabled={!activeProfileId}
              title={p.description ?? undefined}
              onClick={() => {
                setSessionPresetLock(p.id);
              }}
            >
              {p.name.trim().slice(0, 20) || "Preset"}
            </button>
          ))
        ) : (
          <span className="overlay-hover-footnote">
            No presets for this profile.
          </span>
        )}
      </div>
    </div>
  );
}
