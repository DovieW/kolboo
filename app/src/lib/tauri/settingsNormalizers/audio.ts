import { DEFAULT_SETTINGS_VALUES } from "../settingsDefaults";
import type { AudioCue, PlayingAudioHandling } from "../types";

export function normalizePlayingAudioHandling(
	value: unknown,
): PlayingAudioHandling {
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

	return DEFAULT_SETTINGS_VALUES.playing_audio_handling;
}

export function normalizeAudioCue(value: unknown): AudioCue {
	if (
		value === "kolboo" ||
		value === "maraca" ||
		value === "clave" ||
		value === "legacy"
	) {
		return value;
	}

	return DEFAULT_SETTINGS_VALUES.audio_cue;
}

export function normalizeNoiseGateStrength(value: unknown): number {
	if (typeof value !== "number" || !Number.isFinite(value)) return 0;
	const rounded = Math.round(value);
	return Math.min(100, Math.max(0, rounded));
}

export function normalizeNoiseGateThresholdDbfs(value: unknown): number | null {
	if (value == null) return null;
	if (typeof value !== "number" || !Number.isFinite(value)) return null;
	// Clamp to the UI range.
	return Math.min(-30, Math.max(-75, value));
}

export function noiseGateStrengthToThresholdDbfs(
	strength: number,
): number | null {
	const s = normalizeNoiseGateStrength(strength);
	if (s <= 0) return null;
	// Map 1..100 => -75..-30 (same range as the Rust mapping).
	const t = -75 + (s / 100) * 45;
	return Math.min(-30, Math.max(-75, t));
}

export function noiseGateThresholdDbfsToStrength(
	thresholdDbfs: number | null,
): number {
	if (thresholdDbfs == null) return 0;
	const t = normalizeNoiseGateThresholdDbfs(thresholdDbfs);
	if (t == null) return 0;
	const s = ((t + 75) / 45) * 100;
	// Never return 0 when enabled; old UI treated 0 as off.
	return Math.min(100, Math.max(1, Math.round(s)));
}
