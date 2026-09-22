#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "../..");
const APP_ROOT = path.join(REPO_ROOT, "app");
const POLICY_MARKER = ".coverage-patch-v1";

const EXCLUDED_SOURCE_FILES = new Set([
	"app/src/main.tsx",
	"app/src/overlay-main.tsx",
	"app/src/overlay-hover-main.tsx",
	"app/src/quick-ask-main.tsx",
	"app/src/lib/tauri/events.generated.ts",
	"app/src/lib/tauri/types.generated.ts",
	"app/src/lib/tauri/types.ts",
]);

function normalizeRepoPath(filePath, sourceRoot, repoRoot = REPO_ROOT) {
	const absolute = path.isAbsolute(filePath)
		? filePath
		: path.resolve(sourceRoot, filePath);
	return path.relative(repoRoot, absolute).replaceAll(path.sep, "/");
}

function emptyCoverage() {
	return {
		lines: new Map(),
		branches: new Map(),
		functions: new Map(),
	};
}

function addCount(map, key, count) {
	map.set(key, (map.get(key) ?? 0) + count);
}

export function parseLcov(text, { sourceRoot, repoRoot = REPO_ROOT }) {
	const files = new Map();
	let current = null;
	let functionLines = new Map();

	for (const rawLine of text.split(/\r?\n/u)) {
		if (rawLine.startsWith("SF:")) {
			const filePath = normalizeRepoPath(
				rawLine.slice(3),
				sourceRoot,
				repoRoot,
			);
			current = files.get(filePath) ?? emptyCoverage();
			files.set(filePath, current);
			functionLines = new Map();
		} else if (rawLine === "end_of_record") {
			current = null;
			functionLines = new Map();
		} else if (current && rawLine.startsWith("DA:")) {
			const [line, count] = rawLine.slice(3).split(",").map(Number);
			if (Number.isFinite(line) && Number.isFinite(count)) {
				addCount(current.lines, line, count);
			}
		} else if (current && rawLine.startsWith("BRDA:")) {
			const [lineText, block, branch, takenText] = rawLine
				.slice(5)
				.split(",");
			const line = Number(lineText);
			const taken = takenText === "-" ? 0 : Number(takenText);
			if (Number.isFinite(line) && Number.isFinite(taken)) {
				addCount(current.branches, `${line}:${block}:${branch}`, taken);
			}
		} else if (current && rawLine.startsWith("FN:")) {
			const separator = rawLine.indexOf(",", 3);
			const line = Number(rawLine.slice(3, separator));
			const name = rawLine.slice(separator + 1);
			if (separator > 3 && Number.isFinite(line)) functionLines.set(name, line);
		} else if (current && rawLine.startsWith("FNDA:")) {
			const separator = rawLine.indexOf(",", 5);
			const count = Number(rawLine.slice(5, separator));
			const name = rawLine.slice(separator + 1);
			const line = functionLines.get(name);
			if (line !== undefined && Number.isFinite(count)) {
				addCount(current.functions, `${line}:${name}`, count);
			}
		}
	}

	return files;
}

export function mergeCoverageReports(reports) {
	const merged = new Map();
	for (const report of reports) {
		for (const [filePath, coverage] of report) {
			const target = merged.get(filePath) ?? emptyCoverage();
			merged.set(filePath, target);
			for (const [line, count] of coverage.lines) {
				addCount(target.lines, line, count);
			}
			for (const [branch, count] of coverage.branches) {
				addCount(target.branches, branch, count);
			}
			for (const [fn, count] of coverage.functions) {
				addCount(target.functions, fn, count);
			}
		}
	}
	return merged;
}

export function parseChangedLines(diff) {
	const changed = new Map();
	let currentPath = null;

	for (const line of diff.split(/\r?\n/u)) {
		if (line.startsWith("+++ ")) {
			const diffPath = line.slice(4).split("\t", 1)[0];
			if (diffPath === "/dev/null") {
				currentPath = null;
				continue;
			}
			currentPath = diffPath.startsWith("app/")
				? diffPath
				: diffPath.slice(diffPath.indexOf("/") + 1);
			if (!changed.has(currentPath)) changed.set(currentPath, new Set());
			continue;
		}
		if (!currentPath || !line.startsWith("@@")) continue;
		const match = line.match(/\+(\d+)(?:,(\d+))?/u);
		if (!match) continue;
		const start = Number(match[1]);
		const count = match[2] === undefined ? 1 : Number(match[2]);
		for (let offset = 0; offset < count; offset += 1) {
			changed.get(currentPath)?.add(start + offset);
		}
	}

	return changed;
}

export function isCoverageSource(filePath) {
	if (EXCLUDED_SOURCE_FILES.has(filePath)) return false;
	if (/\.d\.ts$/u.test(filePath)) return false;
	if (/\.(?:test|spec)\.(?:ts|tsx)$/u.test(filePath)) return false;
	if (/\/tests?\//u.test(filePath) || /\/tests\.rs$/u.test(filePath)) {
		return false;
	}
	return (
		/^app\/src\/.*\.(?:ts|tsx)$/u.test(filePath) ||
		/^app\/src-tauri\/src\/.*\.rs$/u.test(filePath)
	);
}

function failedLinesByPrefix(values, changedLines) {
	const failures = new Set();
	for (const [identity, count] of values) {
		const line = Number(String(identity).split(":", 1)[0]);
		if (changedLines.has(line) && count === 0) failures.add(line);
	}
	return [...failures].sort((a, b) => a - b);
}

export function evaluatePatchCoverage(changed, coverage) {
	const failures = [];
	let executableLines = 0;

	for (const [filePath, changedLines] of changed) {
		if (!isCoverageSource(filePath)) continue;
		const fileCoverage = coverage.get(filePath);
		if (!fileCoverage) {
			failures.push({ filePath, missingReport: true });
			continue;
		}

		const uncoveredLines = [];
		for (const line of changedLines) {
			if (!fileCoverage.lines.has(line)) continue;
			executableLines += 1;
			if (fileCoverage.lines.get(line) === 0) uncoveredLines.push(line);
		}
		const uncoveredBranches = failedLinesByPrefix(
			fileCoverage.branches,
			changedLines,
		);
		const uncoveredFunctions = failedLinesByPrefix(
			fileCoverage.functions,
			changedLines,
		);

		if (
			uncoveredLines.length > 0 ||
			uncoveredBranches.length > 0 ||
			uncoveredFunctions.length > 0
		) {
			failures.push({
				filePath,
				missingReport: false,
				uncoveredLines,
				uncoveredBranches,
				uncoveredFunctions,
			});
		}
	}

	return { failures, executableLines };
}

function runGit(args) {
	const result = spawnSync("git", args, {
		cwd: REPO_ROOT,
		encoding: "utf8",
		maxBuffer: 50 * 1024 * 1024,
	});
	if (result.status !== 0) {
		throw new Error(result.stderr.trim() || `git ${args.join(" ")} failed`);
	}
	return result.stdout.trim();
}

function hasPolicyMarker(base) {
	return (
		spawnSync("git", ["cat-file", "-e", `${base}:${POLICY_MARKER}`], {
			cwd: REPO_ROOT,
			stdio: "ignore",
		}).status === 0
	);
}

function parseArgs(argv) {
	const options = {
		base: process.env.COVERAGE_BASE ?? "origin/master",
		force: false,
		frontendLcov: path.join(APP_ROOT, "coverage/lcov.info"),
		rustLcov: path.join(APP_ROOT, "coverage/rust-lcov.info"),
	};
	const args = [...argv];
	while (args.length > 0) {
		const arg = args.shift();
		if (arg === "--base") options.base = args.shift() ?? options.base;
		else if (arg === "--force") options.force = true;
		else if (arg === "--frontend-lcov") {
			options.frontendLcov = path.resolve(args.shift() ?? options.frontendLcov);
		} else if (arg === "--rust-lcov") {
			options.rustLcov = path.resolve(args.shift() ?? options.rustLcov);
		}
	}
	return options;
}

export function runPatchCoverageCli(argv = process.argv.slice(2)) {
	try {
		const options = parseArgs(argv);
		if (!options.force && !hasPolicyMarker(options.base)) {
			console.log(
				`Patch coverage bootstrap: ${POLICY_MARKER} is not present on ${options.base}; enforcement begins after this policy lands.`,
			);
			return 0;
		}

		for (const reportPath of [options.frontendLcov, options.rustLcov]) {
			if (!existsSync(reportPath)) {
				throw new Error(`Missing coverage report: ${reportPath}`);
			}
		}

		const mergeBase = runGit(["merge-base", options.base, "HEAD"]);
		const diff = runGit([
			"diff",
			"--unified=0",
			"--diff-filter=AMCR",
			mergeBase,
			"--",
			"app/src",
			"app/src-tauri/src",
		]);
		const changed = parseChangedLines(diff);
		const coverage = mergeCoverageReports([
			parseLcov(readFileSync(options.frontendLcov, "utf8"), {
				sourceRoot: APP_ROOT,
			}),
			parseLcov(readFileSync(options.rustLcov, "utf8"), {
				sourceRoot: APP_ROOT,
			}),
		]);
		const result = evaluatePatchCoverage(changed, coverage);

		if (result.failures.length === 0) {
			console.log(
				`Patch coverage: 100% (${result.executableLines} changed executable lines).`,
			);
			return 0;
		}

		console.error("Patch coverage must be 100% for changed executable code:");
		for (const failure of result.failures) {
			if (failure.missingReport) {
				console.error(`- ${failure.filePath}: missing from coverage reports`);
				continue;
			}
			if (failure.uncoveredLines.length > 0) {
				console.error(
					`- ${failure.filePath}: uncovered lines ${failure.uncoveredLines.join(", ")}`,
				);
			}
			if (failure.uncoveredBranches.length > 0) {
				console.error(
					`- ${failure.filePath}: uncovered branches on lines ${failure.uncoveredBranches.join(", ")}`,
				);
			}
			if (failure.uncoveredFunctions.length > 0) {
				console.error(
					`- ${failure.filePath}: uncovered functions on lines ${failure.uncoveredFunctions.join(", ")}`,
				);
			}
		}
		return 1;
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		return 1;
	}
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	process.exit(runPatchCoverageCli());
}
