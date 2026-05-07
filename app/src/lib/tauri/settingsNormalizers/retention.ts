import { DEFAULT_SETTINGS_VALUES } from "../settingsDefaults";
import type {
	RequestLogsRetentionMode,
	TranscriptionRetentionUnit,
} from "../types";

/**
 * Raw persisted-settings normalization for retention and storage limits.
 *
 * This module intentionally owns only settings-edge coercion/clamping/legacy
 * fallback semantics for retention-related fields in `settings.json`.
 *
 * It does NOT own:
 * - source-aware fallback semantics (`settingsViews.ts`)
 * - frontend display/read-model logic (`settings/dataLifecycle.ts`)
 * - backend pruning behavior (`src-tauri/src/sessions/retention.rs`)
 */

export function normalizeMaxSavedRecordings(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.max_saved_recordings;
	}
	const rounded = Math.round(value);
	// 1..100000 (defensive)
	return Math.min(100000, Math.max(1, rounded));
}

export function normalizeTranscriptionRetentionUnit(
	value: unknown,
): TranscriptionRetentionUnit {
	if (value === "days" || value === "hours") return value;
	return DEFAULT_SETTINGS_VALUES.transcription_retention_unit;
}

export function normalizeTranscriptionRetentionValue(
	value: unknown,
	unit: TranscriptionRetentionUnit,
): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.transcription_retention_value;
	}
	const clamped = Math.max(0, value);

	if (unit === "days") {
		const rounded = Math.round(clamped);
		// 0..36500 days (~100 years) defensive cap
		return Math.min(36500, Math.max(0, rounded));
	}

	// Hours keep fractional values (for example 0.5) because the UI and backend
	// both support sub-day retention without inventing a second persisted field.
	const maxHours = 36500 * 24;
	return Math.min(maxHours, clamped);
}

export function normalizeTranscriptionRetentionDeleteRecordings(
	value: unknown,
): boolean {
	return typeof value === "boolean"
		? value
		: DEFAULT_SETTINGS_VALUES.transcription_retention_delete_recordings;
}

export function normalizeStatsRetentionMaxBytes(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.stats_retention_max_bytes;
	}
	const rounded = Math.round(value);
	// 1MB..5GB (defensive)
	return Math.min(5_000_000_000, Math.max(1_000_000, rounded));
}

export function normalizeRequestLogsRetentionMode(
	value: unknown,
): RequestLogsRetentionMode {
	return value === "time" || value === "amount"
		? value
		: DEFAULT_SETTINGS_VALUES.request_logs_retention_mode;
}

export function normalizeRequestLogsRetentionAmount(value: unknown): number {
	// Keep this modest to avoid runaway memory in the backend.
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.request_logs_retention_amount;
	}
	const rounded = Math.round(value);
	// 1..1000 defensive
	return Math.min(1000, Math.max(1, rounded));
}

export function normalizeRequestLogsRetentionDays(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.request_logs_retention_days;
	}
	const rounded = Math.round(value);
	// 0..36500 (~100 years) defensive
	return Math.min(36500, Math.max(0, rounded));
}

export function normalizeRetentionMode(
	value: unknown,
	fallback: RequestLogsRetentionMode = DEFAULT_SETTINGS_VALUES.request_logs_retention_mode,
): RequestLogsRetentionMode {
	return value === "time" || value === "amount" ? value : fallback;
}

export function normalizeTranscriptionRetentionAmount(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_SETTINGS_VALUES.transcription_retention_amount;
	}
	const rounded = Math.round(value);
	// 1..100000 (defensive)
	return Math.min(100000, Math.max(1, rounded));
}
