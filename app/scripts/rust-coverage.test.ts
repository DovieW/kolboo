import path from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import {
	buildRustCoverageArgs,
	conservativeCargoJobs,
	createRustCoverageEnvironment,
	isCliEntrypoint,
	validateRustCoverageOptions,
	mergeRustBuildIdentities,
} from "./rust-coverage.mjs";
import { evaluatePatchCoverage, parseLcov } from "./patch-coverage.mjs";

describe("rust coverage helper", () => {
	it("merges build hashes without waiving untested generic variants or changing line coverage", () => {
		const input =
			"SF:app/src-tauri/src/native.rs\nFN:10,_RtestWry\nFN:10,_RappWry\nFN:10,_Rmock\nFN:11,_Rother\nFNDA:1,_RtestWry\nFNDA:0,_RappWry\nFNDA:0,_Rmock\nFNDA:0,_Rother\nDA:10,1\nDA:11,0\nend_of_record";
		const output = mergeRustBuildIdentities(input, (names: string[]) =>
			names.map(
				(name) =>
					({
						_RtestWry: "read::<Wry>::{closure#0}",
						_RappWry: "read::<Wry>::{closure#0}",
						_Rmock: "read::<MockRuntime>::{closure#0}",
						_Rother: "read::<Wry>::{closure#1}",
					})[name],
			),
		);
		const coverage = parseLcov(output, {
			sourceRoot: "/repo",
			repoRoot: "/repo",
		});
		const measured = coverage.get("app/src-tauri/src/native.rs");
		expect(measured.functions.get("10:_RtestWry")).toBe(1);
		expect(measured.functions.has("10:_RappWry")).toBe(false);
		expect(measured.functions.get("10:_Rmock")).toBe(0);
		expect(
			evaluatePatchCoverage(
				new Map([["app/src-tauri/src/native.rs", new Set([10, 11])]]),
				coverage,
			).failures,
		).toEqual([
			expect.objectContaining({
				uncoveredFunctions: [10, 11],
				uncoveredLines: [11],
			}),
		]);
	});
	it("keeps source files and locations separate, even when demangled identities match", () => {
		const input =
			"SF:a.rs\nFN:1,_Ra\nFN:2,_Rb\nFNDA:1,_Ra\nFNDA:0,_Rb\nend_of_record\nSF:b.rs\nFN:1,_Rc\nFNDA:0,_Rc\nend_of_record\nSF:a.rs\nFN:1,_Rd\nFNDA:1,_Rd\nend_of_record";
		expect(
			mergeRustBuildIdentities(input, (names: string[]) =>
				names.map(() => "same"),
			),
		).toContain("FNDA:1,_Ra\nend_of_record");
		expect(
			mergeRustBuildIdentities(input, (names: string[]) =>
				names.map(() => "same"),
			),
		).toContain("FNDA:0,_Rb");
		expect(
			mergeRustBuildIdentities(input, (names: string[]) =>
				names.map(() => "same"),
			),
		).toContain("FNDA:0,_Rc");
	});
	it("fails closed on incomplete definitions or demangling and leaves non-Rust records unchanged", () => {
		expect(
			mergeRustBuildIdentities("FN:1,ordinary\nFNDA:0,ordinary", () => {
				throw new Error("not called");
			}),
		).toBe("FN:1,ordinary\nFNDA:0,ordinary");
		for (const decoded of [[], [""]])
			expect(() => mergeRustBuildIdentities("FN:1,_Ra", () => decoded)).toThrow(
				"Incomplete Rust coverage demangling",
			);
		expect(() =>
			mergeRustBuildIdentities("FN:1,_Ra\nFNDA:1,_Rb", () => ["a"]),
		).toThrow("lacks a definition");
		expect(() =>
			mergeRustBuildIdentities("FNDA:1,_Ra\nFN:1,_Ra", () => ["a"]),
		).toThrow("lacks a source location");
	});
	it.each([
		[1, 1],
		[2, 1],
		[12, 6],
		[64, 8],
	])("chooses conservative cargo jobs for %i CPUs", (cpuCount, expected) => {
		expect(conservativeCargoJobs(cpuCount)).toBe(expected);
	});

	it("builds deterministic cargo llvm-cov arguments", () => {
		expect(
			buildRustCoverageArgs({
				manifestPath: "src-tauri/Cargo.toml",
				packages: ["kolboo"],
				tests: ["pipeline"],
				allFeatures: true,
			}),
		).toEqual([
			"llvm-cov",
			"--manifest-path",
			"src-tauri/Cargo.toml",
			"--summary-only",
			"--package",
			"kolboo",
			"--test",
			"pipeline",
			"--all-features",
		]);
	});

	it("builds an LCOV report for patch coverage", () => {
		expect(
			buildRustCoverageArgs({
				lcov: true,
				outputPath: "coverage/rust-lcov.info",
			}),
		).toEqual([
			"llvm-cov",
			"--manifest-path",
			"src-tauri/Cargo.toml",
			"--lcov",
			"--output-path",
			"coverage/rust-lcov.info",
		]);
	});

	it("preserves an existing cargo job limit", () => {
		const env = createRustCoverageEnvironment({ CARGO_BUILD_JOBS: "3" });

		expect(env.CARGO_BUILD_JOBS).toBe("3");
	});

	it("documents missing-tool validation guidance", () => {
		expect(validateRustCoverageOptions()).toContain(
			"cargo llvm-cov must be installed before Rust in-scope coverage can be claimed.",
		);
		expect(validateRustCoverageOptions()).toContain(
			"Install with: cargo +stable install cargo-llvm-cov --locked --version 0.9.1",
		);
		expect(validateRustCoverageOptions({ requireTool: false })).toEqual([]);
	});

	it("detects direct CLI execution from a file URL", () => {
		const scriptPath = path.join(process.cwd(), "scripts", "rust-coverage.mjs");
		const metaUrl = pathToFileURL(scriptPath).href;

		expect(isCliEntrypoint(metaUrl, scriptPath)).toBe(true);
		expect(
			isCliEntrypoint(
				metaUrl,
				path.join(process.cwd(), "scripts", "not-rust-coverage.mjs"),
			),
		).toBe(false);
	});
});
