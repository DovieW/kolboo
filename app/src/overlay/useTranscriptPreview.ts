import { useRef, useState } from "react";
import { useBackendEvent } from "../lib/tauri/useBackendEvent";

/** A preview is provisional, local UI only, and owned by one recording epoch. */
export function useTranscriptPreview() {
	const current = useRef<number | null>(null);
	const closed = useRef(true);
	const [text, setText] = useState("");
	useBackendEvent("stt-partial-transcript", (payload) => {
		if (payload.kind === "started") {
			if (current.current !== null && payload.session_epoch <= current.current)
				return;
			current.current = payload.session_epoch;
			closed.current = false;
			setText("");
		} else if (!closed.current && payload.session_epoch === current.current) {
			setText(payload.text);
		}
	});
	const clear = () => {
		closed.current = true;
		setText("");
	};
	useBackendEvent("pipeline-cancelled", clear);
	useBackendEvent("pipeline-reset", clear);
	useBackendEvent("pipeline-error", clear);
	useBackendEvent("pipeline-transcript-ready", clear);
	return text;
}
