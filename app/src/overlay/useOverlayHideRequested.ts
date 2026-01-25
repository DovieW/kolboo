import { useEffect } from "react";
import { listenTyped } from "../lib/tauri/events";

type UseOverlayHideRequestedInputs = {
	requestAnimatedHide: () => void;
};

/**
 * Backend can request a hide (so we can animate out before the window hides).
 */
export function useOverlayHideRequested({
	requestAnimatedHide,
}: UseOverlayHideRequestedInputs) {
	useEffect(() => {
		let cancelled = false;
		let unlisten: (() => void) | undefined;

		const setup = async () => {
			const dispose = await listenTyped("overlay-hide-requested", () => {
				requestAnimatedHide();
			});
			if (cancelled) {
				dispose();
				return;
			}
			unlisten = dispose;
		};

		void setup();
		return () => {
			cancelled = true;
			unlisten?.();
		};
	}, [requestAnimatedHide]);
}
