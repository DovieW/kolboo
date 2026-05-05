import {
	DEFAULT_SETTINGS_VALUES,
	type DefaultSettingsKey,
} from "./settingsDefaults";
import type { RewritePreset, RewriteProgramPromptProfile } from "./types";

export type SettingsValueSource =
	| "stored"
	| "global"
	| "profile"
	| "preset"
	| "policy"
	| "default";

export type SettingsValueView<T> = {
	value: T;
	source: SettingsValueSource;
	explicitNull: boolean;
};

type UnknownRecord = Record<string, unknown>;

function hasOwnValue(record: unknown, key: string): record is UnknownRecord {
	return (
		record != null && typeof record === "object" && Object.hasOwn(record, key)
	);
}

export function isExplicitNull(record: unknown, key: string): boolean {
	return hasOwnValue(record, key) && record[key] === null;
}

export function isMissing(record: unknown, key: string): boolean {
	return !hasOwnValue(record, key) || record[key] === undefined;
}

export function isInheritedSettingValue(record: unknown, key: string): boolean {
	// Settings UI treats both missing and explicit null profile values as
	// intentional inheritance from the Default/global setting. Keep this near the
	// Settings View helpers so production components do not re-implement the
	// same null-vs-missing rule in slightly different ways.
	return isMissing(record, key) || isExplicitNull(record, key);
}

function normalizeOrNull<T>(
	value: unknown,
	normalize?: (value: unknown) => T | null,
): T | null {
	if (!normalize) return value as T;
	return normalize(value);
}

export function defaultSettingsView<K extends DefaultSettingsKey>(
	key: K,
): SettingsValueView<(typeof DEFAULT_SETTINGS_VALUES)[K]> {
	return {
		value: DEFAULT_SETTINGS_VALUES[key],
		source: "default",
		explicitNull: false,
	};
}

export function settingValueView<T>(params: {
	record: unknown;
	key: string;
	defaultValue: T;
	normalize?: (value: unknown) => T | null;
	source?: SettingsValueSource;
}): SettingsValueView<T> {
	const explicitNull = isExplicitNull(params.record, params.key);
	if (!hasOwnValue(params.record, params.key) || explicitNull) {
		return {
			value: params.defaultValue,
			source: "default",
			explicitNull,
		};
	}

	const normalized = normalizeOrNull(
		params.record[params.key],
		params.normalize,
	);
	if (normalized == null) {
		return {
			value: params.defaultValue,
			source: "default",
			explicitNull: false,
		};
	}

	return {
		value: normalized,
		source: params.source ?? "stored",
		explicitNull: false,
	};
}

export function inheritedSettingView<T>(params: {
	globalValue: T | null | undefined;
	profile?: RewriteProgramPromptProfile | null;
	key: keyof RewriteProgramPromptProfile;
	defaultValue: T;
	normalize?: (value: unknown) => T | null;
}): SettingsValueView<T> {
	const profile = params.profile;
	if (profile && hasOwnValue(profile, params.key)) {
		if (profile[params.key] === null) {
			return resolveGlobalOrDefault(
				params.globalValue,
				params.defaultValue,
				true,
			);
		}

		const normalized = normalizeOrNull(profile[params.key], params.normalize);
		if (normalized != null) {
			return {
				value: normalized,
				source: "profile",
				explicitNull: false,
			};
		}
	}

	return resolveGlobalOrDefault(params.globalValue, params.defaultValue, false);
}

export function presetSettingView<T>(params: {
	globalValue: T | null | undefined;
	profile?: RewriteProgramPromptProfile | null;
	preset?: RewritePreset | null;
	key: keyof RewritePreset & keyof RewriteProgramPromptProfile;
	defaultValue: T;
	normalize?: (value: unknown) => T | null;
}): SettingsValueView<T> {
	const preset = params.preset;
	if (preset && hasOwnValue(preset, params.key)) {
		if (preset[params.key] !== null) {
			const normalized = normalizeOrNull(preset[params.key], params.normalize);
			if (normalized != null) {
				return {
					value: normalized,
					source: "preset",
					explicitNull: false,
				};
			}
		}

		if (preset[params.key] === null) {
			const inherited = inheritedSettingView({
				globalValue: params.globalValue,
				profile: params.profile,
				key: params.key,
				defaultValue: params.defaultValue,
				normalize: params.normalize,
			});

			// A preset-level explicit null means “inherit from profile/global/default”.
			// Preserve that provenance so the Settings View interface can explain the
			// difference between “missing” and “intentionally inherited”.
			return { ...inherited, explicitNull: true };
		}
	}

	return inheritedSettingView({
		globalValue: params.globalValue,
		profile: params.profile,
		key: params.key,
		defaultValue: params.defaultValue,
		normalize: params.normalize,
	});
}

export function findProfileById(
	profiles: readonly RewriteProgramPromptProfile[],
	profileId: string | null | undefined,
): RewriteProgramPromptProfile | null {
	if (!profileId) return null;
	return profiles.find((profile) => profile.id === profileId) ?? null;
}

export function findPresetById(
	profile: RewriteProgramPromptProfile | null | undefined,
	presetId: string | null | undefined,
): RewritePreset | null {
	if (!profile || !presetId) return null;
	return profile.presets?.find((preset) => preset.id === presetId) ?? null;
}

function resolveGlobalOrDefault<T>(
	globalValue: T | null | undefined,
	defaultValue: T,
	explicitNull: boolean,
): SettingsValueView<T> {
	if (globalValue != null) {
		return {
			value: globalValue,
			source: "global",
			explicitNull,
		};
	}

	return {
		value: defaultValue,
		source: "default",
		explicitNull,
	};
}
