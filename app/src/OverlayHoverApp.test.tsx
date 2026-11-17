// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import OverlayHoverApp from "./OverlayHoverApp";

const native = vi.hoisted(() => ({
	visible: false,
	settings: undefined as unknown,
	invoke: vi.fn(),
	hide: vi.fn().mockResolvedValue(undefined),
	show: vi.fn(),
	schedule: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("./hooks/useModifierKeyForwarder", () => ({
	useModifierKeyForwarder: () => {},
}));
vi.mock("./overlay/useHoverVisibility", () => ({
	useHoverVisibility: () => native.visible,
}));
vi.mock("./lib/queries", () => ({
	useSettings: () => ({ data: native.settings }),
}));
vi.mock("./lib/tauri", () => ({
	tauriAPI: {
		hideOverlayHover: native.hide,
		showOverlayHover: native.show,
		scheduleHideOverlayHover: native.schedule,
	},
}));
const host = document.createElement("div");
let root = createRoot(host);
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
afterEach(async () => {
	await act(async () => root.unmount());
	root = createRoot(host);
	vi.useRealTimers();
	vi.clearAllMocks();
	native.visible = false;
	native.settings = undefined;
});
const settings = {
	rewrite_llm_enabled: true,
	rewrite_program_prompt_profiles: [
		{
			id: "default",
			router: { enabled: true, strategy: "auto" },
			presets: [{ id: "concise", name: "Concise" }],
		},
	],
};

it("does no foreground/session polling while hidden, hydrates without self-hiding, and refreshes on show", async () => {
	vi.useFakeTimers();
	native.settings = settings;
	native.invoke.mockImplementation(async (command) =>
		command === "pipeline_get_active_profile_for_foreground_app"
			? { profile_id: "default", profile_name: "Default" }
			: { profile_id: "default", preset_id: "concise" },
	);
	await act(async () => root.render(<OverlayHoverApp />));
	await act(async () => vi.advanceTimersByTimeAsync(5000));
	expect(native.invoke).not.toHaveBeenCalled();
	expect(native.hide).not.toHaveBeenCalled();
	native.visible = true;
	await act(async () => root.render(<OverlayHoverApp />));
	expect(host.textContent).toContain("Concise");
	expect(native.hide).not.toHaveBeenCalled();
	expect(native.invoke).toHaveBeenCalledTimes(2);
	await act(async () => vi.advanceTimersByTimeAsync(1500));
	expect(native.invoke).toHaveBeenCalledTimes(5);
	native.visible = false;
	await act(async () => root.render(<OverlayHoverApp />));
	native.invoke.mockClear();
	await act(async () => vi.advanceTimersByTimeAsync(5000));
	expect(native.invoke).not.toHaveBeenCalled();
	native.visible = true;
	await act(async () => root.render(<OverlayHoverApp />));
	expect(native.invoke).toHaveBeenCalledTimes(2);
});

it("distinguishes unknown hydration from an explicitly ineligible foreground profile", async () => {
	let resolveProfile!: (profile: unknown) => void;
	native.visible = true;
	native.invoke.mockImplementation((command) =>
		command === "pipeline_get_active_profile_for_foreground_app"
			? new Promise((resolve) => {
					resolveProfile = resolve;
				})
			: Promise.resolve({ profile_id: null, preset_id: null }),
	);
	await act(async () => root.render(<OverlayHoverApp />));
	expect(native.hide).not.toHaveBeenCalled();
	native.settings = settings;
	await act(async () => root.render(<OverlayHoverApp />));
	expect(native.hide).not.toHaveBeenCalled();
	await act(async () =>
		resolveProfile({ profile_id: "unconfigured", profile_name: null }),
	);
	expect(native.hide).toHaveBeenCalledOnce();
});

it("ignores requests completing after native hide", async () => {
	const completions: Array<(value: unknown) => void> = [];
	native.visible = true;
	native.settings = settings;
	native.invoke.mockImplementation(
		() =>
			new Promise((resolve) => {
				completions.push(resolve);
			}),
	);
	await act(async () => root.render(<OverlayHoverApp />));
	expect(completions).toHaveLength(2);
	native.visible = false;
	await act(async () => root.render(<OverlayHoverApp />));
	await act(async () => {
		completions[0]?.({ profile_id: "unconfigured" });
		completions[1]?.({ profile_id: "default", preset_id: "concise" });
	});
	expect(native.hide).not.toHaveBeenCalled();
	expect(host.textContent).toBe("");
});
