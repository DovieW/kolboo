/**
 * Convert unknown error values into a user-friendly message.
 *
 * This is mainly for UI notifications where `String(error)` would otherwise
 * show "[object Object]" (common with Tauri invoke CommandError payloads).
 */
export function formatErrorMessage(error: unknown): string {
  if (error == null) return "Unknown error";

  if (typeof error === "string") return error;

  if (
    typeof error === "number" ||
    typeof error === "boolean" ||
    typeof error === "bigint"
  ) {
    return String(error);
  }

  // Standard JS Errors (and many library errors) go here.
  if (error instanceof Error) {
    return error.message?.trim().length ? error.message : error.toString();
  }

  if (typeof error === "object") {
    const maybeMessage = (error as { message?: unknown }).message;
    if (typeof maybeMessage === "string" && maybeMessage.trim().length > 0) {
      return maybeMessage;
    }

    const maybeError = (error as { error?: unknown }).error;
    if (typeof maybeError === "string" && maybeError.trim().length > 0) {
      return maybeError;
    }

    if (hasSensitiveKeys(error)) {
      return "[object Object]";
    }

    const json = safeJsonStringify(error);
    if (json && json !== "{}") return json;
  }

  // Last resort.
  return String(error);
}

const SENSITIVE_KEY_PATTERN =
  /(?:api[-_]?key|access[-_]?token|refresh[-_]?token|token|secret|password|passwd|authorization|bearer)/i;

function hasSensitiveKeys(value: unknown, seen = new WeakSet<object>()): boolean {
  if (value == null || typeof value !== "object") return false;
  if (seen.has(value as object)) return false;
  seen.add(value as object);

  if (Array.isArray(value)) {
    return value.some((entry) => hasSensitiveKeys(entry, seen));
  }

  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (SENSITIVE_KEY_PATTERN.test(key)) return true;
    if (hasSensitiveKeys(entry, seen)) return true;
  }

  return false;
}

function safeJsonStringify(value: unknown): string | null {
  try {
    const seen = new WeakSet<object>();
    return JSON.stringify(value, (_key, v) => {
      if (typeof v === "object" && v !== null) {
        if (seen.has(v)) return "[Circular]";
        seen.add(v);
      }
      return v;
    });
  } catch {
    return null;
  }
}
