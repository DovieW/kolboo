#!/usr/bin/env node
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const DEFAULT_THRESHOLD = 100;

export function normalizeMetric(value) {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return 0;
	}

	return Math.max(0, Math.min(100, value));
}

export function normalizeCoverageSummary(rawSummary) {
	const entries = Object.entries(rawSummary ?? {});
	const fileEntries = entries.filter(([filePath, value]) => {
		return filePath !== "total" && value && typeof value === "object";
	});

	return new Map(
		fileEntries.map(([filePath, value]) => [
			filePath.replaceAll("\\", "/"),
			{
				statements: normalizeMetric(value.statements?.pct),
				branches: normalizeMetric(value.branches?.pct),
				functions: normalizeMetric(value.functions?.pct),
			},
		]),
	);
}

export function evaluateCoverage(
	summary,
	inScopeFiles,
	threshold = DEFAULT_THRESHOLD,
) {
	const normalizedSummary = normalizeCoverageSummary(summary);
	const files = inScopeFiles.map((filePath) => filePath.replaceAll("\\", "/"));

	return files.map((filePath) => {
		const metrics = normalizedSummary.get(filePath);
		const missing = !metrics;
		const failures = missing
			? ["missing"]
			: Object.entries(metrics)
					.filter(([, value]) => value < threshold)
					.map(([metric]) => metric);

		return {
			filePath,
			metrics: metrics ?? {
				statements: 0,
				branches: 0,
				functions: 0,
			},
			pass: failures.length === 0,
			failures,
		};
	});
}

export function formatEvidenceRows(results) {
	return results
		.map((result) => {
			const { statements, branches, functions } = result.metrics;
			return `| ${result.filePath} | ${statements}% | ${branches}% | ${functions}% | ${result.pass ? "PASS" : `FAIL (${result.failures.join(", ")})`} |`;
		})
		.join("\n");
}

export function isCliEntrypoint(
	metaUrl,
	argvPath = process.argv[1],
	platform = process.platform,
) {
	if (!argvPath) {
		return false;
	}

	const modulePath = path.resolve(fileURLToPath(metaUrl));
	const invokedPath = path.resolve(argvPath);
	const normalize = (value) =>
		platform === "win32" ? value.toLowerCase() : value;

	return normalize(modulePath) === normalize(invokedPath);
}

function parseArgs(argv) {
	const args = [...argv];
	const options = {
		summaryPath: "coverage/coverage-summary.json",
		files: [],
		threshold: DEFAULT_THRESHOLD,
	};

	while (args.length > 0) {
		const arg = args.shift();
		if (arg === "--summary") {
			options.summaryPath = args.shift() ?? options.summaryPath;
		} else if (arg === "--file") {
			const filePath = args.shift();
			if (filePath) {
				options.files.push(filePath);
			}
		} else if (arg === "--threshold") {
			const value = Number(args.shift());
			if (Number.isFinite(value)) {
				options.threshold = value;
			}
		}
	}

	return options;
}

export function runCoverageEvidenceCli(argv = process.argv.slice(2)) {
	const options = parseArgs(argv);
	if (options.files.length === 0) {
		console.error(
			"No in-scope files supplied. Use --file <path> for each file.",
		);
		return 1;
	}

	const summary = JSON.parse(readFileSync(options.summaryPath, "utf8"));
	const results = evaluateCoverage(summary, options.files, options.threshold);
	console.log("| File | Statements | Branches | Functions | Status |");
	console.log("|------|------------|----------|-----------|--------|");
	console.log(formatEvidenceRows(results));

	return results.every((result) => result.pass) ? 0 : 1;
}

if (isCliEntrypoint(import.meta.url)) {
	process.exit(runCoverageEvidenceCli());
}
