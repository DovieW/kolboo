import { it } from "vitest";

const IMPORT_HEAVY_TEST_TIMEOUT_MS = 15_000;

// These suites repeatedly reset modules and dynamically import the Tauri facade,
// which is intentionally heavier than normal unit-test setup on slower Windows runs.
export const itWithImportTimeout = (
	name: string,
	testFn: () => Promise<void> | void,
) => it(name, { timeout: IMPORT_HEAVY_TEST_TIMEOUT_MS }, testFn);
