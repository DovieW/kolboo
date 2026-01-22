import { describe, expect, it, vi } from "vitest";
import { getRootElementOrThrow } from "./rootElement";

describe("getRootElementOrThrow", () => {
	it("returns the root element when present", () => {
		const fakeRoot = { id: "root" } as unknown as HTMLElement;
		const getElementById = vi.fn((id: string) =>
			id === "root" ? fakeRoot : null,
		);
		vi.stubGlobal("document", { getElementById });

		const el = getRootElementOrThrow();
		expect(el).toBe(fakeRoot);
		expect(getElementById).toHaveBeenCalledWith("root");
	});

	it("supports a custom element id", () => {
		const fakeRoot = { id: "custom" } as unknown as HTMLElement;
		const getElementById = vi.fn((id: string) =>
			id === "custom" ? fakeRoot : null,
		);
		vi.stubGlobal("document", { getElementById });

		const el = getRootElementOrThrow("custom");
		expect(el).toBe(fakeRoot);
		expect(getElementById).toHaveBeenCalledWith("custom");
	});

	it("throws when the element is missing", () => {
		vi.stubGlobal("document", { getElementById: vi.fn(() => null) });
		expect(() => getRootElementOrThrow()).toThrow("Root element not found");
	});
});
