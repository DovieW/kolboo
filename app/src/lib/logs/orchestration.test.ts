import { describe, expect, it, vi } from "vitest";

import {
	clearLogsAndInvalidate,
	disableHotkeyDebugOnCleanup,
	exportLogsWithDialog,
	getLogsExportFailureNotification,
	getLogsExportPlan,
	getLogsExportSuccessNotification,
	type LogsOrchestrationDependencies,
} from "./orchestration";

function createDeps(
	overrides: Partial<LogsOrchestrationDependencies> = {},
): LogsOrchestrationDependencies {
	return {
		selectExportPath: vi.fn(async () => null),
		exportRequestLogsToFile: vi.fn(async () => undefined),
		clearRequestLogs: vi.fn(async () => undefined),
		updateHotkeyDebugEnabled: vi.fn(async () => undefined),
		...overrides,
	};
}

describe("Logs orchestration helpers", () => {
	it("builds privacy-safe and full export plans", () => {
		expect(getLogsExportPlan("privacySafe")).toEqual({
			kind: "privacySafe",
			defaultPath: "kolboo-request-logs.json",
			stripTextAndPayloads: true,
		});
		expect(getLogsExportPlan("full")).toEqual({
			kind: "full",
			defaultPath: "kolboo-request-logs-full.json",
			stripTextAndPayloads: false,
		});
	});

	it("returns export notification copy for success and failure", () => {
		expect(getLogsExportSuccessNotification("privacySafe")).toEqual({
			title: "Export",
			message: "Exported privacy-safe request logs.",
			color: "teal",
		});
		expect(getLogsExportFailureNotification(new Error("boom"))).toEqual({
			title: "Export failed",
			message: "Error: boom",
			color: "red",
		});
	});

	it("skips export when the user cancels the save dialog", async () => {
		const deps = createDeps();

		await expect(exportLogsWithDialog("privacySafe", deps)).resolves.toEqual({
			kind: "cancelled",
		});
		expect(deps.exportRequestLogsToFile).not.toHaveBeenCalled();
	});

	it("exports using the selected path and privacy stripping mode", async () => {
		const deps = createDeps({
			selectExportPath: vi.fn(async (plan) => {
				expect(plan).toEqual({
					kind: "full",
					defaultPath: "kolboo-request-logs-full.json",
					stripTextAndPayloads: false,
				});
				return "C:/tmp/logs.json";
			}),
		});

		await expect(exportLogsWithDialog("full", deps)).resolves.toEqual({
			kind: "exported",
			exportKind: "full",
			path: "C:/tmp/logs.json",
		});
		expect(deps.exportRequestLogsToFile).toHaveBeenCalledWith({
			path: "C:/tmp/logs.json",
			stripTextAndPayloads: false,
		});
	});

	it("clears logs and triggers request-log invalidation intent", async () => {
		const deps = createDeps();
		const onRequestLogsChanged = vi.fn(async () => undefined);

		await clearLogsAndInvalidate(deps, { onRequestLogsChanged });

		expect(deps.clearRequestLogs).toHaveBeenCalledTimes(1);
		expect(onRequestLogsChanged).toHaveBeenCalledTimes(1);
	});

	it("turns off hotkey debug during cleanup only when needed", async () => {
		const deps = createDeps();
		const onSettingsChanged = vi.fn(async () => undefined);

		await expect(
			disableHotkeyDebugOnCleanup(false, deps, { onSettingsChanged }),
		).resolves.toBe(false);
		expect(deps.updateHotkeyDebugEnabled).not.toHaveBeenCalled();
		expect(onSettingsChanged).not.toHaveBeenCalled();

		await expect(
			disableHotkeyDebugOnCleanup(true, deps, { onSettingsChanged }),
		).resolves.toBe(true);
		expect(deps.updateHotkeyDebugEnabled).toHaveBeenCalledWith(false);
		expect(onSettingsChanged).toHaveBeenCalledTimes(1);
	});
});
