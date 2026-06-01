import type { NotificationData } from "@mantine/notifications";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";

import { type AppSettings, logsAPI, tauriAPI } from "../tauri";

type MaybePromise<T> = T | Promise<T>;

export type LogsExportKind = "privacySafe" | "full";

export type LogsNotificationIntent = NotificationData;

export interface LogsExportPlan {
	kind: LogsExportKind;
	defaultPath: string;
	stripTextAndPayloads: boolean;
}

export interface LogsOrchestrationDependencies {
	selectExportPath: (plan: LogsExportPlan) => Promise<string | null>;
	exportRequestLogsToFile: (params: {
		path: string;
		stripTextAndPayloads: boolean;
	}) => Promise<void>;
	clearRequestLogs: () => Promise<void>;
	updateHotkeyDebugEnabled: (enabled: boolean) => Promise<void>;
}

export interface LogsOrchestrationEffects {
	onRequestLogsChanged: () => MaybePromise<void>;
	onSettingsChanged: () => MaybePromise<void>;
}

export type LogsExportResult =
	| { kind: "cancelled" }
	| { kind: "exported"; exportKind: LogsExportKind; path: string };

const defaultLogsOrchestrationDependencies: LogsOrchestrationDependencies = {
	selectExportPath: async (plan) => {
		const selected = await save({
			defaultPath: plan.defaultPath,
			filters: [{ name: "JSON", extensions: ["json"] }],
		});
		return typeof selected === "string" ? selected : null;
	},
	exportRequestLogsToFile: (params) => logsAPI.exportRequestLogsToFile(params),
	clearRequestLogs: () => logsAPI.clearRequestLogs(),
	updateHotkeyDebugEnabled: (enabled) =>
		tauriAPI.updateHotkeyDebugEnabled(enabled),
};

export function getLogsExportPlan(kind: LogsExportKind): LogsExportPlan {
	if (kind === "full") {
		return {
			kind,
			defaultPath: "kolboo-request-logs-full.json",
			stripTextAndPayloads: false,
		};
	}

	return {
		kind,
		defaultPath: "kolboo-request-logs.json",
		stripTextAndPayloads: true,
	};
}

export function getLogsExportSuccessNotification(
	kind: LogsExportKind,
): LogsNotificationIntent {
	return {
		title: "Export",
		message:
			kind === "privacySafe"
				? "Exported privacy-safe request logs."
				: "Exported full request logs.",
		color: "teal",
	};
}

export function getLogsExportFailureNotification(
	error: unknown,
): LogsNotificationIntent {
	return {
		title: "Export failed",
		message: String(error),
		color: "red",
	};
}

export async function exportLogsWithDialog(
	kind: LogsExportKind,
	deps: LogsOrchestrationDependencies,
): Promise<LogsExportResult> {
	const plan = getLogsExportPlan(kind);
	const path = await deps.selectExportPath(plan);
	if (!path) {
		return { kind: "cancelled" };
	}
	await deps.exportRequestLogsToFile({
		path,
		stripTextAndPayloads: plan.stripTextAndPayloads,
	});
	return {
		kind: "exported",
		exportKind: kind,
		path,
	};
}

export async function clearLogsAndInvalidate(
	deps: Pick<LogsOrchestrationDependencies, "clearRequestLogs">,
	effects: Pick<LogsOrchestrationEffects, "onRequestLogsChanged">,
): Promise<void> {
	await deps.clearRequestLogs();
	await effects.onRequestLogsChanged();
}

export async function disableHotkeyDebugOnCleanup(
	hotkeyDebugEnabled: boolean,
	deps: Pick<LogsOrchestrationDependencies, "updateHotkeyDebugEnabled">,
	effects: Pick<LogsOrchestrationEffects, "onSettingsChanged">,
): Promise<boolean> {
	if (!hotkeyDebugEnabled) {
		return false;
	}
	await deps.updateHotkeyDebugEnabled(false);
	await effects.onSettingsChanged();
	return true;
}

export function useLogsViewOrchestration(args: {
	hotkeyDebugEnabled: boolean;
	deps?: LogsOrchestrationDependencies;
}) {
	const { hotkeyDebugEnabled, deps = defaultLogsOrchestrationDependencies } =
		args;
	const queryClient = useQueryClient();
	const [exportOpened, setExportOpened] = useState(false);
	const hotkeyDebugEnabledRef = useRef(hotkeyDebugEnabled);
	useEffect(() => {
		hotkeyDebugEnabledRef.current = hotkeyDebugEnabled;
	}, [hotkeyDebugEnabled]);

	useEffect(() => {
		return () => {
			// Hotkey debug is intentionally ephemeral. If the user leaves the page,
			// switch the noisy stream back off without blocking unmount.
			void disableHotkeyDebugOnCleanup(hotkeyDebugEnabledRef.current, deps, {
				onSettingsChanged: () =>
					queryClient.invalidateQueries({ queryKey: ["settings"] }),
			});
		};
	}, [deps, queryClient]);
	const exportLogs = useMutation({
		mutationFn: (kind: LogsExportKind) => exportLogsWithDialog(kind, deps),
		onSuccess: (result) => {
			if (result.kind === "exported") {
				setExportOpened(false);
			}
		},
	});
	const clearLogs = useMutation({
		mutationFn: () =>
			clearLogsAndInvalidate(deps, {
				onRequestLogsChanged: () =>
					queryClient.invalidateQueries({ queryKey: ["requestLogs"] }),
			}),
	});
	const updateHotkeyDebugEnabled = useMutation({
		mutationFn: (enabled: boolean) => deps.updateHotkeyDebugEnabled(enabled),
		onMutate: async (enabled: boolean) => {
			await queryClient.cancelQueries({ queryKey: ["settings"] });
			const previous = queryClient.getQueryData<AppSettings>(["settings"]);
			if (previous) {
				queryClient.setQueryData<AppSettings>(["settings"], {
					...previous,
					hotkey_debug_enabled: enabled,
				});
			}
			return { previous };
		},
		onError: (_error, _enabled, context) => {
			if (context?.previous) {
				queryClient.setQueryData<AppSettings>(["settings"], context.previous);
			}
		},
		onSettled: () => {
			queryClient.invalidateQueries({ queryKey: ["settings"] });
		},
	});
	return {
		exportOpened,
		setExportOpened,
		exportLogs,
		clearLogs,
		updateHotkeyDebugEnabled,
	};
}
