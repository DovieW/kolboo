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

    const json = safeJsonStringify(error);
    if (json && json !== "{}") return json;
  }

  // Last resort.
  return String(error);
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
