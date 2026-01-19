import { useCallback, useEffect, useRef } from "react";
import { tauriAPI } from "../lib/tauri";

type HoverPosition = {
	x: number;
	y: number;
	ts: number;
};

type HoverGatingState = {
	lastMouseMoveTs: number;
	lastMousePos: HoverPosition | null;
	lastOverlayShowTs: number;
	suppressHoverUntilLeave: boolean;
};

type OverlayHoverGatingOptions = {
	enabled: boolean;
	shouldShowPresets: boolean;
	getWidgetElement: () => HTMLDivElement | null;
};

const DEFAULT_HOVER_STATE: HoverGatingState = {
	lastMouseMoveTs: Date.now(),
	lastMousePos: null,
	lastOverlayShowTs: 0,
	suppressHoverUntilLeave: false,
};

export function useOverlayHoverGating({
	enabled,
	shouldShowPresets,
	getWidgetElement,
}: OverlayHoverGatingOptions) {
	const stateRef = useRef<HoverGatingState>({ ...DEFAULT_HOVER_STATE });

	useEffect(() => {
		const onMoveWithPos = (event: MouseEvent) => {
			const now = Date.now();
			stateRef.current.lastMouseMoveTs = now;
			stateRef.current.lastMousePos = {
				x: event.clientX,
				y: event.clientY,
				ts: now,
			};
		};

		window.addEventListener("mousemove", onMoveWithPos, { passive: true });
		return () => {
			window.removeEventListener("mousemove", onMoveWithPos);
		};
	}, []);

	const markOverlayShownForHoverGating = useCallback(() => {
		stateRef.current.lastOverlayShowTs = Date.now();
		stateRef.current.suppressHoverUntilLeave = false;

		// If the overlay becomes visible under the cursor, we want to require
		// leave + re-enter before showing the hover panel.
		const pos = stateRef.current.lastMousePos;
		if (!pos) return;

		const el = getWidgetElement();
		if (!el) return;

		// Wait two frames so layout/visibility has settled.
		requestAnimationFrame(() => {
			requestAnimationFrame(() => {
				try {
					const hit = document.elementFromPoint(pos.x, pos.y);
					if (hit && el.contains(hit)) {
						stateRef.current.suppressHoverUntilLeave = true;
						tauriAPI.hideOverlayHover().catch(() => {});
					}
				} catch {
					// ignore
				}
			});
		});
	}, [getWidgetElement]);

	const handleMouseEnter = useCallback(() => {
		if (!enabled || !shouldShowPresets) {
			tauriAPI.hideOverlayHover().catch(() => {});
			return;
		}

		if (stateRef.current.suppressHoverUntilLeave) {
			return;
		}

		const now = Date.now();
		const justShown = now - stateRef.current.lastOverlayShowTs < 650;
		const movedRecently = now - stateRef.current.lastMouseMoveTs < 120;
		if (justShown && !movedRecently) {
			stateRef.current.suppressHoverUntilLeave = true;
			tauriAPI.hideOverlayHover().catch(() => {});
			return;
		}

		tauriAPI.showOverlayHover().catch(() => {});
	}, [enabled, shouldShowPresets]);

	const handleMouseLeave = useCallback(() => {
		if (stateRef.current.suppressHoverUntilLeave) {
			// User has moved away; next enter is an intentional hover.
			stateRef.current.suppressHoverUntilLeave = false;
			return;
		}

		if (!enabled || !shouldShowPresets) {
			tauriAPI.hideOverlayHover().catch(() => {});
			return;
		}

		tauriAPI.scheduleHideOverlayHover(220).catch(() => {});
	}, [enabled, shouldShowPresets]);

	return {
		markOverlayShownForHoverGating,
		handleMouseEnter,
		handleMouseLeave,
	};
}
