import { describe, expect, it } from "vitest";
import { createHash } from "node:crypto";
import {
	applyCoverageExceptions,
	evaluatePatchCoverage,
	mergeCoverageReports,
	parseChangedLines,
	parseLcov,
	resolveCoverageBase,
} from "./patch-coverage.mjs";

const repoRoot = "/repo";

describe("reviewed native coverage snapshots", () => {
	const filePath = "app/src-tauri/src/native.rs";
	const source = "native source\n";
	const entry = {
		filePath,
		sha256: createHash("sha256").update(source).digest("hex"),
		reason: "Needs an actual native desktop",
		evidence: ["native smoke checklist"],
		lines: [1],
		functions: [1],
		instantiations: [2],
	};
	const changed = new Map([[filePath, new Set([1, 2, 3])]]);
	function check(report: string, exception = entry, contents = source) {
		const coverage = parseLcov(`SF:${filePath}\n${report}\nend_of_record`, {
			sourceRoot: repoRoot,
			repoRoot,
		});
		return applyCoverageExceptions(
			evaluatePatchCoverage(changed, coverage),
			changed,
			coverage,
			new Map([[filePath, contents]]),
			[exception],
		);
	}
	it("waives only reviewed lines and separately reports them", () => {
		const result = check(
			"DA:1,0\nDA:2,1\nDA:3,0\nFN:1,_Rnative\nFNDA:0,_Rnative",
		);
		expect(result.applied).toEqual([
			{ filePath, reason: entry.reason, lines: [1], functions: [1] },
		]);
		expect(result.failures[0].uncoveredLines).toEqual([3]);
	});
	it("rejects changed source and undocumented exceptions", () => {
		for (const invalid of [
			{ ...entry, sha256: "wrong" },
			{ ...entry, reason: "" },
			{ ...entry, evidence: [] },
		]) {
			expect(() => check("DA:1,0", invalid)).toThrow("needs review");
		}
		expect(() => check("DA:1,0", entry, "edited source")).toThrow(
			"needs review",
		);
		expect(
			check("DA:1,0", entry, source.replaceAll("\n", "\r\n")).failures,
		).toEqual([]);
	});
	it("does not waive branches or turn unexecuted functions into covered code", () => {
		const result = check(
			"DA:1,0\nDA:2,0\nBRDA:1,0,0,0\nFN:2,_Rone\nFN:2,_Rtwo\nFNDA:0,_Rone\nFNDA:0,_Rtwo",
		);
		expect(result.failures[0].uncoveredBranches).toEqual([1]);
		expect(result.failures[0].uncoveredFunctions).toEqual([2]);
		expect(result.failures[0].uncoveredLines).toEqual([2]);
	});
	it("only accepts compiler instantiations when their source definition actually ran", () => {
		const report = "DA:2,1\nFN:2,_Rone\nFN:2,_Rtwo\nFNDA:1,_Rone\nFNDA:0,_Rtwo";
		expect(check(report).failures).toEqual([]);
		expect(check(report).applied[0].functions).toEqual([2]);
		expect(
			check(report.replaceAll("_Rtwo", "unknown")).failures[0]
				.uncoveredFunctions,
		).toEqual([2]);
	});
	it("keeps missing reports fatal unless every changed target-gated line was reviewed", () => {
		const evaluate = (unmeasuredLines: number[]) =>
			applyCoverageExceptions(
				evaluatePatchCoverage(changed, new Map()),
				changed,
				new Map(),
				new Map([[filePath, source]]),
				[{ ...entry, unmeasuredLines }],
			);
		expect(evaluate([1, 2]).failures).toEqual([
			{ filePath, missingReport: true },
		]);
		expect(evaluate([1, 2, 3]).failures).toEqual([]);
		expect(evaluate([1, 2, 3]).applied[0].unmeasured).toBe(true);
	});
});

describe("100% patch coverage gate", () => {
	it("enforces every change since the first policy commit even before it reaches master", () => {
		const calls: string[][] = [];
		const git = (args: string[]) => {
			calls.push(args);
			return args[0] === "merge-base"
				? "old-base"
				: "policy-added\npolicy-readded";
		};
		expect(
			resolveCoverageBase("origin/master", git, (revision: string) =>
				revision.startsWith("policy"),
			),
		).toEqual({ base: "policy-added", enforced: true });
		expect(calls[1]).toEqual([
			"log",
			"--reverse",
			"--format=%H",
			"--diff-filter=A",
			"old-base..HEAD",
			"--",
			".coverage-patch-v1",
		]);
		expect(resolveCoverageBase("origin/master", git, () => true)).toEqual({
			base: "old-base",
			enforced: true,
		});
		expect(
			resolveCoverageBase(
				"origin/master",
				() => "",
				() => false,
			),
		).toEqual({ base: "", enforced: false });
	});
	it("maps changed hunks to their new line numbers", () => {
		const changed =
			parseChangedLines(`diff --git c/app/src/example.ts w/app/src/example.ts
+++ w/app/src/example.ts
@@ -2,0 +3,2 @@
+first
+second`);
		expect([...(changed.get("app/src/example.ts") ?? [])]).toEqual([3, 4]);
	});

	it("accepts standard prefixes and ignores deleted files", () => {
		const changed =
			parseChangedLines(`diff --git a/app/src/example.ts b/app/src/example.ts
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
		const changed = new Map([["app/src/example.ts", new Set([3, 4, 5])]]);
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
			evaluatePatchCoverage(changed, mergeCoverageReports([frontend, rust])),
		).toEqual({ failures: [], executableLines: 2, metadataExceptions: [] });
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

	describe("approved Tauri metadata exception", () => {
		const file = "app/src-tauri/src/commands/history.rs";
		const source =
			"#[tauri::command]\npub async fn get_history_activity(\n    body\n)";
		const lcov = `SF:${file}
FN:1,_RNCgeneratedWrapper
FN:1,_RNCgeneratedTestWrapper
FN:2,_RNvget_history_activity
FNDA:0,_RNCgeneratedWrapper
FNDA:0,_RNCgeneratedTestWrapper
FNDA:1,_RNvget_history_activity
DA:1,0
DA:2,1
DA:3,1
end_of_record`;
		function evaluate(
			text = lcov,
			contents: string | undefined = source,
			filePath = file,
			changed = [1, 2, 3],
		) {
			return evaluatePatchCoverage(
				new Map([[filePath, new Set(changed)]]),
				parseLcov(text, { sourceRoot: repoRoot, repoRoot }),
				new Map([[filePath, contents]]),
			);
		}

		it("reports only the allowlisted annotation as exempt, retaining body coverage", () => {
			expect(evaluate()).toEqual({
				failures: [],
				executableLines: 2,
				metadataExceptions: [{ filePath: file, line: 1 }],
			});
			expect(evaluate(lcov, source.replaceAll("\n", "\r\n"))).toEqual(
				evaluate(),
			);
		});

		it("still rejects an uncovered command body", () => {
			const result = evaluate(lcov.replace("DA:3,1", "DA:3,0"));
			expect(result.failures).toEqual([
				expect.objectContaining({
					uncoveredLines: [3],
					uncoveredFunctions: [],
				}),
			]);
			expect(result.metadataExceptions).toEqual([{ filePath: file, line: 1 }]);
		});

		it("still rejects an uncovered closure inside the body", () => {
			const result = evaluate(
				lcov.replace(
					"DA:3,1",
					"FN:3,_RNCbodyClosure\nFNDA:0,_RNCbodyClosure\nDA:3,1",
				),
			);
			expect(result.failures[0].uncoveredFunctions).toEqual([3]);
		});

		it.each([
			["unexecuted command", lcov.replace("FNDA:1,_RNv", "FNDA:0,_RNv")],
			[
				"missing command function",
				lcov.replace("FNDA:1,_RNvget_history_activity\n", ""),
			],
			[
				"unknown annotation function",
				lcov.replaceAll("_RNCgeneratedWrapper", "unknownWrapper"),
			],
			[
				"no generated function records",
				lcov.replaceAll(/FNDA:0,_RNC[^\n]*\n/g, ""),
			],
			["different line mapping", lcov.replace("DA:1,0", "DA:1,1")],
			[
				"executed wrapper",
				lcov.replace(
					"FNDA:0,_RNCgeneratedWrapper",
					"FNDA:1,_RNCgeneratedWrapper",
				),
			],
			["branch on annotation", lcov.replace("DA:1,0", "DA:1,0\nBRDA:1,0,0,0")],
		])("fails closed for %s", (_name, report) => {
			const result = evaluate(report);
			expect(result.metadataExceptions).toEqual([]);
			expect(result.failures).not.toHaveLength(0);
		});

		it.each([
			[
				"another command",
				source.replace("get_history_activity", "get_history"),
			],
			[
				"inline code",
				source.replace("#[tauri::command]\n", "#[tauri::command] "),
			],
			["other macro", source.replace("tauri::command", "other::command")],
			[
				"macro options",
				source.replace("tauri::command", "tauri::command(async)"),
			],
			["truncated source", "#[tauri::command]"],
			["empty source", ""],
		])("does not apply to %s", (_name, contents) => {
			expect(evaluate(lcov, contents).metadataExceptions).toEqual([]);
			expect(evaluate(lcov, contents).failures[0].uncoveredLines).toEqual([1]);
		});

		it("does not apply without source evidence or to another file", () => {
			const coverage = parseLcov(lcov, { sourceRoot: repoRoot, repoRoot });
			expect(
				evaluatePatchCoverage(new Map([[file, new Set([1])]]), coverage)
					.failures[0].uncoveredLines,
			).toEqual([1]);
			const other = "app/src-tauri/src/commands/other.rs";
			expect(
				evaluate(lcov.replace(file, other), source, other).metadataExceptions,
			).toEqual([]);
			expect(
				evaluate(lcov.replace(file, other), source, other).failures[0]
					.uncoveredLines,
			).toEqual([1]);
		});

		it("does not list unchanged annotation lines as patch exceptions", () => {
			expect(evaluate(lcov, source, file, [3])).toEqual({
				failures: [],
				executableLines: 1,
				metadataExceptions: [],
			});
		});
	});
});
