import { describe, expect, it } from "vitest";
import {
	evaluatePatchCoverage,
	mergeCoverageReports,
	parseChangedLines,
	parseLcov,
} from "./patch-coverage.mjs";

const repoRoot = "/repo";

describe("100% patch coverage gate", () => {
	it("maps changed hunks to their new line numbers", () => {
		const changed = parseChangedLines(`diff --git c/app/src/example.ts w/app/src/example.ts
+++ w/app/src/example.ts
@@ -2,0 +3,2 @@
+first
+second`);
		expect([...(changed.get("app/src/example.ts") ?? [])]).toEqual([3, 4]);
	});

	it("accepts standard prefixes and ignores deleted files", () => {
		const changed = parseChangedLines(`diff --git a/app/src/example.ts b/app/src/example.ts
+++ b/app/src/example.ts
@@ -0,0 +1 @@
+added
diff --git a/app/src/deleted.ts b/app/src/deleted.ts
+++ /dev/null
@@ -1 +0,0 @@
-deleted`);
		expect([...(changed.get("app/src/example.ts") ?? [])]).toEqual([1]);
		expect(changed.has("app/src/deleted.ts")).toBe(false);
	});

	it("requires changed lines, branches, and functions to execute", () => {
		const changed = new Map([
			["app/src/example.ts", new Set([3, 4, 5])],
		]);
		const coverage = parseLcov(
			`SF:src/example.ts
FN:3,newBehavior
FNDA:0,newBehavior
DA:3,1
DA:4,0
DA:5,1
BRDA:5,0,0,1
BRDA:5,0,1,0
end_of_record`,
			{ sourceRoot: "/repo/app", repoRoot },
		);

		expect(evaluatePatchCoverage(changed, coverage).failures).toEqual([
			expect.objectContaining({
				filePath: "app/src/example.ts",
				uncoveredLines: [4],
				uncoveredBranches: [5],
				uncoveredFunctions: [3],
			}),
		]);
	});

	it("combines frontend and Rust LCOV while ignoring tests and type-only files", () => {
		const frontend = parseLcov("SF:src/example.ts\nDA:2,1\nend_of_record", {
			sourceRoot: "/repo/app",
			repoRoot,
		});
		const rust = parseLcov(
			"SF:/repo/app/src-tauri/src/example.rs\nDA:7,1\nend_of_record",
			{ sourceRoot: "/repo/app", repoRoot },
		);
		const changed = new Map([
			["app/src/example.ts", new Set([2])],
			["app/src/example.test.ts", new Set([1])],
			["app/src/lib/tauri/types.ts", new Set([1])],
			["app/src-tauri/src/example.rs", new Set([7])],
		]);

		expect(
			evaluatePatchCoverage(
				changed,
				mergeCoverageReports([frontend, rust]),
			),
		).toEqual({ failures: [], executableLines: 2 });
	});

	it("fails closed when changed production source is absent from reports", () => {
		const result = evaluatePatchCoverage(
			new Map([["app/src/newFeature.ts", new Set([1])]]),
			new Map(),
		);
		expect(result.failures).toEqual([
			{ filePath: "app/src/newFeature.ts", missingReport: true },
		]);
	});
});
