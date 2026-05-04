import path from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import {
	buildRustCoverageArgs,
	conservativeCargoJobs,
	createRustCoverageEnvironment,
	isCliEntrypoint,
	validateRustCoverageOptions,
} from "./rust-coverage.mjs";

describe("rust coverage helper", () => {
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

	it("preserves an existing cargo job limit", () => {
		const env = createRustCoverageEnvironment({ CARGO_BUILD_JOBS: "3" });

		expect(env.CARGO_BUILD_JOBS).toBe("3");
	});

	it("documents missing-tool validation guidance", () => {
		expect(validateRustCoverageOptions()).toContain(
			"cargo llvm-cov must be installed before Rust in-scope coverage can be claimed.",
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
