import type { QuickAskDismissMode } from "./tauri";

export const DEFAULT_QUICK_ASK_DISMISS_MODE: QuickAskDismissMode = "manual";

export const QUICK_ASK_DISMISS_MODE_LABELS: Record<
	QuickAskDismissMode,
	string
> = {
	manual: "Manual",
	auto: "Auto",
};

export const QUICK_ASK_DISMISS_MODE_OPTIONS = (
	Object.entries(QUICK_ASK_DISMISS_MODE_LABELS) as Array<
		[QuickAskDismissMode, string]
	>
).map(([value, label]) => ({ value, label }));
