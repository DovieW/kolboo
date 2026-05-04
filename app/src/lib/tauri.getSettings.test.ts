import { describe, expect, vi } from "vitest";
import { itWithImportTimeout } from "./testTimeouts";

type StoreLike = {
  get<T = unknown>(key: string): Promise<T | undefined>;
  set<T = unknown>(key: string, value: T): Promise<void>;
  delete(key: string): Promise<void>;
  save(): Promise<void>;
};

class FakeStore implements StoreLike {
  public readonly data = new Map<string, unknown>();
  public readonly setCalls: Array<{ key: string; value: unknown }> = [];
  public readonly deleteCalls: string[] = [];
  public saveCalls = 0;

  constructor(initial: Record<string, unknown>) {
    for (const [k, v] of Object.entries(initial)) {
      this.data.set(k, v);
    }
  }

  async get<T = unknown>(key: string): Promise<T | undefined> {
    return this.data.get(key) as T | undefined;
  }

  async set<T = unknown>(key: string, value: T): Promise<void> {
    this.data.set(key, value);
    this.setCalls.push({ key, value });
  }

  async delete(key: string): Promise<void> {
    this.data.delete(key);
    this.deleteCalls.push(key);
  }

  async save(): Promise<void> {
    this.saveCalls += 1;
  }
}

let currentStore: FakeStore;

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (x: string) => x,
  invoke: vi.fn(async () => {
    throw new Error("invoke() not implemented in unit tests");
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {
    // no-op
  }),
  listen: vi.fn(async () => {
    return () => {
      // no-op
    };
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    // minimal stub
  })),
}));

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: vi.fn(async () => currentStore),
  },
}));

describe("tauriAPI.getSettings() normalization", () => {
	itWithImportTimeout(
    "migrates legacy cleanup_prompt_sections.main -> cleanup_prompt_sections.system (read-only)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        cleanup_prompt_sections: {
          main: "Hello legacy prompt",
        },
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.cleanup_prompt_sections).toEqual({
        system: { content: "Hello legacy prompt" },
      });

      // Getter should not mutate the store.
      expect(currentStore.setCalls).toHaveLength(0);
      expect(currentStore.saveCalls).toBe(0);
    },
  );

	itWithImportTimeout(
    "defaults invalid accent_color to DEFAULT_ACCENT_HEX (read-only)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        accent_color: "not-a-color",
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      // Don’t hardcode the hex; import the canonical constant.
      const { DEFAULT_ACCENT_HEX } = await import("./accentColor");
      expect(settings.accent_color).toBe(DEFAULT_ACCENT_HEX);
      expect(currentStore.setCalls).toHaveLength(0);
      expect(currentStore.saveCalls).toBe(0);
    },
  );

	itWithImportTimeout(
    "migrates legacy profile program_path -> program_paths[]",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        rewrite_program_prompt_profiles: [
          {
            id: "profile-1",
            name: "Profile One",
            program_path: "C:\\Program Files\\Foo\\foo.exe",
          },
        ],
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.rewrite_program_prompt_profiles).toHaveLength(1);
      expect(
        settings.rewrite_program_prompt_profiles[0]?.program_paths,
      ).toEqual(["C:\\Program Files\\Foo\\foo.exe"]);
    },
  );

	itWithImportTimeout(
    "uses legacy quick_ask_hotkey as fallback when quick_ask_hold_hotkey is missing",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_hotkey: { modifiers: ["Control"], key: "K" },
        // quick_ask_hold_hotkey intentionally missing
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.quick_ask_hold_hotkey).toEqual({
        modifiers: ["Control"],
        key: "K",
      });
    },
  );

	itWithImportTimeout(
    "does NOT fall back to legacy quick_ask_hotkey when quick_ask_hold_hotkey is explicitly null",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_hold_hotkey: null,
        quick_ask_hotkey: { modifiers: ["Control"], key: "K" },
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.quick_ask_hold_hotkey).toBeNull();
    },
  );

	itWithImportTimeout(
    "normalizes quick_ask_dismiss_mode (invalid -> manual)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_dismiss_mode: "banana",
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.quick_ask_dismiss_mode).toBe("manual");
    },
  );

	itWithImportTimeout(
    "normalizes quick_ask_conversation_history_count (invalid/missing -> default, clamps range)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_conversation_history_count: "banana",
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();
      expect(settings.quick_ask_conversation_history_count).toBe(3);

      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_conversation_history_count: 0,
      });
      const { tauriAPI: tauriAPI2 } = await import("./tauri");
      const settings2 = await tauriAPI2.getSettings();
      expect(settings2.quick_ask_conversation_history_count).toBe(1);

      vi.resetModules();
      currentStore = new FakeStore({
        quick_ask_conversation_history_count: 999,
      });
      const { tauriAPI: tauriAPI3 } = await import("./tauri");
      const settings3 = await tauriAPI3.getSettings();
      expect(settings3.quick_ask_conversation_history_count).toBe(20);
    },
  );

	itWithImportTimeout(
    "normalizes malformed cleanup_prompt_sections.system (read-only)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        cleanup_prompt_sections: {
          system: { content: 123, extra: "ignored" },
          other: "ignored",
        },
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.cleanup_prompt_sections).toEqual({
        system: { content: null },
      });
      expect(currentStore.setCalls).toHaveLength(0);
      expect(currentStore.saveCalls).toBe(0);
    },
  );

	itWithImportTimeout(
    "migrates legacy auto_mute_audio boolean into playing_audio_handling",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        auto_mute_audio: true,
        // playing_audio_handling intentionally missing
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();
      expect(settings.playing_audio_handling).toBe("mute");

      vi.resetModules();
      currentStore = new FakeStore({
        auto_mute_audio: false,
      });
      const { tauriAPI: tauriAPI2 } = await import("./tauri");
      const settings2 = await tauriAPI2.getSettings();
      expect(settings2.playing_audio_handling).toBe("none");
    },
  );

	itWithImportTimeout(
    "uses legacy noise_gate_strength when noise_gate_threshold_dbfs is missing",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        noise_gate_strength: 50,
        // noise_gate_threshold_dbfs intentionally missing
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      // strength 50 => -75 + (0.5 * 45) = -52.5
      expect(settings.noise_gate_threshold_dbfs).not.toBeNull();
      expect(settings.noise_gate_threshold_dbfs).toBeCloseTo(-52.5, 5);
    },
  );

	itWithImportTimeout(
    "uses legacy transcription_retention_days when unit+value are missing",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        transcription_retention_days: 2.2,
        // transcription_retention_unit/value intentionally missing
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();
      expect(settings.transcription_retention_unit).toBe("days");
      expect(settings.transcription_retention_value).toBe(2);
    },
  );

	itWithImportTimeout(
    "normalizes profile.disabled (missing -> false, preserves true/false)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        rewrite_program_prompt_profiles: [
          {
            id: "profile-a",
            name: "Profile A",
            program_paths: [],
            // disabled missing
          },
          {
            id: "profile-b",
            name: "Profile B",
            program_paths: [],
            disabled: true,
          },
          {
            id: "profile-c",
            name: "Profile C",
            program_paths: [],
            disabled: false,
          },
        ],
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.rewrite_program_prompt_profiles).toHaveLength(3);
      expect(settings.rewrite_program_prompt_profiles[0]?.disabled).toBe(false);
      expect(settings.rewrite_program_prompt_profiles[1]?.disabled).toBe(true);
      expect(settings.rewrite_program_prompt_profiles[2]?.disabled).toBe(false);
    },
  );

	itWithImportTimeout(
    "normalizes legacy/typo enum values (overlay_monitor_target, main_window_close_behavior, output_mode)",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        overlay_monitor_target: "activeWindow",
        main_window_close_behavior: "close_window",
        output_mode: "keystrokes",
      });

      const { tauriAPI } = await import("./tauri");
      const settings = await tauriAPI.getSettings();

      expect(settings.overlay_monitor_target).toBe("active_window");
      expect(settings.main_window_close_behavior).toBe("minimize_to_tray");
      expect(settings.output_mode).toBe("paste");
    },
  );

	itWithImportTimeout(
    "uses Settings View defaults for malformed flat settings without mutating the store",
    async () => {
      vi.resetModules();
      currentStore = new FakeStore({
        sound_enabled: "yes please",
        overlay_mode: "sideways",
        output_hit_enter: "true",
      });

      const { tauriAPI } = await import("./tauri");
      const { DEFAULT_SETTINGS_VALUES } =
        await import("./tauri/settingsDefaults");
      const settings = await tauriAPI.getSettings();

      expect(settings.sound_enabled).toBe(
        DEFAULT_SETTINGS_VALUES.sound_enabled,
      );
      expect(settings.overlay_mode).toBe(DEFAULT_SETTINGS_VALUES.overlay_mode);
      expect(settings.output_hit_enter).toBe(
        DEFAULT_SETTINGS_VALUES.output_hit_enter,
      );
      expect(currentStore.setCalls).toHaveLength(0);
      expect(currentStore.saveCalls).toBe(0);
    },
  );
});
