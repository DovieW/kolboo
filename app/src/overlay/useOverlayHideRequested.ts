import { useBackendEvent } from "../lib/tauri/useBackendEvent";

/** Subscribe once; the latest hide gate owns whether an exit is safe. */
export function useOverlayHideRequested({
	requestAnimatedHide,
}: {
	requestAnimatedHide: () => void;
}) {
	useBackendEvent("overlay-hide-requested", requestAnimatedHide);
}
