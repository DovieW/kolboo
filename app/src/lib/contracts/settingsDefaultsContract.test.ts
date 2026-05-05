import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS_VALUES } from "../tauri/settingsDefaults";

const rustDefaultsPath = path.resolve(
	process.cwd(),
	"src-tauri/src/settings/default_values.rs",
);

function readRustDefaultConst(name: string): string {
	const source = readFileSync(rustDefaultsPath, "utf8");
	const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	const match = source.match(
		new RegExp(`pub const ${escaped}: [^=]+ = (?<value>[^;]+);`),
	);
	if (!match?.groups?.value) {
		throw new Error(`Missing Rust default const ${name}`);
	}
	return match.groups.value.trim();
}

function parseRustStringConst(name: string): string {
	const value = readRustDefaultConst(name);
	const match = value.match(/^"(?<inner>.*)"$/);
	if (!match?.groups?.inner) {
		throw new Error(`Rust default const ${name} is not a string literal`);
	}
	return match.groups.inner;
}

function parseRustNumberConst(name: string): number {
	const value = readRustDefaultConst(name).replace(/_u?(?:32|64|size)?$/, "");
	return Number(value.replaceAll("_", ""));
}

function parseRustBooleanConst(name: string): boolean {
	const value = readRustDefaultConst(name);
	if (value === "true") return true;
	if (value === "false") return false;
	throw new Error(`Rust default const ${name} is not a boolean literal`);
}

describe("settings defaults cross-layer contract", () => {
	it("keeps shared string defaults aligned between TypeScript and Rust", () => {
		const stringDefaults = [
			["DEFAULT_STT_PROVIDER", DEFAULT_SETTINGS_VALUES.stt_provider],
			["DEFAULT_STT_LANGUAGE", DEFAULT_SETTINGS_VALUES.stt_language],
			[
				"DEFAULT_LOCAL_WHISPER_LOAD_MODE",
				DEFAULT_SETTINGS_VALUES.local_whisper_load_mode,
			],
			["DEFAULT_OVERLAY_MODE", DEFAULT_SETTINGS_VALUES.overlay_mode],
			[
				"DEFAULT_OVERLAY_MONITOR_TARGET",
				DEFAULT_SETTINGS_VALUES.overlay_monitor_target,
			],
			["DEFAULT_WIDGET_POSITION", DEFAULT_SETTINGS_VALUES.widget_position],
			["DEFAULT_OUTPUT_MODE", DEFAULT_SETTINGS_VALUES.output_mode],
			[
				"DEFAULT_MAIN_WINDOW_CLOSE_BEHAVIOR",
				DEFAULT_SETTINGS_VALUES.main_window_close_behavior,
			],
			[
				"DEFAULT_PLAYING_AUDIO_HANDLING",
				DEFAULT_SETTINGS_VALUES.playing_audio_handling,
			],
			[
				"DEFAULT_REQUEST_LOGS_RETENTION_MODE",
				DEFAULT_SETTINGS_VALUES.request_logs_retention_mode,
			],
			[
				"DEFAULT_TRANSCRIPTION_RETENTION_MODE",
				DEFAULT_SETTINGS_VALUES.transcription_retention_mode,
			],
			[
				"DEFAULT_TRANSCRIPTION_RETENTION_UNIT",
				DEFAULT_SETTINGS_VALUES.transcription_retention_unit,
			],
			[
				"DEFAULT_RECORDINGS_RETENTION_MODE",
				DEFAULT_SETTINGS_VALUES.recordings_retention_mode,
			],
			[
				"DEFAULT_RECORDINGS_RETENTION_UNIT",
				DEFAULT_SETTINGS_VALUES.recordings_retention_unit,
			],
			[
				"DEFAULT_STATS_RETENTION_UNIT",
				DEFAULT_SETTINGS_VALUES.stats_retention_unit,
			],
			[
				"DEFAULT_QUICK_ASK_DISMISS_MODE",
				DEFAULT_SETTINGS_VALUES.quick_ask_dismiss_mode,
			],
			["DEFAULT_OCR_MODEL", DEFAULT_SETTINGS_VALUES.ocr_model],
			["DEFAULT_OCR_AUTH_MODE", DEFAULT_SETTINGS_VALUES.ocr_auth_mode],
			[
				"DEFAULT_OCR_AUTO_CAPTURE_TIMING",
				DEFAULT_SETTINGS_VALUES.ocr_auto_capture_timing,
			],
			["DEFAULT_OCR_RESIZE_FILTER", DEFAULT_SETTINGS_VALUES.ocr_resize_filter],
			[
				"DEFAULT_ACTIVE_WINDOW_OCR_MODE",
				DEFAULT_SETTINGS_VALUES.rewrite_active_window_ocr_mode,
			],
			[
				"DEFAULT_ACTIVE_WINDOW_OCR_MODE",
				DEFAULT_SETTINGS_VALUES.quick_replace_active_window_ocr_mode,
			],
			[
				"DEFAULT_ACTIVE_WINDOW_OCR_MODE",
				DEFAULT_SETTINGS_VALUES.quick_ask_active_window_ocr_mode,
			],
		] as const;

		for (const [rustConst, tsValue] of stringDefaults) {
			// This table is deliberately broad: the Settings View defaults are an
			// interface contract, so adding a Rust runtime default without updating the
			// TypeScript effective default should fail loudly here.
			expect(parseRustStringConst(rustConst)).toBe(tsValue);
		}
	});

	it("keeps shared numeric and boolean defaults aligned between TypeScript and Rust", () => {
		const numberDefaults = [
			[
				"DEFAULT_MAX_SAVED_RECORDINGS",
				DEFAULT_SETTINGS_VALUES.max_saved_recordings,
			],
			[
				"DEFAULT_REQUEST_LOGS_RETENTION_AMOUNT",
				DEFAULT_SETTINGS_VALUES.request_logs_retention_amount,
			],
			[
				"DEFAULT_REQUEST_LOGS_RETENTION_DAYS",
				DEFAULT_SETTINGS_VALUES.request_logs_retention_days,
			],
			[
				"DEFAULT_TRANSCRIPTION_RETENTION_AMOUNT",
				DEFAULT_SETTINGS_VALUES.transcription_retention_amount,
			],
			[
				"DEFAULT_TRANSCRIPTION_RETENTION_VALUE",
				DEFAULT_SETTINGS_VALUES.transcription_retention_value,
			],
			[
				"DEFAULT_RECORDINGS_RETENTION_AMOUNT",
				DEFAULT_SETTINGS_VALUES.recordings_retention_amount,
			],
			[
				"DEFAULT_RECORDINGS_RETENTION_VALUE",
				DEFAULT_SETTINGS_VALUES.recordings_retention_value,
			],
			[
				"DEFAULT_STATS_RETENTION_VALUE",
				DEFAULT_SETTINGS_VALUES.stats_retention_value,
			],
			[
				"DEFAULT_STATS_RETENTION_MAX_BYTES",
				DEFAULT_SETTINGS_VALUES.stats_retention_max_bytes,
			],
			[
				"DEFAULT_OCR_REQUEST_TIMEOUT_MS",
				DEFAULT_SETTINGS_VALUES.ocr_request_timeout_ms,
			],
			[
				"DEFAULT_OCR_CONTEXT_MAX_CHARS",
				DEFAULT_SETTINGS_VALUES.ocr_context_max_chars,
			],
			[
				"DEFAULT_OCR_HALLUCINATION_THRESHOLD",
				DEFAULT_SETTINGS_VALUES.ocr_hallucination_threshold,
			],
			[
				"DEFAULT_OCR_RESIZE_MAX_DIMENSION",
				DEFAULT_SETTINGS_VALUES.ocr_resize_max_dimension,
			],
			[
				"DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_COUNT",
				DEFAULT_SETTINGS_VALUES.quick_ask_conversation_history_count,
			],
			[
				"DEFAULT_HOT_MIC_PRE_ROLL_MS",
				DEFAULT_SETTINGS_VALUES.hot_mic_pre_roll_ms,
			],
			[
				"DEFAULT_QUIET_AUDIO_MIN_DURATION_SECS",
				DEFAULT_SETTINGS_VALUES.quiet_audio_min_duration_secs,
			],
			[
				"DEFAULT_QUIET_AUDIO_RMS_DBFS_THRESHOLD",
				DEFAULT_SETTINGS_VALUES.quiet_audio_rms_dbfs_threshold,
			],
			[
				"DEFAULT_QUIET_AUDIO_PEAK_DBFS_THRESHOLD",
				DEFAULT_SETTINGS_VALUES.quiet_audio_peak_dbfs_threshold,
			],
		] as const;
		const booleanDefaults = [
			[
				"DEFAULT_HOTKEY_DEBUG_ENABLED",
				DEFAULT_SETTINGS_VALUES.hotkey_debug_enabled,
			],
			["DEFAULT_STT_LIVE_OUTPUT", DEFAULT_SETTINGS_VALUES.stt_live_output],
			[
				"DEFAULT_STT_SIMULATED_STREAMING",
				DEFAULT_SETTINGS_VALUES.stt_simulated_streaming,
			],
			["DEFAULT_SOUND_ENABLED", DEFAULT_SETTINGS_VALUES.sound_enabled],
			[
				"DEFAULT_OVERLAY_SHOW_DETAILED_LOADING",
				DEFAULT_SETTINGS_VALUES.overlay_show_detailed_loading,
			],
			["DEFAULT_OUTPUT_HIT_ENTER", DEFAULT_SETTINGS_VALUES.output_hit_enter],
			[
				"DEFAULT_OUTPUT_CLIPBOARD_PRIVACY_MODE",
				DEFAULT_SETTINGS_VALUES.output_clipboard_privacy_mode,
			],
			[
				"DEFAULT_OUTPUT_SMART_PASTE_PROTECTION",
				DEFAULT_SETTINGS_VALUES.output_smart_paste_protection,
			],
			[
				"DEFAULT_REWRITE_LLM_ENABLED",
				DEFAULT_SETTINGS_VALUES.rewrite_llm_enabled,
			],
			[
				"DEFAULT_QUICK_REPLACE_ENABLED",
				DEFAULT_SETTINGS_VALUES.quick_replace_enabled,
			],
			[
				"DEFAULT_REQUEST_LOGS_PRIVACY_MODE",
				DEFAULT_SETTINGS_VALUES.request_logs_privacy_mode,
			],
			[
				"DEFAULT_TRANSCRIPTION_RETENTION_DELETE_RECORDINGS",
				DEFAULT_SETTINGS_VALUES.transcription_retention_delete_recordings,
			],
			[
				"DEFAULT_QUICK_ASK_CONVERSATION_HISTORY_ENABLED",
				DEFAULT_SETTINGS_VALUES.quick_ask_conversation_history_enabled,
			],
			[
				"DEFAULT_QUICK_ASK_INCLUDE_SELECTED_TEXT",
				DEFAULT_SETTINGS_VALUES.quick_ask_include_selected_text,
			],
			[
				"DEFAULT_OCR_HALLUCINATION_PROTECTION",
				DEFAULT_SETTINGS_VALUES.ocr_hallucination_protection,
			],
			["DEFAULT_HOT_MIC_ENABLED", DEFAULT_SETTINGS_VALUES.hot_mic_enabled],
			[
				"DEFAULT_MIC_AUTO_RECOVER_ENABLED",
				DEFAULT_SETTINGS_VALUES.mic_auto_recover_enabled,
			],
			[
				"DEFAULT_QUIET_AUDIO_GATE_ENABLED",
				DEFAULT_SETTINGS_VALUES.quiet_audio_gate_enabled,
			],
			[
				"DEFAULT_QUIET_AUDIO_REQUIRE_SPEECH",
				DEFAULT_SETTINGS_VALUES.quiet_audio_require_speech,
			],
			[
				"DEFAULT_AUDIO_DOWNMIX_TO_MONO",
				DEFAULT_SETTINGS_VALUES.audio_downmix_to_mono,
			],
			[
				"DEFAULT_AUDIO_RESAMPLE_TO_16KHZ",
				DEFAULT_SETTINGS_VALUES.audio_resample_to_16khz,
			],
			[
				"DEFAULT_AUDIO_HIGHPASS_ENABLED",
				DEFAULT_SETTINGS_VALUES.audio_highpass_enabled,
			],
			["DEFAULT_AUDIO_AGC_ENABLED", DEFAULT_SETTINGS_VALUES.audio_agc_enabled],
			[
				"DEFAULT_AUDIO_NOISE_SUPPRESSION_ENABLED",
				DEFAULT_SETTINGS_VALUES.audio_noise_suppression_enabled,
			],
		] as const;

		for (const [rustConst, tsValue] of numberDefaults) {
			expect(parseRustNumberConst(rustConst)).toBe(tsValue);
		}

		for (const [rustConst, tsValue] of booleanDefaults) {
			expect(parseRustBooleanConst(rustConst)).toBe(tsValue);
		}
	});
});
