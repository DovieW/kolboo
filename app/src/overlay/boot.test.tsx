// @vitest-environment happy-dom
import { act } from "react";
import { expect, it, vi } from "vitest";

const boot = vi.hoisted(() => ({
	config: Promise.withResolvers<void>(),
	diagnostics: Promise.withResolvers<boolean>(),
}));
vi.mock("../lib/telemetry/sentry", () => ({
	initSentry: () => boot.diagnostics.promise,
	getSentryReactRootOptions: () => undefined,
}));
vi.mock("@tauri-apps/api/core", () => ({
	invoke: (command: string) =>
		command === "sync_pipeline_config"
			? boot.config.promise
			: Promise.resolve(),
}));
vi.mock("../hooks/useModifierKeyForwarder", () => ({
	useModifierKeyForwarder: () => {},
}));
vi.mock("./RecordingControl", () => ({
	default: () => <button type="button">Ready to record</button>,
}));

it("mounts and presents the recording control without waiting for diagnostics setup", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	document.body.innerHTML = '<div id="root"></div>';
	await act(async () => {
		await import("../overlay-main");
	});
	expect(
		document.querySelector('[role="status"]')?.getAttribute("aria-label"),
	).toBe("Starting overlay");
	await act(async () => {
		boot.config.resolve();
	});
	expect(document.querySelector("button")?.textContent).toBe("Ready to record");
	boot.diagnostics.resolve(false);
});
