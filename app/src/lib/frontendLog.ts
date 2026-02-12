/**
 * Frontend-to-backend log bridge.
 *
 * Sends structured log entries from the frontend into the Rust tracing
 * pipeline, which writes them to the same rolling log files the backend uses.
 * This is the primary way to get frontend logs from production builds where
 * DevTools is not available to end users.
 *
 * Logs are batched and flushed every 200ms to avoid excessive IPC overhead.
 */
import { invoke } from "@tauri-apps/api/core";

type LogLevel = "error" | "warn" | "info" | "debug";

interface QueuedEntry {
	level: LogLevel;
	scope: string;
	message: string;
}

const queue: QueuedEntry[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;

function flush() {
	flushTimer = null;
	const batch = queue.splice(0);
	for (const entry of batch) {
		invoke("frontend_log", {
			level: entry.level,
			scope: entry.scope,
			message: entry.message,
		}).catch(() => {
			// Swallow — we can't log a failure to log.
		});
	}
}

function enqueue(level: LogLevel, scope: string, message: string) {
	queue.push({ level, scope, message });
	if (!flushTimer) {
		flushTimer = setTimeout(flush, 200);
	}
}

/**
 * Log a message to the backend trace log files.
 *
 * @example
 * ```ts
 * frontendLog.info("setup-guide", "Welcome phase started");
 * frontendLog.debug("overlay", `pipeline=${state}`);
 * frontendLog.error("settings", `Failed to save: ${err}`);
 * ```
 */
export const frontendLog = {
	error: (scope: string, message: string) => enqueue("error", scope, message),
	warn: (scope: string, message: string) => enqueue("warn", scope, message),
	info: (scope: string, message: string) => enqueue("info", scope, message),
	debug: (scope: string, message: string) => enqueue("debug", scope, message),
};
