// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act, useState } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import {
	CustomModelEditor,
	customModelMetadata,
	customModelRows,
} from "./CustomModelEditor";

Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);

it("keeps raw IDs separate from optional plain-text names and validates metadata", () => {
	expect(customModelRows(["raw"])).toEqual([{ id: "raw", name: "" }]);
	expect(customModelRows(["raw"], { raw: undefined })).toEqual([
		{ id: "raw", name: "" },
	]);
	expect(customModelRows(["raw", "toString"], { raw: "Friendly" })).toEqual([
		{ id: "raw", name: "Friendly" },
		{ id: "toString", name: "" },
	]);
	expect(
		customModelMetadata([
			{ id: " raw ", name: " <b>Name</b> " },
			{ id: "other", name: " " },
		]),
	).toEqual({ ids: ["raw", "other"], names: { raw: "<b>Name</b>" } });
	const special = customModelMetadata([{ id: "__proto__", name: "Safe" }]);
	expect(Object.hasOwn(special.names, "__proto__")).toBe(true);
	expect(
		Object.getOwnPropertyDescriptor(special.names, "__proto__")?.value,
	).toBe("Safe");
	for (const rows of [
		[{ id: " ", name: "" }],
		[
			{ id: "same", name: "" },
			{ id: " same ", name: "" },
		],
		[{ id: "x".repeat(201), name: "" }],
		[{ id: "raw", name: "x".repeat(101) }],
		[{ id: "raw\u0000id", name: "" }],
		[{ id: "raw", name: "bad\nname" }],
		[{ id: "raw", name: "bad\u0080name" }],
		[{ id: "raw", name: "bad\u007fname" }],
		Array.from({ length: 101 }, (_, i) => ({ id: `id${i}`, name: "" })),
	])
		expect(() => customModelMetadata(rows)).toThrow();
	expect(customModelMetadata([])).toEqual({ ids: [], names: {} });
	expect(
		customModelMetadata([{ id: "raw", name: "א".repeat(100) }]).names.raw,
	).toBe("א".repeat(100));
});

it("edits rows and bulk-adds IDs without discarding names or introducing duplicates", async () => {
	Object.defineProperty(document, "fonts", {
		configurable: true,
		value: new EventTarget(),
	});
	const host = document.createElement("div");
	document.body.append(host);
	const root = createRoot(host);
	const changed = vi.fn();
	function Harness({ disabled = false }: { disabled?: boolean }) {
		const [rows, setRows] = useState(
			customModelRows(["one", "two"], { one: "Named" }),
		);
		return (
			<CustomModelEditor
				label="Text"
				rows={rows}
				disabled={disabled}
				onChange={(value) => {
					changed(value);
					setRows(value);
				}}
			/>
		);
	}
	const button = (label: string) => {
		const found = [...host.querySelectorAll("button")].find(
			(b) => b.textContent === label || b.getAttribute("aria-label") === label,
		);
		if (!found) throw new Error(`Missing ${label}`);
		return found;
	};
	const input = async (label: string, value: string) => {
		const element = host.querySelector(
			`[aria-label="${label}"]`,
		) as HTMLInputElement;
		await act(async () => {
			Object.getOwnPropertyDescriptor(
				HTMLInputElement.prototype,
				"value",
			)?.set?.call(element, value);
			element.dispatchEvent(new Event("input", { bubbles: true }));
		});
	};
	try {
		await act(async () =>
			root.render(
				<MantineProvider>
					<Harness />
				</MantineProvider>,
			),
		);
		await input("Text model ID 1", "raw-renamed");
		await input("Text display name 1", "<b>Friendly</b>");
		expect(changed.mock.lastCall?.[0]).toEqual([
			{ id: "raw-renamed", name: "<b>Friendly</b>" },
			{ id: "two", name: "" },
		]);
		await act(async () => button("Bulk add text").click());
		const textarea = host.querySelector("textarea") as HTMLTextAreaElement;
		await act(async () => {
			Object.getOwnPropertyDescriptor(
				HTMLTextAreaElement.prototype,
				"value",
			)?.set?.call(textarea, "two\n three \nthree\n\n");
			textarea.dispatchEvent(new Event("input", { bubbles: true }));
		});
		await act(async () => button("Add IDs").click());
		expect(changed.mock.lastCall?.[0]).toHaveLength(3);
		expect(changed.mock.lastCall?.[0][0].name).toBe("<b>Friendly</b>");
		expect(host.querySelector("b")).toBeNull();
		await act(async () => button("Remove text model 2").click());
		expect(
			changed.mock.lastCall?.[0].map((row: { id: string }) => row.id),
		).toEqual(["raw-renamed", "three"]);
		await act(async () => button("Add text model").click());
		expect(changed.mock.lastCall?.[0][2]).toEqual({ id: "", name: "" });
		await act(async () => button("Bulk add text").click());
		await act(async () => button("Close bulk entry").click());
		expect(host.querySelector("textarea")).toBeNull();
		await act(async () =>
			root.render(
				<MantineProvider>
					<Harness disabled />
				</MantineProvider>,
			),
		);
		for (const control of host.querySelectorAll("button,input"))
			expect(control.hasAttribute("disabled")).toBe(true);
	} finally {
		await act(async () => root.unmount());
		host.remove();
	}
});
