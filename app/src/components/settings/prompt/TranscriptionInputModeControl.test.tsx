// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { TranscriptionInputModeControl } from "./TranscriptionInputModeControl";

Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
it("offers inheritance and both input modes without selecting an incompatible realtime-only mode", async () => {
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const change = vi.fn();
	async function render(
		props: Partial<Parameters<typeof TranscriptionInputModeControl>[0]> = {},
	) {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<TranscriptionInputModeControl
						value={null}
						inheritedMode="auto"
						onChange={change}
						{...props}
					/>
				</MantineProvider>,
			),
		);
	}
	async function choose(label: string) {
		const input = host.querySelector<HTMLInputElement>("input");
		expect(input).not.toBeNull();
		await act(async () => input?.click());
		const option = [
			...document.querySelectorAll<HTMLElement>('[role="option"]'),
		].find((e) => e.textContent === label);
		expect(option).toBeDefined();
		await act(async () => option?.click());
	}
	await render();
	expect(host.querySelector('.settings-row .settings-label')?.textContent).toBe('Input mode');
	expect(host.querySelector('.settings-row-actions .settings-input-mode-control')).not.toBeNull();
	expect(host.querySelector<HTMLInputElement>("input")?.value).toBe("Auto");
	await choose("After recording");
	expect(change).toHaveBeenLastCalledWith("after_recording");
	await render({ value: "after_recording" });
	await choose("Auto");
	expect(change).toHaveBeenLastCalledWith("auto");
	await render({ canInherit: true, inheritedMode: "after_recording" });
	expect(host.querySelector<HTMLInputElement>("input")?.value).toBe(
		"Inherit (After recording)",
	);
	await render({
		canInherit: true,
		inheritedMode: "after_recording",
		value: "auto",
	});
	await choose("Inherit (After recording)");
	expect(change).toHaveBeenLastCalledWith(null);
	await render({ canInherit: true, inheritedMode: "auto", realtimeOnly: true });
	expect(host.textContent).toContain("requires realtime input");
	change.mockClear();
	await choose("After recording");
	expect(change).not.toHaveBeenCalled();
	await render({
		value: "after_recording",
		realtimeOnly: true,
		disabled: true,
	});
	expect(host.querySelector<HTMLInputElement>("input")?.disabled).toBe(true);
	expect(host.textContent).toContain("change the model or choose Auto");
	await act(async () => root.unmount());
	host.remove();
});
