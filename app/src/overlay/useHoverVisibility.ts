import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { listenTyped } from "../lib/tauri/events";

/** Subscribe before acknowledging readiness; the native show intent may predate
 * this page. A hidden utility page must not keep querying foreground programs. */
export function useHoverVisibility() {
	const [visible, setVisible] = useState(false);
	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void listenTyped("overlay-hover-visibility-changed", (next) => {
			if (!disposed) setVisible(next);
		})
			.then(async (stop) => {
				if (disposed) {
					stop();
					return;
				}
				unlisten = stop;
				await invoke("overlay_hover_frontend_ready");
			})
			.catch(() => {
				// A failed subscription is not readiness; a later page reload can retry.
			});
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, []);
	return visible;
}
