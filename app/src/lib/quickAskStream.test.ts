import { expect, it } from "vitest";
import { QuickAskStream } from "./quickAskStream";
import type { QuickAskAnswerPayload, QuickAskPartialPayload } from "./tauri";

it("accepts monotonic snapshots and ignores old, duplicate and dismissed answers", () => {
	const stream = new QuickAskStream();
	const partial = (request_id: string, seq: number, text = "שלום") => ({
		request_id,
		seq,
		text,
	});
	expect(stream.partial(partial("one", 1))).toBeNull();
	stream.begin("one");
	expect(stream.partial(partial("one", 1))).toBe("שלום");
	expect(stream.partial(partial("one", 1))).toBeNull();
	expect(stream.partial(partial("one", 0))).toBeNull();
	expect(stream.partial(partial("one", 3, "שלום world"))).toBe("שלום world");
	expect(stream.partial(partial("one", 2))).toBeNull();
	stream.begin("two");
	expect(stream.partial(partial("one", 100))).toBeNull();
	expect(stream.finish({ request_id: "one", ok: true, answer: "old" })).toBe(
		false,
	);
	expect(
		stream.finish({ request_id: "two", ok: false, error: "interrupted" }),
	).toBe(true);
	expect(stream.partial(partial("two", 1))).toBeNull();
	expect(stream.finish({ request_id: "two", ok: true, answer: "late" })).toBe(
		false,
	);
	expect(stream.cancel()).toBeNull();
	stream.begin("three");
	expect(stream.cancel()).toBe("three");
	expect(stream.cancel()).toBeNull();
	expect(stream.partial(partial("three", 1))).toBeNull();
	stream.begin(null);
	expect(
		stream.finish({ ok: true, answer: "Legacy backend final answer" }),
	).toBe(true);
});

it("rejects malformed and oversized native events without ending the current answer", () => {
	const stream = new QuickAskStream();
	stream.begin("one");
	for (const payload of [
		null,
		{ request_id: "one", seq: Number.NaN, text: "a" },
		{ request_id: "one", seq: 1.5, text: "a" },
		{ request_id: "one", seq: 1, text: null },
		{ request_id: "one", seq: 1, text: "a".repeat(1024 * 1024 + 1) },
	]) {
		expect(stream.partial(payload as QuickAskPartialPayload)).toBeNull();
	}
	for (const payload of [
		null,
		"invalid",
		{ request_id: "one", ok: "yes" },
		{ request_id: "one", ok: true },
		{ request_id: "one", ok: false },
	]) {
		expect(stream.finish(payload as unknown as QuickAskAnswerPayload)).toBe(
			false,
		);
	}
	expect(stream.partial({ request_id: "one", seq: 2, text: "answer" })).toBe(
		"answer",
	);
	expect(stream.finish({ request_id: "one", ok: true, answer: "answer" })).toBe(
		true,
	);
});
