import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Store } from "@tauri-apps/plugin-store";
import { z } from "zod";
import { DEFAULT_ACCENT_HEX, normalizeHexColor } from "./accentColor";

/**
 * Connection state for UI display (maps from pipeline state)
 */
export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "idle"
  | "recording"
  | "processing";

interface TypeTextResult {
  success: boolean;
  error?: string;
}

export interface HotkeyConfig {
  modifiers: string[];
  key: string;
}

// Zod schema for HotkeyConfig validation
export const HotkeyConfigSchema = z.object({
  modifiers: z.array(z.string()),
  key: z.string().min(1, "Key is required"),
});

function normalizeHotkeyConfig(
  value: unknown,
  fallback: HotkeyConfig | null
): HotkeyConfig | null {
  // Explicit null means "disabled".
  if (value === null) return null;

  // Missing/invalid means fallback to default.
  const result = HotkeyConfigSchema.safeParse(value);
  return result.success ? result.data : fallback;
}

interface HistoryEntry {
  id: string;
  timestamp: string;
  text: string;
  status?: "in_progress" | "success" | "error";
  error_message?: string | null;
  profile_id?: string | null;
  profile_name?: string | null;
  stt_provider?: string | null;
  stt_model?: string | null;
  llm_provider?: string | null;
  llm_model?: string | null;
  // Request id of the WAV recording to use for playback/rerun.
  recording_request_id?: string | null;
}

export type HistoryDeleteMode =
  | "entry_only"
  | "entry_and_recording"
  | "recording_and_all_entries";

export interface HistoryDeleteOptions {
  recording_id: string | null;
  recording_exists: boolean;
  recording_ref_count: number;
}

export interface HistoryDeleteResult {
  deleted_entries: number;
  deleted_recording: boolean;
}

export interface HistoryPageQuery {
  filterText?: string;
  showFailed?: boolean;
  showEmptyTranscript?: boolean;
  selectedSttModelKeys?: string[];
  selectedLlmModelKeys?: string[];
  page?: number;
  pageSize?: number;
  includeUsageCounts?: boolean;
}

export interface ModelUsageCount {
  key: string;
  count: number;
}

export interface HistoryPageResult {
  items: HistoryEntry[];
  totalAll: number;
  totalFiltered: number;
  page: number;
  pageSize: number;
  sttModelUsage: ModelUsageCount[];
  llmModelUsage: ModelUsageCount[];
}

export interface PromptSection {
  enabled: boolean;
  content: string | null;
}

export interface CleanupPromptSections {
  main: PromptSection;
  advanced: PromptSection;
  dictionary: PromptSection;
}

// Per-profile prompt overrides: each section can be omitted/null to inherit from Default.
export interface CleanupPromptSectionsOverride {
  main?: PromptSection | null;
  advanced?: PromptSection | null;
  dictionary?: PromptSection | null;
}

export interface RewriteProgramPromptProfile {
  id: string;
  name: string;
  program_paths: string[];
  cleanup_prompt_sections: CleanupPromptSectionsOverride | null;

  // Per-profile gate for the optional LLM rewrite step (falls back to AppSettings.rewrite_llm_enabled)
  rewrite_llm_enabled?: boolean | null;

  // Per-profile overrides for the pipeline
  stt_provider?: string | null;
  stt_model?: string | null;
  stt_timeout_seconds?: number | null;
  llm_provider?: string | null;
  llm_model?: string | null;

  // Per-profile provider-specific thinking/reasoning knobs
  // (null/undefined means inherit from Default/global settings)
  openai_reasoning_effort?: OpenAiReasoningEffort | null;
  gemini_thinking_budget?: number | null;
  gemini_thinking_level?: "minimal" | "low" | "medium" | "high" | null;
  anthropic_thinking_budget?: number | null;

  // Per-profile overrides for UI (Option 1: override-or-inherit)
  // NOTE: These are persisted in settings.json as part of the profile object.
  // The backend may ignore them until it is updated to apply them at runtime.
  sound_enabled?: boolean | null;
  playing_audio_handling?: PlayingAudioHandling | null;
  overlay_mode?: OverlayMode | null;
  widget_position?: WidgetPosition | null;
  output_mode?: OutputMode | null;

  // After paste, optionally press Enter.
  // (May be ignored by backend until runtime/profile routing supports it.)
  output_hit_enter?: boolean | null;
}

export type PlayingAudioHandling = "none" | "mute" | "pause" | "mute_and_pause";

export type AudioCue = "kolboo" | "maraca" | "clave" | "legacy";

export type OverlayMode = "always" | "never" | "recording_only";

export type WidgetPosition =
  | "center"
  | "top-left"
  | "top-center"
  | "top-right"
  | "bottom-left"
  | "bottom-center"
  | "bottom-right";

export type OutputMode = "paste" | "paste_and_clipboard" | "clipboard";

export type TranscriptionRetentionUnit = "days" | "hours";

export type RequestLogsRetentionMode = "amount" | "time";

export type SettingsGuideState = "pending" | "skipped" | "completed";

// ============================================================================
// Network / proxy settings
// ============================================================================

export type ProxyMode = "no_proxy" | "system" | "manual";

export type TrustedCaCertFormat = "pem" | "der";

export interface TrustedCaCertificate {
  id: string;
  file_name: string;
  format: TrustedCaCertFormat;
  data_base64: string;
}

export interface ManualProxySettings {
  /** Proxy URL applied to both http + https. Example: "http://127.0.0.1:8080" */
  proxy_url: string;
  /** Comma- or whitespace-separated bypass list (NO_PROXY semantics). */
  no_proxy: string;
  /** Optional basic auth username. */
  username: string;
  /** Optional basic auth password. */
  password: string;
}

export interface ProxySettings {
  mode: ProxyMode;
  manual: ManualProxySettings;
  trusted_ca_certificates: TrustedCaCertificate[];
  /** When true, accept invalid TLS certs (including self-signed). */
  danger_accept_invalid_certs: boolean;
}

export interface WindowsInternetProxySettings {
  proxy_enable: boolean | null;
  proxy_server: string | null;
  proxy_override: string | null;
  auto_config_url: string | null;
}

export interface SystemProxyInfo {
  env_http_proxy: string | null;
  env_https_proxy: string | null;
  env_no_proxy: string | null;

  // Windows-only, best-effort.
  windows_internet_settings: WindowsInternetProxySettings | null;
}

export type OpenAiReasoningEffort =
  | "none"
  | "minimal"
  | "low"
  | "medium"
  | "high"
  | "xhigh";

export type CostTimeframe = "24h" | "7d" | "30d" | "90d" | "all";

export interface CostSummary {
  timeframe: CostTimeframe | string;
  total_usd_micros: number;
  events_total: number;
  events_with_cost: number;
  earliest_included_at: string | null;
  latest_included_at: string | null;
}

export interface ProviderCostTotal {
  provider: string;
  total_usd_micros: number;
  events_total: number;
  events_with_cost: number;
}

export interface CostByProvider {
  timeframe: CostTimeframe | string;
  providers: ProviderCostTotal[];
}

export type ModelPricingKind = "stt" | "llm";

export interface SttModelPricing {
  usd_micros_per_minute?: number | null;
  usd_micros_per_hour?: number | null;
  min_billed_secs?: number | null;
}

export interface LlmModelPricing {
  input_usd_micros_per_1m: number;
  cached_input_usd_micros_per_1m?: number | null;
  output_usd_micros_per_1m: number;
}

export interface ModelPricing {
  kind: ModelPricingKind;
  provider: string;
  model: string;
  stt?: SttModelPricing | null;
  llm?: LlmModelPricing | null;
}

function normalizeOutputMode(value: unknown): OutputMode {
  if (
    value === "paste" ||
    value === "paste_and_clipboard" ||
    value === "clipboard"
  ) {
    return value;
  }

  // Legacy/disabled values:
  // - "keystrokes"
  // - "keystrokes_and_clipboard"
  // - "auto_paste"
  return "paste";
}

// What the window close (X) button does for the main/settings window.
//
// NOTE: We previously used "close_window" (destroy the window but keep the tray app running).
// That option is now treated as legacy and maps to "exit_program".
export type MainWindowCloseBehavior = "exit_program" | "minimize_to_tray";

function normalizeMainWindowCloseBehavior(
  value: unknown
): MainWindowCloseBehavior {
  if (value === "minimize_to_tray" || value === "exit_program") return value;

  // Legacy value (kept for backward compatibility)
  if (value === "close_window") return "minimize_to_tray";

  // Default for unknown/missing values
  return "minimize_to_tray";
}

export interface AppSettings {
  toggle_hotkey: HotkeyConfig | null;
  hold_hotkey: HotkeyConfig | null;
  paste_last_hotkey: HotkeyConfig | null;
  selected_mic_id: string | null;
  sound_enabled: boolean;
  audio_cue: AudioCue;
  /** User-selected accent color (hex). */
  accent_color: string | null;
  // Global gate for the optional LLM rewrite step
  rewrite_llm_enabled: boolean;
  cleanup_prompt_sections: CleanupPromptSections | null;
  rewrite_program_prompt_profiles: RewriteProgramPromptProfile[];
  stt_provider: string | null;
  stt_model: string | null;
  // Global STT prompt (applies to all transcriptions when supported by the selected provider/model)
  stt_transcription_prompt: string | null;
  // AquaVoice server override (optional)
  aquavoice_base_url: string | null;

  // Global proxy configuration for outgoing HTTP requests
  proxy_settings: ProxySettings;
  llm_provider: string | null;
  llm_model: string | null;

  // Provider-specific knobs
  // When true, treat Groq usage as free-tier (UI-only for now; kept in settings for future backend usage).
  groq_free_tier: boolean;

  // When true, treat AssemblyAI usage as free-tier for stats filtering.
  assemblyai_free_tier: boolean;

  // When true, treat Speechmatics usage as free-tier for stats filtering.
  speechmatics_free_tier: boolean;

  // Optional per-provider reasoning/thinking knobs.
  // These are ignored unless the selected provider/model supports them.
  openai_reasoning_effort: OpenAiReasoningEffort | null;
  anthropic_thinking_budget: number | null;
  gemini_thinking_budget: number | null;
  gemini_thinking_level: "minimal" | "low" | "medium" | "high" | null;

  playing_audio_handling: PlayingAudioHandling;
  stt_timeout_seconds: number | null;
  overlay_mode: OverlayMode;
  widget_position: WidgetPosition;
  output_mode: OutputMode;
  output_hit_enter: boolean;

  /** What the window close button does for the main/settings window. */
  main_window_close_behavior: MainWindowCloseBehavior;

  // Hallucination protection (quiet-audio gate)
  quiet_audio_gate_enabled: boolean;
  quiet_audio_min_duration_secs: number;
  quiet_audio_rms_dbfs_threshold: number;
  quiet_audio_peak_dbfs_threshold: number;
  // Extra protection: if enabled, also require that VAD detects speech.
  quiet_audio_require_speech: boolean;

  // Capture behavior (Hot Mic + recovery)
  // When enabled, keep the microphone stream open while idle and maintain a rolling pre-roll.
  hot_mic_enabled: boolean;
  // How much audio to keep before record start (ms). Only used when hot_mic_enabled is true.
  hot_mic_pre_roll_ms: number;
  // When enabled, watchdog the mic stream and attempt auto-recovery on hangs/disconnects.
  mic_auto_recover_enabled: boolean;

  // Experimental: noise gate threshold (dBFS). null means off.
  noise_gate_threshold_dbfs: number | null;

  // Voice pickup (stop-time preprocessing)
  audio_downmix_to_mono: boolean;
  audio_resample_to_16khz: boolean;
  audio_highpass_enabled: boolean;
  audio_agc_enabled: boolean;
  audio_noise_suppression_enabled: boolean;

  // How many recordings/history entries to retain
  max_saved_recordings: number;

  // Time-based retention for transcriptions/history.
  // 0 means keep forever.
  transcription_retention_unit: TranscriptionRetentionUnit;
  transcription_retention_value: number;
  // If enabled, deleting old transcriptions also deletes their recordings (best-effort).
  transcription_retention_delete_recordings: boolean;

  // Persisted stats retention (usage/cost events).
  // 0 means keep forever.
  stats_retention_unit: TranscriptionRetentionUnit;
  stats_retention_value: number;
  // Defensive cap for on-disk stats storage.
  stats_retention_max_bytes: number;

  // Request logs retention (in-memory request log history)
  request_logs_retention_mode: RequestLogsRetentionMode;
  // Only used when mode === "amount"
  request_logs_retention_amount: number;
  // Only used when mode === "time" (0 = forever)
  request_logs_retention_days: number;
}

function normalizeProxyMode(value: unknown): ProxyMode {
  if (value === "no_proxy" || value === "system" || value === "manual") {
    return value;
  }
  return "system";
}

function normalizeManualProxySettings(value: unknown): ManualProxySettings {
  const v = value && typeof value === "object" ? (value as any) : ({} as any);

  const proxy_url = typeof v.proxy_url === "string" ? v.proxy_url : "";
  const no_proxy =
    typeof v.no_proxy === "string" ? v.no_proxy : "localhost,127.0.0.1";
  const username = typeof v.username === "string" ? v.username : "";
  const password = typeof v.password === "string" ? v.password : "";

  return { proxy_url, no_proxy, username, password };
}

function normalizeProxySettings(value: unknown): ProxySettings {
  const v = value && typeof value === "object" ? (value as any) : ({} as any);
  const mode = normalizeProxyMode(v.mode);
  const manual = normalizeManualProxySettings(v.manual);

  const normalizeTrustedCaCertFormat = (
    value: unknown
  ): TrustedCaCertFormat => {
    return value === "der" ? "der" : "pem";
  };

  const normalizeTrustedCaCertificate = (
    value: unknown
  ): TrustedCaCertificate | null => {
    if (!value || typeof value !== "object") return null;
    const x = value as any;
    const id = typeof x.id === "string" ? x.id : "";
    const file_name = typeof x.file_name === "string" ? x.file_name : "";
    const format = normalizeTrustedCaCertFormat(x.format);
    const data_base64 = typeof x.data_base64 === "string" ? x.data_base64 : "";
    if (!id || !data_base64) return null;
    return { id, file_name, format, data_base64 };
  };

  const trusted_ca_certificates: TrustedCaCertificate[] = Array.isArray(
    v.trusted_ca_certificates
  )
    ? v.trusted_ca_certificates
        .map(normalizeTrustedCaCertificate)
        .filter((c): c is TrustedCaCertificate => c !== null)
    : [];

  const danger_accept_invalid_certs =
    typeof v.danger_accept_invalid_certs === "boolean"
      ? v.danger_accept_invalid_certs
      : false;

  return {
    mode,
    manual,
    trusted_ca_certificates,
    danger_accept_invalid_certs,
  };
}

function normalizePlayingAudioHandling(value: unknown): PlayingAudioHandling {
  if (
    value === "none" ||
    value === "mute" ||
    value === "pause" ||
    value === "mute_and_pause"
  ) {
    return value;
  }

  // Legacy boolean (auto_mute_audio) migration:
  // - true  => mute
  // - false => none
  if (typeof value === "boolean") {
    return value ? "mute" : "none";
  }

  // Default for fresh installs / missing setting
  return "mute";
}

function normalizeAudioCue(value: unknown): AudioCue {
  if (
    value === "kolboo" ||
    value === "maraca" ||
    value === "clave" ||
    value === "legacy"
  ) {
    return value;
  }

  // Default for fresh installs / missing setting
  return "kolboo";
}

function normalizeNoiseGateStrength(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0;
  const rounded = Math.round(value);
  return Math.min(100, Math.max(0, rounded));
}

function normalizeOpenAiReasoningEffort(
  value: unknown
): OpenAiReasoningEffort | null {
  if (typeof value !== "string") return null;
  const v = value.trim().toLowerCase();
  if (
    v === "none" ||
    v === "minimal" ||
    v === "low" ||
    v === "medium" ||
    v === "high" ||
    v === "xhigh"
  ) {
    return v;
  }
  return null;
}

function normalizeGeminiThinkingLevel(
  value: unknown
): "minimal" | "low" | "medium" | "high" | null {
  if (typeof value !== "string") return null;
  const v = value.trim().toLowerCase();
  if (v === "minimal" || v === "low" || v === "medium" || v === "high")
    return v;
  return null;
}

function normalizeGeminiThinkingBudget(value: unknown): number | null {
  if (value == null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  // Keep it integer-ish (Gemini expects an integer token budget).
  return Math.trunc(value);
}

function normalizeAnthropicThinkingBudget(value: unknown): number | null {
  if (value == null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  // Keep it integer-ish; Anthropic expects an integer token budget.
  const n = Math.trunc(value);
  // The cookbook notes a minimum budget of 1024 for extended thinking.
  if (n < 1024) return 1024;
  // Defensive cap; actual max varies by model.
  return Math.min(32768, n);
}

function normalizeAnthropicThinkingBudgetAllowOff(
  value: unknown
): number | null {
  // For per-profile overrides we want an explicit "off" state even if the
  // Default/global setting enables thinking. Represent that as 0.
  if (value == null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const n = Math.trunc(value);
  if (n <= 0) return 0;
  if (n < 1024) return 1024;
  return Math.min(32768, n);
}

function normalizeNoiseGateThresholdDbfs(value: unknown): number | null {
  if (value == null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  // Clamp to the UI range.
  return Math.min(-30, Math.max(-75, value));
}

function noiseGateStrengthToThresholdDbfs(strength: number): number | null {
  const s = normalizeNoiseGateStrength(strength);
  if (s <= 0) return null;
  // Map 1..100 => -75..-30 (same range as the Rust mapping).
  const t = -75 + (s / 100) * 45;
  return Math.min(-30, Math.max(-75, t));
}

function noiseGateThresholdDbfsToStrength(
  thresholdDbfs: number | null
): number {
  if (thresholdDbfs == null) return 0;
  const t = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
  if (t == null) return 0;
  const s = ((t + 75) / 45) * 100;
  // Never return 0 when enabled; old UI treated 0 as off.
  return Math.min(100, Math.max(1, Math.round(s)));
}

function normalizeMaxSavedRecordings(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 1000;
  const rounded = Math.round(value);
  // 1..100000 (defensive)
  return Math.min(100000, Math.max(1, rounded));
}

function normalizeTranscriptionRetentionUnit(
  value: unknown
): TranscriptionRetentionUnit {
  if (value === "days" || value === "hours") return value;
  return "days";
}

function normalizeTranscriptionRetentionValue(
  value: unknown,
  unit: TranscriptionRetentionUnit
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0;
  const clamped = Math.max(0, value);

  if (unit === "days") {
    const rounded = Math.round(clamped);
    // 0..36500 days (~100 years) defensive cap
    return Math.min(36500, Math.max(0, rounded));
  }

  // hours: allow decimals (e.g. 0.5). Cap at ~100 years worth of hours.
  const maxHours = 36500 * 24;
  return Math.min(maxHours, clamped);
}

function normalizeTranscriptionRetentionDeleteRecordings(
  value: unknown
): boolean {
  return typeof value === "boolean" ? value : false;
}

function normalizeStatsRetentionMaxBytes(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 50_000_000;
  const rounded = Math.round(value);
  // 1MB..5GB (defensive)
  return Math.min(5_000_000_000, Math.max(1_000_000, rounded));
}

function normalizeRequestLogsRetentionMode(
  value: unknown
): RequestLogsRetentionMode {
  return value === "time" || value === "amount" ? value : "amount";
}

function normalizeRequestLogsRetentionAmount(value: unknown): number {
  // Keep this modest to avoid runaway memory in the backend.
  if (typeof value !== "number" || !Number.isFinite(value)) return 50;
  const rounded = Math.round(value);
  // 1..1000 defensive
  return Math.min(1000, Math.max(1, rounded));
}

function normalizeRequestLogsRetentionDays(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 7;
  const rounded = Math.round(value);
  // 0..36500 (~100 years) defensive
  return Math.min(36500, Math.max(0, rounded));
}

// ============================================================================
// Default values - must match Rust defaults
// ============================================================================

const DEFAULT_HOTKEY_MODIFIERS: string[] = [];

export const defaultToggleHotkey: HotkeyConfig = {
  modifiers: DEFAULT_HOTKEY_MODIFIERS,
  key: "F3",
};

export const defaultHoldHotkey: HotkeyConfig | null = null;

export const defaultPasteLastHotkey: HotkeyConfig | null = null;

// ============================================================================
// Store helpers
// ============================================================================

let storeInstance: Store | null = null;

const SETTINGS_GUIDE_STATE_KEY = "settings_guide_state";

function normalizeSettingsGuideState(value: unknown): SettingsGuideState {
  if (value === "pending" || value === "skipped" || value === "completed") {
    return value;
  }
  return "pending";
}

async function getStore(): Promise<Store> {
  if (!storeInstance) {
    storeInstance = await Store.load("settings.json");
  }
  return storeInstance;
}

// ============================================================================
// Hotkey validation helpers (Zod-based)
// ============================================================================

/**
 * Check if two hotkey configs are equivalent (case-insensitive comparison)
 */
export function hotkeyIsSameAs(a: HotkeyConfig, b: HotkeyConfig): boolean {
  if (a.key.toLowerCase() !== b.key.toLowerCase()) return false;
  if (a.modifiers.length !== b.modifiers.length) return false;
  return a.modifiers.every((mod) =>
    b.modifiers.some((other) => mod.toLowerCase() === other.toLowerCase())
  );
}

type HotkeyType = "toggle" | "hold" | "paste_last";

const HOTKEY_LABELS: Record<HotkeyType, string> = {
  toggle: "toggle",
  hold: "hold",
  paste_last: "paste last",
};

/**
 * Create a Zod schema for validating a hotkey doesn't conflict with existing hotkeys
 */
export function createHotkeyDuplicateSchema(
  allHotkeys: Record<HotkeyType, HotkeyConfig | null>,
  excludeType: HotkeyType
) {
  return HotkeyConfigSchema.superRefine((hotkey, ctx) => {
    for (const [type, existing] of Object.entries(allHotkeys)) {
      if (type === excludeType) continue;
      if (!existing) continue;

      if (hotkeyIsSameAs(hotkey, existing)) {
        ctx.addIssue({
          code: "custom",
          message: `This shortcut is already used for the ${
            HOTKEY_LABELS[type as HotkeyType]
          } hotkey`,
        });
        return;
      }
    }
  });
}

/**
 * Validate that a hotkey doesn't conflict with other hotkeys
 * Returns error message if invalid, null if valid
 */
export function validateHotkeyNotDuplicate(
  newHotkey: HotkeyConfig | null,
  allHotkeys: {
    toggle: HotkeyConfig | null;
    hold: HotkeyConfig | null;
    paste_last: HotkeyConfig | null;
  },
  excludeType: HotkeyType
): string | null {
  if (!newHotkey) return null;
  const schema = createHotkeyDuplicateSchema(allHotkeys, excludeType);
  const result = schema.safeParse(newHotkey);
  if (!result.success) {
    return result.error.issues[0]?.message ?? "Invalid hotkey";
  }
  return null;
}

// ============================================================================
// Tauri API
// ============================================================================

export const tauriAPI = {
  async typeText(text: string): Promise<TypeTextResult> {
    try {
      await invoke("type_text", { text });
      return { success: true };
    } catch (error) {
      return { success: false, error: String(error) };
    }
  },

  async onStartRecording(callback: () => void): Promise<UnlistenFn> {
    return listen("recording-start", callback);
  },

  async onStopRecording(callback: () => void): Promise<UnlistenFn> {
    return listen("recording-stop", callback);
  },

  async getCostSummary(params: {
    timeframe: CostTimeframe;
    kind?: "all" | "stt" | "llm";
    sttModelKeys?: string[];
    llmModelKeys?: string[];
    excludeFreeTier?: boolean;
  }): Promise<CostSummary> {
    const kind = params.kind === "all" ? undefined : params.kind;
    return invoke("get_cost_summary_v2", {
      params: {
        timeframe: params.timeframe,
        kind,
        sttModelKeys: params.sttModelKeys,
        llmModelKeys: params.llmModelKeys,
        excludeFreeTier: params.excludeFreeTier,
      },
    });
  },

  async getCostByProvider(params: {
    timeframe: CostTimeframe;
    kind?: "all" | "stt" | "llm";
    sttModelKeys?: string[];
    llmModelKeys?: string[];
    excludeFreeTier?: boolean;
  }): Promise<CostByProvider> {
    const kind = params.kind === "all" ? undefined : params.kind;
    return invoke("get_cost_by_provider_v2", {
      params: {
        timeframe: params.timeframe,
        kind,
        sttModelKeys: params.sttModelKeys,
        llmModelKeys: params.llmModelKeys,
        excludeFreeTier: params.excludeFreeTier,
      },
    });
  },

  async getModelPricing(params: {
    provider: string;
    kind: ModelPricingKind;
    model: string;
  }): Promise<ModelPricing | null> {
    return invoke("get_model_pricing", {
      provider: params.provider,
      kind: params.kind,
      model: params.model,
    });
  },

  // Settings API - using store plugin directly
  async getSettings(): Promise<AppSettings> {
    const store = await getStore();

    // Keep a tiny subset of settings mirrored in localStorage so the UI can apply
    // critical visuals (accent color) before the async store read completes.
    // This reduces first-paint flicker on startup.
    const tryWriteLocalStorage = (key: string, value: string | null) => {
      try {
        if (typeof window === "undefined") return;
        if (!window.localStorage) return;
        if (value === null) {
          window.localStorage.removeItem(key);
        } else {
          window.localStorage.setItem(key, value);
        }
      } catch {
        // ignore (private mode / disabled storage)
      }
    };

    const LOCAL_ACCENT_COLOR_KEY = "tv_accent_color";

    const normalizePromptSection = (value: any): PromptSection | null => {
      if (value === null) return null;
      if (!value || typeof value !== "object") return null;

      const enabled =
        typeof (value as any).enabled === "boolean"
          ? (value as any).enabled
          : true;
      const content =
        typeof (value as any).content === "string"
          ? (value as any).content
          : null;

      return { enabled, content };
    };

    const normalizeCleanupPromptSectionsOverride = (
      value: any
    ): CleanupPromptSectionsOverride | null => {
      if (value === null || value === undefined) return null;
      if (!value || typeof value !== "object") return null;

      const v = value as any;
      const out: CleanupPromptSectionsOverride = {};

      if (Object.prototype.hasOwnProperty.call(v, "main")) {
        out.main = normalizePromptSection(v.main);
      }
      if (Object.prototype.hasOwnProperty.call(v, "advanced")) {
        out.advanced = normalizePromptSection(v.advanced);
      }
      if (Object.prototype.hasOwnProperty.call(v, "dictionary")) {
        out.dictionary = normalizePromptSection(v.dictionary);
      }

      return out;
    };

    const normalizeRewriteProfile = (
      p: any
    ): RewriteProgramPromptProfile | null => {
      if (!p || typeof p !== "object") return null;
      const id = typeof p.id === "string" ? p.id : "";
      const name = typeof p.name === "string" ? p.name : "";

      const program_paths_raw = (p as any).program_paths;
      const legacy_program_path = (p as any).program_path;

      const program_paths = Array.isArray(program_paths_raw)
        ? program_paths_raw.filter((x) => typeof x === "string")
        : typeof legacy_program_path === "string" &&
          legacy_program_path.length > 0
        ? [legacy_program_path]
        : [];

      const cleanup_prompt_sections = normalizeCleanupPromptSectionsOverride(
        (p as any).cleanup_prompt_sections
      );
      const stt_provider =
        typeof (p as any).stt_provider === "string"
          ? (p as any).stt_provider
          : null;
      const stt_model =
        typeof (p as any).stt_model === "string" ? (p as any).stt_model : null;
      const stt_timeout_seconds =
        typeof (p as any).stt_timeout_seconds === "number"
          ? (p as any).stt_timeout_seconds
          : null;
      const llm_provider =
        typeof (p as any).llm_provider === "string"
          ? (p as any).llm_provider
          : null;
      const llm_model =
        typeof (p as any).llm_model === "string" ? (p as any).llm_model : null;

      const openai_reasoning_effort = normalizeOpenAiReasoningEffort(
        (p as any).openai_reasoning_effort
      );
      const gemini_thinking_budget = normalizeGeminiThinkingBudget(
        (p as any).gemini_thinking_budget
      );
      const gemini_thinking_level = normalizeGeminiThinkingLevel(
        (p as any).gemini_thinking_level
      );
      const anthropic_thinking_budget =
        normalizeAnthropicThinkingBudgetAllowOff(
          (p as any).anthropic_thinking_budget
        );
      const rewrite_llm_enabled =
        typeof (p as any).rewrite_llm_enabled === "boolean"
          ? (p as any).rewrite_llm_enabled
          : null;

      const sound_enabled =
        typeof (p as any).sound_enabled === "boolean"
          ? (p as any).sound_enabled
          : null;
      const playing_audio_handling_raw = (p as any).playing_audio_handling;
      const legacy_auto_mute_audio = (p as any).auto_mute_audio;

      const playing_audio_handling: PlayingAudioHandling | null =
        typeof playing_audio_handling_raw === "string"
          ? normalizePlayingAudioHandling(playing_audio_handling_raw)
          : typeof legacy_auto_mute_audio === "boolean"
          ? legacy_auto_mute_audio
            ? "mute"
            : "none"
          : null;

      const overlay_mode =
        (p as any).overlay_mode === "always" ||
        (p as any).overlay_mode === "never" ||
        (p as any).overlay_mode === "recording_only"
          ? ((p as any).overlay_mode as OverlayMode)
          : null;

      const widget_position =
        (p as any).widget_position === "center" ||
        (p as any).widget_position === "top-left" ||
        (p as any).widget_position === "top-center" ||
        (p as any).widget_position === "top-right" ||
        (p as any).widget_position === "bottom-left" ||
        (p as any).widget_position === "bottom-center" ||
        (p as any).widget_position === "bottom-right"
          ? ((p as any).widget_position as WidgetPosition)
          : null;

      const output_mode =
        typeof (p as any).output_mode === "string"
          ? normalizeOutputMode((p as any).output_mode)
          : null;

      const output_hit_enter =
        typeof (p as any).output_hit_enter === "boolean"
          ? (p as any).output_hit_enter
          : null;

      if (!id) return null;

      return {
        id,
        name,
        program_paths,
        cleanup_prompt_sections,
        rewrite_llm_enabled,
        stt_provider,
        stt_model,
        stt_timeout_seconds,
        llm_provider,
        llm_model,
        openai_reasoning_effort,
        gemini_thinking_budget,
        gemini_thinking_level,
        anthropic_thinking_budget,
        sound_enabled,
        playing_audio_handling,
        overlay_mode,
        widget_position,
        output_mode,
        output_hit_enter,
      };
    };

    const rawProfiles =
      (await store.get<any>("rewrite_program_prompt_profiles")) ?? [];
    const rewrite_program_prompt_profiles: RewriteProgramPromptProfile[] =
      Array.isArray(rawProfiles)
        ? rawProfiles
            .map(normalizeRewriteProfile)
            .filter((p): p is RewriteProgramPromptProfile => p !== null)
        : [];

    const settings: AppSettings = {
      toggle_hotkey: normalizeHotkeyConfig(
        await store.get("toggle_hotkey"),
        defaultToggleHotkey
      ),
      hold_hotkey: normalizeHotkeyConfig(
        await store.get("hold_hotkey"),
        defaultHoldHotkey
      ),
      paste_last_hotkey: normalizeHotkeyConfig(
        await store.get("paste_last_hotkey"),
        defaultPasteLastHotkey
      ),
      selected_mic_id:
        (await store.get<string | null>("selected_mic_id")) ?? null,
      sound_enabled: (await store.get<boolean>("sound_enabled")) ?? true,
      audio_cue: normalizeAudioCue(await store.get("audio_cue")),
      accent_color: await(async () => {
        const raw = (await store.get<string | null>("accent_color")) ?? null;
        const normalized = normalizeHexColor(raw);

        // If unset/invalid, default to the app's default accent.
        // (Tangerine is an explicit option in the UI, not the implicit default.)
        if (!normalized) {
          await store.set("accent_color", DEFAULT_ACCENT_HEX);
          await store.save();
          return DEFAULT_ACCENT_HEX;
        }

        return normalized;
      })(),
      rewrite_llm_enabled:
        (await store.get<boolean>("rewrite_llm_enabled")) ?? false,
      cleanup_prompt_sections:
        (await store.get<CleanupPromptSections | null>(
          "cleanup_prompt_sections"
        )) ?? null,
      rewrite_program_prompt_profiles,
      stt_provider: (await store.get<string | null>("stt_provider")) ?? null,
      stt_model: (await store.get<string | null>("stt_model")) ?? null,
      stt_transcription_prompt:
        (await store.get<string | null>("stt_transcription_prompt")) ?? null,
      aquavoice_base_url:
        (await store.get<string | null>("aquavoice_base_url")) ?? null,
      proxy_settings: normalizeProxySettings(await store.get("proxy_settings")),
      llm_provider: (await store.get<string | null>("llm_provider")) ?? null,
      llm_model: (await store.get<string | null>("llm_model")) ?? null,
      groq_free_tier: (await store.get<boolean>("groq_free_tier")) ?? true,
      assemblyai_free_tier:
        (await store.get<boolean>("assemblyai_free_tier")) ?? true,
      speechmatics_free_tier:
        (await store.get<boolean>("speechmatics_free_tier")) ?? true,
      openai_reasoning_effort: normalizeOpenAiReasoningEffort(
        await store.get("openai_reasoning_effort")
      ),
      anthropic_thinking_budget: normalizeAnthropicThinkingBudget(
        await store.get("anthropic_thinking_budget")
      ),
      gemini_thinking_budget: normalizeGeminiThinkingBudget(
        await store.get("gemini_thinking_budget")
      ),
      gemini_thinking_level: normalizeGeminiThinkingLevel(
        await store.get("gemini_thinking_level")
      ),
      playing_audio_handling: normalizePlayingAudioHandling(
        (await store.get("playing_audio_handling")) ??
          // Legacy key for migration:
          (await store.get<boolean>("auto_mute_audio")) ??
          // If neither exists, default to mute
          "mute"
      ),
      stt_timeout_seconds:
        (await store.get<number | null>("stt_timeout_seconds")) ?? null,
      overlay_mode:
        (await store.get<OverlayMode>("overlay_mode")) ?? "recording_only",
      widget_position:
        (await store.get<WidgetPosition>("widget_position")) ?? "bottom-center",
      output_mode: normalizeOutputMode(await store.get("output_mode")),
      output_hit_enter: (await store.get<boolean>("output_hit_enter")) ?? false,

      main_window_close_behavior: normalizeMainWindowCloseBehavior(
        await store.get("main_window_close_behavior")
      ),

      quiet_audio_gate_enabled:
        (await store.get<boolean>("quiet_audio_gate_enabled")) ?? true,
      quiet_audio_min_duration_secs:
        (await store.get<number>("quiet_audio_min_duration_secs")) ?? 0.15,
      quiet_audio_rms_dbfs_threshold:
        (await store.get<number>("quiet_audio_rms_dbfs_threshold")) ?? -60,
      quiet_audio_peak_dbfs_threshold:
        (await store.get<number>("quiet_audio_peak_dbfs_threshold")) ?? -50,
      quiet_audio_require_speech:
        (await store.get<boolean>("quiet_audio_require_speech")) ?? false,

      hot_mic_enabled: (await store.get<boolean>("hot_mic_enabled")) ?? false,
      hot_mic_pre_roll_ms:
        (await store.get<number>("hot_mic_pre_roll_ms")) ?? 1500,
      mic_auto_recover_enabled:
        (await store.get<boolean>("mic_auto_recover_enabled")) ?? false,

      noise_gate_threshold_dbfs: await(async () => {
        const configured = normalizeNoiseGateThresholdDbfs(
          await store.get("noise_gate_threshold_dbfs")
        );
        if (configured != null) return configured;

        // Legacy fallback
        const legacyStrength = normalizeNoiseGateStrength(
          await store.get("noise_gate_strength")
        );
        return noiseGateStrengthToThresholdDbfs(legacyStrength);
      })(),

      audio_downmix_to_mono:
        (await store.get<boolean>("audio_downmix_to_mono")) ?? true,
      audio_resample_to_16khz:
        (await store.get<boolean>("audio_resample_to_16khz")) ?? false,
      audio_highpass_enabled:
        (await store.get<boolean>("audio_highpass_enabled")) ?? true,
      audio_agc_enabled:
        (await store.get<boolean>("audio_agc_enabled")) ?? false,
      audio_noise_suppression_enabled:
        (await store.get<boolean>("audio_noise_suppression_enabled")) ?? false,

      max_saved_recordings: normalizeMaxSavedRecordings(
        await store.get("max_saved_recordings")
      ),

      request_logs_retention_mode: normalizeRequestLogsRetentionMode(
        await store.get("request_logs_retention_mode")
      ),
      request_logs_retention_amount: normalizeRequestLogsRetentionAmount(
        await store.get("request_logs_retention_amount")
      ),
      request_logs_retention_days: normalizeRequestLogsRetentionDays(
        await store.get("request_logs_retention_days")
      ),

      // Time retention: new (unit+value), with legacy fallback to transcription_retention_days.
      ...await(async () => {
        const rawUnit = await store.get("transcription_retention_unit");
        const rawValue = await store.get("transcription_retention_value");

        // Legacy installs only have days.
        if (rawUnit == null && rawValue == null) {
          const legacyDays = normalizeTranscriptionRetentionValue(
            await store.get("transcription_retention_days"),
            "days"
          );
          return {
            transcription_retention_unit: "days" as const,
            transcription_retention_value: legacyDays,
          };
        }

        const unit = normalizeTranscriptionRetentionUnit(rawUnit);
        const value = normalizeTranscriptionRetentionValue(rawValue, unit);
        return {
          transcription_retention_unit: unit,
          transcription_retention_value: value,
        };
      })(),
      transcription_retention_delete_recordings:
        normalizeTranscriptionRetentionDeleteRecordings(
          await store.get("transcription_retention_delete_recordings")
        ),

      // Stats retention (persisted on disk).
      ...await(async () => {
        const rawUnit = await store.get("stats_retention_unit");
        const rawValue = await store.get("stats_retention_value");

        const unit = normalizeTranscriptionRetentionUnit(rawUnit ?? "days");
        const value = normalizeTranscriptionRetentionValue(
          rawValue ?? 30,
          unit
        );

        return {
          stats_retention_unit: unit,
          stats_retention_value: value,
        };
      })(),
      stats_retention_max_bytes: normalizeStatsRetentionMaxBytes(
        await store.get("stats_retention_max_bytes")
      ),
    };

    // Mirror the accent so index.html can apply it synchronously at next launch.
    tryWriteLocalStorage(LOCAL_ACCENT_COLOR_KEY, settings.accent_color ?? null);

    return settings;
  },

  async getSystemProxyInfo(): Promise<SystemProxyInfo> {
    return invoke<SystemProxyInfo>("get_system_proxy_info");
  },

  async loadTrustedCaCertificateFromFile(
    path: string
  ): Promise<TrustedCaCertificate> {
    return invoke<TrustedCaCertificate>(
      "load_trusted_ca_certificate_from_file",
      { path }
    );
  },

  /**
   * Force the settings store to reload from disk.
   * Useful for secondary windows (overlay) when another window updates settings.json.
   */
  async reloadSettingsFromDisk(): Promise<void> {
    // @tauri-apps/plugin-store doesn't expose an instance reload API.
    // Recreate the Store instance so future reads come from disk.
    storeInstance = await Store.load("settings.json");
  },

  async updateAccentColor(color: string | null): Promise<void> {
    const store = await getStore();
    const normalized = normalizeHexColor(color);

    try {
      if (typeof window !== "undefined" && window.localStorage) {
        const LOCAL_ACCENT_COLOR_KEY = "tv_accent_color";
        if (!normalized) {
          window.localStorage.removeItem(LOCAL_ACCENT_COLOR_KEY);
        } else {
          window.localStorage.setItem(LOCAL_ACCENT_COLOR_KEY, normalized);
        }
      }
    } catch {
      // ignore
    }

    if (!normalized) {
      await store.delete("accent_color");
    } else {
      await store.set("accent_color", normalized);
    }

    await store.save();

    // Notify other windows (overlay) to refresh cached settings.
    // Include the new accent in the payload so the overlay can update immediately
    // without waiting for a disk reload.
    await emit("settings-changed", { accent_color: normalized ?? null });
  },

  async updateMainWindowCloseBehavior(
    behavior: MainWindowCloseBehavior
  ): Promise<void> {
    const store = await getStore();
    const normalized = normalizeMainWindowCloseBehavior(behavior);
    await store.set("main_window_close_behavior", normalized);
    await store.save();

    // Notify other windows (overlay) to refresh cached settings.
    await emit("settings-changed", { main_window_close_behavior: normalized });
  },

  async updateToggleHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    const store = await getStore();
    await store.set("toggle_hotkey", hotkey);
    await store.save();
  },

  async updateHoldHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    const store = await getStore();
    await store.set("hold_hotkey", hotkey);
    await store.save();
  },

  async updatePasteLastHotkey(hotkey: HotkeyConfig | null): Promise<void> {
    const store = await getStore();
    await store.set("paste_last_hotkey", hotkey);
    await store.save();
  },

  async updateSelectedMic(micId: string | null): Promise<void> {
    const store = await getStore();
    await store.set("selected_mic_id", micId);
    await store.save();

    // Notify other windows (overlay) to refresh cached settings.
    await emit("settings-changed", {});
  },

  async updateSoundEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("sound_enabled", enabled);
    await store.save();
  },

  async updateAudioCue(cue: AudioCue): Promise<void> {
    const store = await getStore();
    await store.set("audio_cue", normalizeAudioCue(cue));
    await store.save();

    // Notify other windows (overlay) to refresh cached settings.
    await emit("settings-changed", {});
  },

  async updateRewriteLlmEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("rewrite_llm_enabled", enabled);
    await store.save();
  },

  async updateCleanupPromptSections(
    sections: CleanupPromptSections | null
  ): Promise<void> {
    const store = await getStore();
    await store.set("cleanup_prompt_sections", sections);
    await store.save();
  },

  async updateRewriteProgramPromptProfiles(
    profiles: RewriteProgramPromptProfile[]
  ): Promise<void> {
    const store = await getStore();
    await store.set("rewrite_program_prompt_profiles", profiles);
    await store.save();
  },

  async listOpenWindows(): Promise<OpenWindowInfo[]> {
    return invoke("list_open_windows");
  },

  async getForegroundProcessPath(): Promise<string | null> {
    return invoke("get_foreground_process_path");
  },

  async updateSTTProvider(provider: string | null): Promise<void> {
    const store = await getStore();
    await store.set("stt_provider", provider);
    await store.save();
  },

  async updateGroqFreeTier(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("groq_free_tier", !!enabled);
    await store.save();
  },

  async updateAssemblyAiFreeTier(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("assemblyai_free_tier", !!enabled);
    await store.save();
  },

  async updateSpeechmaticsFreeTier(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("speechmatics_free_tier", !!enabled);
    await store.save();
  },

  async updateSTTModel(model: string | null): Promise<void> {
    const store = await getStore();
    await store.set("stt_model", model);
    await store.save();
  },

  async updateSTTTranscriptionPrompt(prompt: string | null): Promise<void> {
    const store = await getStore();
    await store.set("stt_transcription_prompt", prompt);
    await store.save();
  },

  async updateProxySettings(proxySettings: ProxySettings): Promise<void> {
    const store = await getStore();
    await store.set("proxy_settings", normalizeProxySettings(proxySettings));
    await store.save();
  },

  async updateLLMProvider(provider: string | null): Promise<void> {
    const store = await getStore();
    await store.set("llm_provider", provider);
    await store.save();
  },

  async updateLLMModel(model: string | null): Promise<void> {
    const store = await getStore();
    await store.set("llm_model", model);
    await store.save();
  },

  async updateOpenAiReasoningEffort(
    effort: OpenAiReasoningEffort | null
  ): Promise<void> {
    const store = await getStore();
    if (effort == null) {
      await store.delete("openai_reasoning_effort");
    } else {
      await store.set(
        "openai_reasoning_effort",
        normalizeOpenAiReasoningEffort(effort)
      );
    }
    await store.save();
  },

  async updateAnthropicThinkingBudget(budget: number | null): Promise<void> {
    const store = await getStore();
    if (budget == null) {
      await store.delete("anthropic_thinking_budget");
    } else {
      await store.set(
        "anthropic_thinking_budget",
        normalizeAnthropicThinkingBudget(budget)
      );
    }
    await store.save();
  },

  async updateGeminiThinkingBudget(budget: number | null): Promise<void> {
    const store = await getStore();
    if (budget == null) {
      await store.delete("gemini_thinking_budget");
    } else {
      await store.set(
        "gemini_thinking_budget",
        normalizeGeminiThinkingBudget(budget)
      );
    }
    await store.save();
  },

  async updateGeminiThinkingLevel(
    level: "minimal" | "low" | "medium" | "high" | null
  ): Promise<void> {
    const store = await getStore();
    if (level == null) {
      await store.delete("gemini_thinking_level");
    } else {
      await store.set(
        "gemini_thinking_level",
        normalizeGeminiThinkingLevel(level)
      );
    }
    await store.save();
  },

  async updatePlayingAudioHandling(
    handling: PlayingAudioHandling
  ): Promise<void> {
    const store = await getStore();
    await store.set("playing_audio_handling", handling);
    await store.save();
  },

  async updateSTTTimeout(timeoutSeconds: number | null): Promise<void> {
    const store = await getStore();
    await store.set("stt_timeout_seconds", timeoutSeconds);
    await store.save();
  },

  async updateOverlayMode(mode: OverlayMode): Promise<void> {
    const store = await getStore();
    await store.set("overlay_mode", mode);
    await store.save();
    // Apply the mode immediately
    await invoke("set_overlay_mode", { mode });

    // Notify other windows (overlay) to refresh cached settings.
    await emit("settings-changed", {});
  },

  async updateWidgetPosition(position: WidgetPosition): Promise<void> {
    const store = await getStore();
    await store.set("widget_position", position);
    await store.save();
    // Apply the position immediately
    await invoke("set_widget_position", { position });

    // Notify other windows (overlay) to refresh cached settings.
    await emit("settings-changed", {});
  },

  async updateOutputMode(mode: OutputMode): Promise<void> {
    const store = await getStore();
    await store.set("output_mode", mode);
    await store.save();
  },

  async updateOutputHitEnter(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("output_hit_enter", enabled);
    await store.save();
  },

  async updateQuietAudioGateEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("quiet_audio_gate_enabled", enabled);
    await store.save();
  },

  async updateQuietAudioMinDurationSecs(seconds: number): Promise<void> {
    const store = await getStore();
    await store.set("quiet_audio_min_duration_secs", seconds);
    await store.save();
  },

  async updateQuietAudioRmsDbfsThreshold(dbfs: number): Promise<void> {
    const store = await getStore();
    await store.set("quiet_audio_rms_dbfs_threshold", dbfs);
    await store.save();
  },

  async updateQuietAudioPeakDbfsThreshold(dbfs: number): Promise<void> {
    const store = await getStore();
    await store.set("quiet_audio_peak_dbfs_threshold", dbfs);
    await store.save();
  },

  async updateQuietAudioRequireSpeech(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("quiet_audio_require_speech", enabled);
    await store.save();
  },

  async updateHotMicEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("hot_mic_enabled", !!enabled);
    await store.save();
  },

  async updateHotMicPreRollMs(ms: number): Promise<void> {
    const store = await getStore();
    const normalized = Number.isFinite(ms) ? Math.max(0, Math.round(ms)) : 0;
    await store.set("hot_mic_pre_roll_ms", normalized);
    await store.save();
  },

  async updateMicAutoRecoverEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("mic_auto_recover_enabled", !!enabled);
    await store.save();
  },

  async updateNoiseGateThresholdDbfs(
    thresholdDbfs: number | null
  ): Promise<void> {
    const store = await getStore();
    const normalized = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
    await store.set("noise_gate_threshold_dbfs", normalized);
    // Best-effort legacy key for downgrade compatibility.
    await store.set(
      "noise_gate_strength",
      noiseGateThresholdDbfsToStrength(normalized)
    );
    await store.save();
  },

  async updateNoiseGateStrength(strength: number): Promise<void> {
    const store = await getStore();
    const normalizedStrength = normalizeNoiseGateStrength(strength);
    await store.set("noise_gate_strength", normalizedStrength);
    // Keep the new key in sync for newer builds.
    await store.set(
      "noise_gate_threshold_dbfs",
      noiseGateStrengthToThresholdDbfs(normalizedStrength)
    );
    await store.save();
  },

  async updateAudioDownmixToMono(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("audio_downmix_to_mono", enabled);
    await store.save();
  },

  async updateAudioResampleTo16khz(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("audio_resample_to_16khz", enabled);
    await store.save();
  },

  async updateAudioHighpassEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("audio_highpass_enabled", enabled);
    await store.save();
  },

  async updateAudioAgcEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("audio_agc_enabled", enabled);
    await store.save();
  },

  async updateAudioNoiseSuppressionEnabled(enabled: boolean): Promise<void> {
    const store = await getStore();
    await store.set("audio_noise_suppression_enabled", enabled);
    await store.save();
  },

  async updateMaxSavedRecordings(max: number): Promise<void> {
    const store = await getStore();
    await store.set("max_saved_recordings", normalizeMaxSavedRecordings(max));
    await store.save();
  },

  async updateRequestLogsRetention(params: {
    mode: RequestLogsRetentionMode;
    amount: number;
    days: number;
  }): Promise<void> {
    const store = await getStore();

    const mode = normalizeRequestLogsRetentionMode(params.mode);
    const amount = normalizeRequestLogsRetentionAmount(params.amount);
    const days = normalizeRequestLogsRetentionDays(params.days);

    await store.set("request_logs_retention_mode", mode);
    await store.set("request_logs_retention_amount", amount);
    await store.set("request_logs_retention_days", days);
    await store.save();
  },

  async updateTranscriptionRetentionDays(days: number): Promise<void> {
    const store = await getStore();
    const normalized = normalizeTranscriptionRetentionValue(days, "days");
    // Legacy key (kept for backward compatibility)
    await store.set("transcription_retention_days", normalized);
    // New keys
    await store.set("transcription_retention_unit", "days");
    await store.set("transcription_retention_value", normalized);
    await store.save();
  },

  async updateTranscriptionRetention(params: {
    unit: TranscriptionRetentionUnit;
    value: number;
  }): Promise<void> {
    const store = await getStore();
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const value = normalizeTranscriptionRetentionValue(params.value, unit);

    await store.set("transcription_retention_unit", unit);
    await store.set("transcription_retention_value", value);

    // Best-effort: keep the legacy days key in sync when unit is days.
    // (If unit is hours, we leave the legacy key untouched to avoid silently
    // changing semantics for older builds.)
    if (unit === "days") {
      await store.set("transcription_retention_days", value);
    }

    await store.save();
  },

  async updateTranscriptionRetentionDeleteRecordings(
    enabled: boolean
  ): Promise<void> {
    const store = await getStore();
    await store.set(
      "transcription_retention_delete_recordings",
      normalizeTranscriptionRetentionDeleteRecordings(enabled)
    );
    await store.save();
  },

  async updateStatsRetention(params: {
    unit: TranscriptionRetentionUnit;
    value: number;
    max_bytes?: number;
  }): Promise<void> {
    const store = await getStore();
    const unit = normalizeTranscriptionRetentionUnit(params.unit);
    const value = normalizeTranscriptionRetentionValue(params.value, unit);

    await store.set("stats_retention_unit", unit);
    await store.set("stats_retention_value", value);

    if (typeof params.max_bytes === "number") {
      await store.set(
        "stats_retention_max_bytes",
        normalizeStatsRetentionMaxBytes(params.max_bytes)
      );
    }

    await store.save();
  },

  async isAudioMuteSupported(): Promise<boolean> {
    return invoke("is_audio_mute_supported");
  },

  // API Key management
  async hasApiKey(storeKey: string): Promise<boolean> {
    const store = await getStore();
    const value = await store.get<string>(storeKey);
    return value !== null && value !== undefined && value.length > 0;
  },

  async getApiKey(storeKey: string): Promise<string | null> {
    const store = await getStore();
    const value = await store.get<string>(storeKey);
    return value ?? null;
  },

  async setApiKey(storeKey: string, apiKey: string): Promise<void> {
    const store = await getStore();
    await store.set(storeKey, apiKey);
    await store.save();
  },

  async clearApiKey(storeKey: string): Promise<void> {
    const store = await getStore();
    await store.delete(storeKey);
    await store.save();
  },

  // Onboarding / guide state
  async getSettingsGuideState(): Promise<SettingsGuideState> {
    const store = await getStore();
    const raw = await store.get(SETTINGS_GUIDE_STATE_KEY);
    const state = normalizeSettingsGuideState(raw);

    try {
      if (typeof window !== "undefined" && window.localStorage) {
        window.localStorage.setItem("tv_settings_guide_state", state);
      }
    } catch {
      // ignore
    }

    return state;
  },

  async setSettingsGuideState(state: SettingsGuideState): Promise<void> {
    const store = await getStore();
    await store.set(
      SETTINGS_GUIDE_STATE_KEY,
      normalizeSettingsGuideState(state)
    );
    await store.save();

    try {
      if (typeof window !== "undefined" && window.localStorage) {
        window.localStorage.setItem("tv_settings_guide_state", state);
      }
    } catch {
      // ignore
    }

    // Notify other windows that persisted state changed.
    await emit("settings-changed", { [SETTINGS_GUIDE_STATE_KEY]: state });
  },

  async resetHotkeysToDefaults(): Promise<void> {
    const store = await getStore();
    await store.set("toggle_hotkey", defaultToggleHotkey);
    await store.set("hold_hotkey", defaultHoldHotkey);
    await store.set("paste_last_hotkey", defaultPasteLastHotkey);
    await store.save();
  },

  async registerShortcuts(): Promise<void> {
    return invoke("register_shortcuts");
  },

  async unregisterShortcuts(): Promise<void> {
    return invoke("unregister_shortcuts");
  },

  // History API
  async addHistoryEntry(text: string): Promise<HistoryEntry> {
    return invoke("add_history_entry", { text });
  },

  async getHistory(limit?: number): Promise<HistoryEntry[]> {
    return invoke("get_history", { limit });
  },

  async getHistoryPage(params: HistoryPageQuery): Promise<HistoryPageResult> {
    return invoke("get_history_page", { params });
  },

  async deleteHistoryEntry(id: string): Promise<boolean> {
    return invoke("delete_history_entry", { id });
  },

  async getHistoryDeleteOptions(id: string): Promise<HistoryDeleteOptions> {
    return invoke("get_history_delete_options", { id });
  },

  async deleteHistoryEntryEx(
    id: string,
    mode: HistoryDeleteMode
  ): Promise<HistoryDeleteResult> {
    return invoke("delete_history_entry_ex", { id, mode });
  },

  async clearHistory(): Promise<void> {
    return invoke("clear_history");
  },

  // Overlay API
  async resizeOverlay(width: number, height: number): Promise<void> {
    return invoke("resize_overlay", { width, height });
  },

  async startDragging(): Promise<void> {
    const window = getCurrentWindow();
    return window.startDragging();
  },

  // Connection state sync between windows
  async emitConnectionState(state: ConnectionState): Promise<void> {
    return emit("connection-state-changed", { state });
  },

  async onConnectionStateChanged(
    callback: (state: ConnectionState) => void
  ): Promise<UnlistenFn> {
    return listen<{ state: ConnectionState }>(
      "connection-state-changed",
      (event) => {
        callback(event.payload.state);
      }
    );
  },

  // History sync between windows
  async emitHistoryChanged(): Promise<void> {
    return emit("history-changed", {});
  },

  async onHistoryChanged(callback: () => void): Promise<UnlistenFn> {
    return listen("history-changed", () => {
      callback();
    });
  },

  async onStatsChanged(callback: () => void): Promise<UnlistenFn> {
    return listen("stats-changed", () => {
      callback();
    });
  },

  // Settings sync between windows (main -> overlay)
  async emitSettingsChanged(
    payload: Record<string, unknown> = {}
  ): Promise<void> {
    return emit("settings-changed", payload);
  },

  async onSettingsChanged(
    callback: (payload: unknown) => void
  ): Promise<UnlistenFn> {
    return listen("settings-changed", (event) => {
      callback(event.payload);
    });
  },
};

export interface OpenWindowInfo {
  title: string;
  process_path: string;
}

// ============================================================================
// LLM API
// ============================================================================

export interface TestLlmRewriteResponse {
  output: string;
  provider_used: string;
  model_used: string;
}

export interface LlmProviderInfo {
  id: string;
  name: string;
  requires_api_key: boolean;
  default_model: string;
  models: string[];
}

export interface LlmCompleteResponse {
  output: string;
  provider_used: string;
  model_used: string;
}

export const llmAPI = {
  getLlmProviders: () => invoke<LlmProviderInfo[]>("get_llm_providers"),

  testLlmRewrite: (params: { transcript: string; profileId?: string | null }) =>
    invoke<TestLlmRewriteResponse>("test_llm_rewrite", {
      transcript: params.transcript,
      // IMPORTANT: Tauri command arg mapping uses camelCase in JS.
      // Rust signature uses `profile_id`, which Tauri maps from `profileId`.
      profileId: params.profileId ?? null,
    }),

  complete: (params: {
    provider: string;
    model?: string | null;
    systemPrompt: string;
    userPrompt: string;
  }) =>
    invoke<LlmCompleteResponse>("llm_complete", {
      // Rust signature: llm_complete(pipeline, args: LlmCompleteArgs)
      args: {
        provider: params.provider,
        model: params.model ?? null,

        // The backend accepts both camelCase and snake_case via serde aliases.
        systemPrompt: params.systemPrompt,
        userPrompt: params.userPrompt,
      },
    }),
};

export const sttAPI = {
  testTranscribeLastAudio: (params: { profileId?: string | null }) =>
    invoke<string>("pipeline_test_transcribe_last_audio", {
      // See note above: Rust uses `profile_id`, JS should pass `profileId`.
      profileId: params.profileId ?? null,
    }),

  hasLastAudio: () => invoke<boolean>("pipeline_has_last_audio"),

  getLastRecordingDiagnostics: () =>
    invoke<AudioCaptureDiagnostics | null>(
      "pipeline_get_last_recording_diagnostics"
    ),

  // Retry a previous request using its persisted audio.
  // Returns the final text (STT + optional LLM), same as normal transcription.
  retryTranscription: (params: { requestId: string }) =>
    invoke<string>("pipeline_retry_transcription", {
      requestId: params.requestId,
    }),
};

export interface AudioLevelStats {
  duration_secs: number;
  rms: number;
  peak: number;
}

export interface AudioCaptureDiagnostics {
  stats: AudioLevelStats;
  // null when speech detection wasn't computed for the last recording.
  speech_detected: boolean | null;
}

export interface AudioSettingsTestWavs {
  raw_wav_base64: string;
  processed_wav_base64: string;
}

export const audioSettingsTestAPI = {
  startRecording: () =>
    invoke<void>("pipeline_test_audio_settings_start_recording"),
  stopRecording: () =>
    invoke<AudioSettingsTestWavs>(
      "pipeline_test_audio_settings_stop_recording"
    ),
};

// ============================================================================
// Config API - Using Tauri commands
// ============================================================================

export interface DefaultSectionsResponse {
  main: string;
  advanced: string;
  dictionary: string;
}

interface ProviderInfo {
  value: string;
  label: string;
  is_local: boolean;
}

interface AvailableProvidersResponse {
  stt: ProviderInfo[];
  llm: ProviderInfo[];
}

export const configAPI = {
  // Default prompt sections (from Tauri)
  getDefaultSections: () =>
    invoke<DefaultSectionsResponse>("get_default_sections"),

  // Available providers (from Tauri, based on configured API keys)
  getAvailableProviders: () =>
    invoke<AvailableProvidersResponse>("get_available_providers"),

  // Sync pipeline config when settings change
  syncPipelineConfig: () => invoke<void>("sync_pipeline_config"),
};

// ============================================================================
// Request Logs API
// ============================================================================

export type LogLevel = "debug" | "info" | "warn" | "error";
export type RequestStatus = "in_progress" | "success" | "error" | "cancelled";

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  message: string;
  details: string | null;
}

export interface RequestLog {
  id: string;
  started_at: string;
  ended_at: string | null;
  stt_provider: string;
  stt_model: string | null;
  llm_provider: string | null;
  llm_model: string | null;

  profile_id?: string | null;
  profile_name?: string | null;

  raw_transcript: string | null;
  final_text: string | null;
  // Total request processing duration (ms). Excludes recording time when available.
  total_duration_ms: number | null;
  stt_duration_ms: number | null;
  llm_duration_ms: number | null;
  status: RequestStatus;
  error_message: string | null;
  entries: LogEntry[];

  stt_is_free_tier: boolean;
  llm_is_free_tier: boolean;
  stt_estimated_cost_usd_micros: number | null;
  llm_estimated_cost_usd_micros: number | null;

  // Optional provider payloads for debugging.
  // Binary audio is redacted and represented with placeholders.
  stt_request_json?: unknown;
  stt_response_json?: unknown;
  llm_request_json?: unknown;
  llm_response_json?: unknown;
}

export interface RecordingsStats {
  count: number;
  bytes: number;
}

export interface DataStorageSummary {
  recordings_count: number;
  recordings_bytes: number;
  history_count: number;
  history_bytes: number;
  request_logs_count: number;
  stats_files_count: number;
  stats_bytes: number;
  settings_bytes: number;
  api_keys_set_count: number;
}

export const logsAPI = {
  getRequestLogs: (limit?: number) =>
    invoke<RequestLog[]>("get_request_logs", { limit: limit ?? 50 }),

  clearRequestLogs: () => invoke<void>("clear_request_logs"),
};

// ============================================================================
// Data / Danger Zone
// ============================================================================

export const dataAPI = {
  getStorageSummary: () =>
    invoke<DataStorageSummary>("get_data_storage_summary"),

  deleteAllRecordings: () => invoke<number>("recordings_delete_all"),

  deleteAllApiKeys: () => invoke<void>("delete_all_api_keys"),

  deleteAllSettings: () => invoke<void>("delete_all_settings"),

  deleteAllStats: () => invoke<void>("delete_all_stats"),

  deleteAllData: () => invoke<void>("delete_all_data"),
};

// ============================================================================
// Recordings API (playback)
// ============================================================================

export const recordingsAPI = {
  // Returns a URL usable as an <audio src>, or null if no recording exists.
  getRecordingAssetUrl: async (params: { requestId: string }) => {
    const path = await invoke<string | null>("recording_get_wav_path", {
      requestId: params.requestId,
    });
    return path ? convertFileSrc(path) : null;
  },

  // Returns base64 WAV bytes, or null if no recording exists.
  getRecordingWavBase64: (params: { requestId: string }) =>
    invoke<string | null>("recording_get_wav_base64", {
      requestId: params.requestId,
    }),

  // Open recordings directory in file explorer.
  openRecordingsFolder: () => invoke<void>("recordings_open_folder"),

  // Total size (bytes) used by saved recordings.
  getRecordingsStorageBytes: () =>
    invoke<number>("recordings_get_storage_bytes"),

  // Stats for UI display (count + bytes).
  getRecordingsStats: () => invoke<RecordingsStats>("recordings_get_stats"),
};
