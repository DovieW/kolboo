export type TextDiffChunk = {
	value: string;
	added?: boolean;
	removed?: boolean;
};

function normalizeEol(text: string): string {
	return text.replace(/\r\n/g, "\n");
}

function tokenizePreserveWhitespace(text: string): string[] {
	// Split into tokens while preserving whitespace as its own token.
	// We also split punctuation into separate tokens, so tiny edits like adding
	// a comma become an insertion of "," rather than a replacement of "word".
	const normalized = normalizeEol(text);

	// Order matters: we want to preserve whitespace tokens, keep "word-like" runs,
	// and treat everything else as its own token.
	//
	// Uses unicode properties to behave better with non-English text.
	const re = /(\s+|[\p{L}\p{N}]+(?:[’'][\p{L}\p{N}]+)*|[^\s])/gu;
	const tokens: string[] = [];
	for (const match of normalized.matchAll(re)) {
		const tok = match[0];
		if (tok.length > 0) tokens.push(tok);
	}
	return tokens;
}

function coalesceChunks(chunks: TextDiffChunk[]): TextDiffChunk[] {
	const out: TextDiffChunk[] = [];
	for (const chunk of chunks) {
		const prev = out[out.length - 1];
		const sameKind =
			prev &&
			Boolean(prev.added) === Boolean(chunk.added) &&
			Boolean(prev.removed) === Boolean(chunk.removed);

		if (prev && sameKind) {
			prev.value += chunk.value;
		} else {
			out.push({ ...chunk });
		}
	}
	return out;
}

function myersDiff(a: string[], b: string[]): TextDiffChunk[] {
	const n = a.length;
	const m = b.length;

	if (n === 0 && m === 0) return [];
	if (n === 0) return [{ value: b.join(""), added: true }];
	if (m === 0) return [{ value: a.join(""), removed: true }];

	// Myers O((N+M)D) diff over token arrays.
	// trace[d] holds the V map for that edit distance d.
	const max = n + m;

	let v = new Map<number, number>();
	v.set(1, 0);

	const trace: Array<Map<number, number>> = [];

	let found = false;
	let dFound = 0;

	for (let d = 0; d <= max; d += 1) {
		const vNext = new Map<number, number>();

		for (let k = -d; k <= d; k += 2) {
			const kPlus = v.get(k + 1);
			const kMinus = v.get(k - 1);

			let x: number;
			if (k === -d || (k !== d && (kMinus ?? -1) < (kPlus ?? -1))) {
				x = kPlus ?? 0;
			} else {
				x = (kMinus ?? 0) + 1;
			}

			let y = x - k;

			while (x < n && y < m && a[x] === b[y]) {
				x += 1;
				y += 1;
			}

			vNext.set(k, x);

			if (x >= n && y >= m) {
				trace.push(vNext);
				found = true;
				dFound = d;
				break;
			}
		}

		if (found) break;

		trace.push(vNext);
		v = vNext;
	}

	// Backtrack to produce an edit script.
	let x = n;
	let y = m;

	const ops: Array<
		| { type: "equal"; tok: string }
		| { type: "insert"; tok: string }
		| { type: "delete"; tok: string }
	> = [];

	for (let d = dFound; d > 0; d -= 1) {
		const vPrev = trace[d - 1];
		if (!vPrev) break;
		const k = x - y;

		const kPrev =
			k === -d ||
			(k !== d && (vPrev.get(k - 1) ?? -1) < (vPrev.get(k + 1) ?? -1))
				? k + 1
				: k - 1;

		const xPrev = vPrev.get(kPrev) ?? 0;
		const yPrev = xPrev - kPrev;

		// Walk back along the diagonal (matching tokens).
		while (x > xPrev && y > yPrev) {
			const tok = a[x - 1];
			if (tok !== undefined) ops.push({ type: "equal", tok });
			x -= 1;
			y -= 1;
		}

		// One edit step.
		if (x === xPrev) {
			const tok = b[y - 1];
			if (tok !== undefined) ops.push({ type: "insert", tok });
			y -= 1;
		} else {
			const tok = a[x - 1];
			if (tok !== undefined) ops.push({ type: "delete", tok });
			x -= 1;
		}
	}

	// Finish remaining common prefix.
	while (x > 0 && y > 0) {
		if (a[x - 1] === b[y - 1]) {
			const tok = a[x - 1];
			if (tok !== undefined) ops.push({ type: "equal", tok });
			x -= 1;
			y -= 1;
		} else {
			// If we diverged early, fall back to deletes/inserts.
			const tok = a[x - 1];
			if (tok !== undefined) ops.push({ type: "delete", tok });
			x -= 1;
		}
	}
	while (x > 0) {
		const tok = a[x - 1];
		if (tok !== undefined) ops.push({ type: "delete", tok });
		x -= 1;
	}
	while (y > 0) {
		const tok = b[y - 1];
		if (tok !== undefined) ops.push({ type: "insert", tok });
		y -= 1;
	}

	ops.reverse();

	const chunks: TextDiffChunk[] = ops.map((op) => {
		if (op.type === "equal") return { value: op.tok };
		if (op.type === "insert") return { value: op.tok, added: true };
		return { value: op.tok, removed: true };
	});

	return coalesceChunks(chunks);
}

export function diffTextInline(before: string, after: string): TextDiffChunk[] {
	const a = tokenizePreserveWhitespace(before);
	const b = tokenizePreserveWhitespace(after);
	return myersDiff(a, b);
}

export function isDiffTrivial(chunks: TextDiffChunk[]): boolean {
	return chunks.every((c) => !c.added && !c.removed);
}
