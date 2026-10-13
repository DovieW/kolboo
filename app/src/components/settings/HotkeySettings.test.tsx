// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import {
	createHotkeyDuplicateSchema,
	HotkeyShortcutCardSchema,
} from "../../lib/hotkeys";
import { HotkeySettings } from "./HotkeySettings";

const state = vi.hoisted(() => ({ create: vi.fn() }));
vi.mock("../../lib/queries", () => ({
	useSettings: () => ({
		data: {
			hotkey_shortcuts: [
				{
					id: "saved-open",
					type: "show_window",
					hotkey: { modifiers: ["ctrl"], key: "KeyK" },
				},
			],
		},
		isLoading: false,
	}),
	useCreateHotkeyShortcutCard: () => ({
		mutate: state.create,
		isPending: false,
	}),
	useUpdateHotkeyShortcutCard: () => ({ mutate: vi.fn(), isPending: false }),
	useDeleteHotkeyShortcutCard: () => ({ mutate: vi.fn(), isPending: false }),
	useResetHotkeysToDefaults: () => ({ mutate: vi.fn(), isPending: false }),
}));

it("renders a saved Open Kolboo card and lets users add another unbound window shortcut", async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	try {
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HotkeySettings />
				</MantineProvider>,
			),
		);
		expect(host.textContent).toContain("Show and focus the main window");
		const select = host.querySelector<HTMLInputElement>(
			'input[placeholder="Choose shortcut type"]',
		);
		if (!select) throw new Error("Missing shortcut type selector");
		await act(async () => select.click());
		const option = [
			...document.querySelectorAll<HTMLElement>('[role="option"]'),
		].find((el) => el.textContent === "Open Kolboo");
		if (!option) throw new Error("Missing Open Kolboo option");
		await act(async () => option.click());
		const add = [...host.querySelectorAll("button")].find(
			(el) => el.textContent === "Add shortcut",
		);
		if (!add) throw new Error("Missing add shortcut button");
		await act(async () => add.click());
		expect(state.create).toHaveBeenCalledWith({
			id: expect.any(String),
			type: "show_window",
			hotkey: null,
		});
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});

it("accepts window cards and rejects a duplicate window shortcut without needing a legacy setting", () => {
	const hotkey = { modifiers: ["ctrl"], key: "KeyK" };
	expect(
		HotkeyShortcutCardSchema.parse({ id: "open", type: "show_window", hotkey }),
	).toEqual({ id: "open", type: "show_window", hotkey });
	const result = createHotkeyDuplicateSchema(
		{ show_window: hotkey },
		"toggle",
	).safeParse(hotkey);
	expect(result.success).toBe(false);
	if (result.success) throw new Error("Duplicate accepted");
	expect(result.error.issues[0]?.message).toContain("open Kolboo");
	expect(
		createHotkeyDuplicateSchema(
			{ show_window: hotkey },
			"show_window",
		).safeParse(hotkey).success,
	).toBe(true);
});
