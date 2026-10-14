import type { QuickAskAnswerPayload, QuickAskPartialPayload } from "./tauri";

/** Correlation and ordering apply equally to previews, completion and dismissal. */
export class QuickAskStream {
	private requestId: string | null = null;
	private seq = 0;
	private active = false;
	private generation = 0;
	get version() {
		return this.generation;
	}

	begin(requestId: string | null) {
		this.generation += 1;
		this.requestId = requestId;
		this.seq = 0;
		this.active = true;
	}

	partial(payload: QuickAskPartialPayload): string | null {
		if (
			!this.active ||
			!payload ||
			payload.request_id !== this.requestId ||
			!Number.isSafeInteger(payload.seq) ||
			payload.seq <= this.seq ||
			typeof payload.text !== "string" ||
			payload.text.length > 1024 * 1024
		)
			return null;
		this.seq = payload.seq;
		return payload.text;
	}

	finish(payload: QuickAskAnswerPayload): boolean {
		if (
			!this.active ||
			!payload ||
			typeof payload !== "object" ||
			(payload.request_id ?? null) !== this.requestId ||
			(payload.ok !== true && payload.ok !== false) ||
			(payload.ok
				? typeof payload.answer !== "string"
				: typeof payload.error !== "string")
		)
			return false;
		this.active = false;
		return true;
	}

	cancel(): string | null {
		this.generation += 1;
		const requestId = this.active ? this.requestId : null;
		this.active = false;
		return requestId;
	}
}
