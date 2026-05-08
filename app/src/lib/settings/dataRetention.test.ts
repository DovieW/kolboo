import { describe, expect, it, vi } from "vitest";
import {
	buildLogsRetentionControls,
	buildTranscriptionRetentionControls,
	commitLogsRetention,
	commitRecordingsRetention,
	commitStatsRetention,
	type DataRetentionDependencies,
	type DataRetentionEffects,
	readDataRetentionSources,
	shouldResetDataRetentionDrafts,
} from "./dataRetention";

function createDeps(
	overrides: Partial<DataRetentionDependencies> = {},
): DataRetentionDependencies {
	return {
		updateRequestLogsRetention: vi.fn(async () => undefined),
		updateRecordingsRetention: vi.fn(async () => undefined),
		updateMaxSavedRecordings: vi.fn(async () => undefined),
		updateTranscriptionRetentionPolicy: vi.fn(async () => undefined),
		updateTranscriptionRetentionDeleteRecordings: vi.fn(async () => undefined),
		updateStatsRetention: vi.fn(async () => undefined),
		...overrides,
	};
}

function createEffects(
	overrides: Partial<DataRetentionEffects> = {},
): DataRetentionEffects {
	return {
		onSettingsChanged: vi.fn(async () => undefined),
		onRequestLogsChanged: vi.fn(async () => undefined),
		onRecordingsChanged: vi.fn(async () => undefined),
		...overrides,
	};
}

describe("Data retention orchestration helpers", () => {
	it("reads retention sources with defensive defaults", () => {
		expect(readDataRetentionSources(undefined)).toEqual({
			logs: {
				mode: "amount",
				amount: 10,
				unit: "days",
				value: 7,
			},
			recordings: {
				mode: "amount",
				amount: 50,
				unit: "days",
				value: 0,
			},
			transcription: {
				mode: "time",
				amount: 1000,
				unit: "days",
				value: 0,
			},
			transcriptionDeleteRecordings: false,
			stats: {
				unit: "days",
				value: 30,
			},
		});
	});

	it("detects when retention source changes should reset drafts", () => {
		const current = readDataRetentionSources({
			request_logs_retention_mode: "amount",
			request_logs_retention_amount: 10,
			request_logs_retention_days: 7,
		});
		const same = readDataRetentionSources({
			request_logs_retention_mode: "amount",
			request_logs_retention_amount: 10,
			request_logs_retention_days: 7,
		});
		const changed = readDataRetentionSources({
			request_logs_retention_mode: "time",
			request_logs_retention_amount: 10,
			request_logs_retention_days: 7,
		});

		expect(shouldResetDataRetentionDrafts(current, same)).toBe(false);
		expect(shouldResetDataRetentionDrafts(current, changed)).toBe(true);
	});

	it("preserves retention duration when the logs unit changes and carries read-only state", () => {
		const onCommit = vi.fn();
		const controls = buildLogsRetentionControls({
			source: {
				mode: "time",
				amount: 10,
				unit: "days",
				value: 2,
			},
			draft: null,
			disabled: true,
			onCommit,
		});

		expect(controls.disabled).toBe(true);
		controls.onUnitChange("hours");

		expect(onCommit).toHaveBeenCalledWith({
			mode: "time",
			amount: 10,
			unit: "hours",
			value: 48,
		});
	});

	it("computes transcription delete-recordings disable state from profile scope and retention values", () => {
		const controls = buildTranscriptionRetentionControls({
			source: {
				mode: "time",
				amount: 1000,
				unit: "days",
				value: 0,
			},
			draft: null,
			deleteRecordings: false,
			disabled: true,
			onCommit: vi.fn(),
			onDeleteRecordingsChange: vi.fn(),
		});

		expect(controls.disabled).toBe(true);
		expect(controls.deleteRecordingsDisabled).toBe(true);
	});

	it("commits logs retention with targeted settings and request-log invalidation", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await commitLogsRetention(
			{
				mode: "time",
				amount: 10,
				unit: "hours",
				value: 24,
			},
			deps,
			effects,
		);

		expect(deps.updateRequestLogsRetention).toHaveBeenCalledWith({
			mode: "time",
			amount: 10,
			days: 1,
		});
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.onRequestLogsChanged).toHaveBeenCalledTimes(1);
	});

	it("commits recordings retention and keeps the legacy max-saved-recordings key in sync for amount mode", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await commitRecordingsRetention(
			{
				mode: "amount",
				amount: 120,
				unit: "days",
				value: 0,
			},
			deps,
			effects,
		);

		expect(deps.updateRecordingsRetention).toHaveBeenCalledWith({
			mode: "amount",
			amount: 120,
			unit: "days",
			value: 0,
		});
		expect(deps.updateMaxSavedRecordings).toHaveBeenCalledWith(120);
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.onRecordingsChanged).toHaveBeenCalledTimes(1);
	});

	it("commits stats retention with settings-only invalidation", async () => {
		const deps = createDeps();
		const effects = createEffects();

		await commitStatsRetention(
			{
				unit: "hours",
				value: 12,
			},
			deps,
			effects,
		);

		expect(deps.updateStatsRetention).toHaveBeenCalledWith({
			unit: "hours",
			value: 12,
		});
		expect(effects.onSettingsChanged).toHaveBeenCalledTimes(1);
		expect(effects.onRequestLogsChanged).not.toHaveBeenCalled();
		expect(effects.onRecordingsChanged).not.toHaveBeenCalled();
	});
});
