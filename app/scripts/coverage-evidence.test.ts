import path from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import {
	evaluateCoverage,
	formatEvidenceRows,
	isCliEntrypoint,
	normalizeCoverageSummary,
} from "./coverage-evidence.mjs";

describe("coverage evidence helpers", () => {
	it("normalizes coverage summary file paths and missing metrics", () => {
		const summary = normalizeCoverageSummary({
			"src\\example.ts": {
				statements: { pct: 100 },
				branches: { pct: Number.NaN },
				functions: { pct: 88.5 },
			},
			total: {
				statements: { pct: 10 },
			},
		});

		expect(summary.get("src/example.ts")).toEqual({
			statements: 100,
			branches: 0,
			functions: 88.5,
		});
		expect(summary.has("total")).toBe(false);
	});

	it("passes only when all in-scope metrics meet the threshold", () => {
		const results = evaluateCoverage(
			{
				"src/pass.ts": {
					statements: { pct: 100 },
					branches: { pct: 100 },
					functions: { pct: 100 },
				},
				"src/fail.ts": {
					statements: { pct: 100 },
					branches: { pct: 99 },
					functions: { pct: 100 },
				},
			},
			["src/pass.ts", "src/fail.ts", "src/missing.ts"],
		);

		expect(results).toEqual([
			expect.objectContaining({ filePath: "src/pass.ts", pass: true }),
			expect.objectContaining({
				filePath: "src/fail.ts",
				pass: false,
				failures: ["branches"],
			}),
			expect.objectContaining({
				filePath: "src/missing.ts",
				pass: false,
				failures: ["missing"],
			}),
		]);
	});

	it("formats markdown evidence rows", () => {
		const rows = formatEvidenceRows([
			{
				filePath: "src/pass.ts",
				metrics: { statements: 100, branches: 100, functions: 100 },
				pass: true,
				failures: [],
			},
			{
				filePath: "src/fail.ts",
				metrics: { statements: 100, branches: 50, functions: 100 },
				pass: false,
				failures: ["branches"],
			},
		]);

		expect(rows).toContain("| src/pass.ts | 100% | 100% | 100% | PASS |");
		expect(rows).toContain(
			"| src/fail.ts | 100% | 50% | 100% | FAIL (branches) |",
		);
	});

	it("detects direct CLI execution from a file URL", () => {
		const scriptPath = path.join(
			process.cwd(),
			"scripts",
			"coverage-evidence.mjs",
		);
		const metaUrl = pathToFileURL(scriptPath).href;

		expect(isCliEntrypoint(metaUrl, scriptPath)).toBe(true);
		expect(
			isCliEntrypoint(
				metaUrl,
				path.join(process.cwd(), "scripts", "not-coverage-evidence.mjs"),
			),
		).toBe(false);
	});
});
