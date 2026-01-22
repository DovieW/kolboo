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
		let unlisten: (() => void) | undefined;

		const setup = async () => {
			unlisten = await listenTyped("overlay-hide-requested", () => {
				requestAnimatedHide();
			});
		};

		void setup();
		return () => {
			unlisten?.();
		};
	}, [requestAnimatedHide]);
}
