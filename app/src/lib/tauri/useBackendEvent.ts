import { useEffect, useRef } from "react";
import { type EventMap, type EventName, listenTyped } from "./events";

/** Keep one subscription, with fresh callbacks and safe asynchronous teardown. */
export function useBackendEvent<K extends EventName>(
	name: K,
	handler: (payload: EventMap[K]) => void,
	enabled = true,
) {
	const current = useRef(handler);
	current.current = handler;
	useEffect(() => {
		if (!enabled) return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void listenTyped(name, (payload) => {
			if (!disposed) current.current(payload);
		})
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => {
				// State polling remains a fallback when native events are unavailable.
			});
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [name, enabled]);
}
