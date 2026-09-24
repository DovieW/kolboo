import { beforeEach, describe, expect, it, vi } from "vitest";
import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { runPatchCoverageCli } from "./patch-coverage.mjs";

vi.mock("node:fs", () => ({
	existsSync: vi.fn(() => true),
	readFileSync: vi.fn(),
}));
vi.mock("node:child_process", () => ({ spawnSync: vi.fn() }));

describe("patch gate CLI metadata exception", () => {
	const file = "app/src-tauri/src/commands/history.rs";
	let changedFile: string;
	let sourceAvailable: boolean;

	beforeEach(() => {
		vi.restoreAllMocks();
		changedFile = file;
		sourceAvailable = true;
		vi.spyOn(console, "log").mockImplementation(() => {});
		vi.spyOn(console, "error").mockImplementation(() => {});
		vi.mocked(spawnSync).mockImplementation(
			(_command, args) =>
				({
					status: 0,
					stderr: "",
					stdout:
						args?.[0] === "diff"
							? `+++ b/${changedFile}\n@@ -0,0 +1,3 @@\n+annotation\n+function\n+body`
							: "base-sha",
				}) as ReturnType<typeof spawnSync>,
		);
		vi.mocked(readFileSync).mockImplementation((name) => {
			const path = String(name);
			if (path.endsWith("patch-coverage-exceptions.json")) return "[]";
			if (path.endsWith("/coverage/lcov.info")) return "";
			if (path.endsWith("/coverage/rust-lcov.info"))
				return `SF:${changedFile.replace(/^app\//, "")}
FN:1,_RNCwrapper
FN:2,_RNvget_history_activity
FNDA:0,_RNCwrapper
FNDA:1,_RNvget_history_activity
DA:1,0
DA:2,1
DA:3,1
end_of_record`;
			if (path.endsWith(file) && sourceAvailable)
				return "#[tauri::command]\npub async fn get_history_activity(\nbody";
			throw new Error("Source unavailable");
		});
	});

	it("loads source evidence and explicitly reports the exception", () => {
		expect(runPatchCoverageCli(["--force", "--base", "HEAD"])).toBe(0);
		expect(console.log).toHaveBeenCalledWith(
			`Approved Tauri metadata exception: ${file}:1 (command body remains enforced).`,
		);
		expect(console.log).toHaveBeenCalledWith(
			"Patch coverage: 100% (2 changed executable lines).",
		);
	});

	it("fails when the approved source cannot be read", () => {
		sourceAvailable = false;
		expect(runPatchCoverageCli(["--force"])).toBe(1);
		expect(console.error).toHaveBeenCalledWith("Source unavailable");
	});

	it("does not load exception sources or waive failures for other commands", () => {
		changedFile = "app/src-tauri/src/commands/other.rs";
		vi.mocked(readFileSync).mockClear();
		expect(runPatchCoverageCli(["--force"])).toBe(1);
		expect(readFileSync).toHaveBeenCalledTimes(3);
		expect(console.error).toHaveBeenCalledWith(
			`- ${changedFile}: uncovered lines 1`,
		);
	});

	it("includes untracked production files and fails when they have no report", () => {
		const git = vi.mocked(spawnSync).getMockImplementation();
		if (!git) throw new Error("Missing git fixture");
		vi.mocked(spawnSync).mockImplementation((command, args, options) =>
			args?.[0] === "ls-files"
				? ({
						status: 0,
						stderr: "",
						stdout: "app/src/new-feature.ts\napp/src/new-feature.test.ts",
					} as ReturnType<typeof spawnSync>)
				: git(command, args, options),
		);
		const read = vi.mocked(readFileSync).getMockImplementation();
		if (!read) throw new Error("Missing filesystem fixture");
		vi.mocked(readFileSync).mockImplementation((name, options) =>
			String(name).endsWith("app/src/new-feature.ts")
				? "export const feature = () => 42;\n"
				: read(name, options),
		);
		expect(runPatchCoverageCli([])).toBe(1);
		expect(console.error).toHaveBeenCalledWith(
			"- app/src/new-feature.ts: missing from coverage reports",
		);
		expect(console.error).not.toHaveBeenCalledWith(
			expect.stringContaining("new-feature.test.ts"),
		);
	});

	it("does not bootstrap-skip a branch that already contains the coverage policy", () => {
		vi.mocked(spawnSync).mockImplementation(
			(_command, args) =>
				({
					status:
						args?.[0] === "cat-file" && String(args[2]).startsWith("old-base:")
							? 1
							: 0,
					stderr: "",
					stdout:
						args?.[0] === "merge-base"
							? "old-base"
							: args?.[0] === "log"
								? "policy-added"
								: "",
				}) as ReturnType<typeof spawnSync>,
		);
		vi.mocked(existsSync).mockReturnValueOnce(false);
		expect(runPatchCoverageCli([])).toBe(1);
		expect(console.error).toHaveBeenCalledWith(
			expect.stringContaining("Missing coverage report"),
		);
		expect(console.log).toHaveBeenCalledWith(
			"Patch coverage baseline: policy-added (requested origin/master).",
		);
	});
});
