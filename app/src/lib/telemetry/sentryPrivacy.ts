import type { ErrorEvent, StackFrame } from "@sentry/react";

// Error messages can contain arbitrary provider responses or dictated text.
// Keep code locations for grouping/symbolication, not a blacklist of content.
const ERROR_TYPES = new Set([
	"Error",
	"TypeError",
	"RangeError",
	"ReferenceError",
	"SyntaxError",
	"URIError",
	"EvalError",
	"AggregateError",
	"DOMException",
]);
const TAG_VALUES = new Map<string, readonly string[]>([
	["service", ["desktop"]],
	["runtime", ["webview"]],
	["surface", ["main", "overlay", "overlay_hover", "quick_ask"]],
	["tier", ["community", "personal", "enterprise", "unknown"]],
	["smoke_test", ["true"]],
	["smoke_trigger", ["query-param", "runtime-env"]],
	["os", ["linux", "windows", "macos", "unknown"]],
]);

function codeLocation(value: string): string {
	// Packaged asset names and debug IDs suffice for source-map matching. Never
	// send a local user directory, URL query, or fragment from a thrown error.
	const path = value.replace(/[?#].*$/, "").replaceAll("\\", "/");
	return `app:///${path.replace(/^.*\//, "")}`;
}

function scrubFrame(frame: StackFrame): StackFrame {
	return {
		filename: frame.filename ? codeLocation(frame.filename) : undefined,
		function: frame.function,
		lineno: frame.lineno,
		colno: frame.colno,
		in_app: frame.in_app,
	};
}

export function scrubSentryEvent(event: ErrorEvent): ErrorEvent {
	const tags: NonNullable<ErrorEvent["tags"]> = {};
	for (const [key, value] of Object.entries(event.tags ?? {})) {
		if (typeof value !== "string") continue;
		if (TAG_VALUES.get(key)?.includes(value)) tags[key] = value;
		// Actions are internal identifiers, never messages supplied by a caller.
		if (
			key === "action" &&
			/^(?:smoke_test|license_[a-z_]{1,64})$/.test(value)
		) {
			tags[key] = value;
		}
	}

	return {
		event_id: event.event_id,
		type: undefined,
		timestamp: event.timestamp,
		level: event.level,
		platform: event.platform,
		release: event.release,
		dist: event.dist,
		environment: event.environment,
		sdk: event.sdk,
		tags,
		message:
			event.message === undefined
				? undefined
				: "Desktop error (details withheld)",
		exception: event.exception && {
			values: event.exception.values?.map((exception) => ({
				type: ERROR_TYPES.has(exception.type ?? "") ? exception.type : "Error",
				value: "Error details withheld for privacy",
				stacktrace: exception.stacktrace && {
					frames: exception.stacktrace.frames?.map(scrubFrame),
				},
				mechanism: exception.mechanism && {
					type: exception.mechanism.type,
					handled: exception.mechanism.handled,
				},
			})),
		},
		// These IDs are essential: scrubbing must not make minified reports useless.
		debug_meta: event.debug_meta && {
			images: event.debug_meta.images
				?.filter((image) => image.type === "sourcemap")
				.map((image) => ({
					type: "sourcemap" as const,
					debug_id: image.debug_id,
					code_file: codeLocation(image.code_file),
				})),
		},
	};
}
