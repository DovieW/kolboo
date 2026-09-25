// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { UsageStatsView } from "./UsageStatsView";

const state = vi.hoisted(() => ({
	license: undefined as undefined | { status: string; tier: string },
}));
vi.mock("../../lib/queries/license", () => ({
	useLicenseState: () => ({ data: state.license }),
}));
vi.mock("../../lib/modelOptions", () => ({
	listAllSttModelKeys: () => [{ key: "stt-key", label: "Speech model" }],
	listAllLlmModelKeys: () => [{ key: "llm-key", label: "Text model" }],
}));
vi.mock("./ActivityPanel", () => ({
	ActivityPanel: ({
		timeframe,
		modelsOnly,
	}: {
		timeframe: string;
		modelsOnly: boolean;
	}) => (
		<div>
			{modelsOnly ? "Models data" : "Activity data"} {timeframe}
		</div>
	),
}));
const cost = vi.hoisted(() => vi.fn());
vi.mock("./CostTab", () => ({
	CostTab: (props: unknown) => {
		cost(props);
		return <div>Cost estimates</div>;
	},
}));
let host: HTMLDivElement;
let root: Root;
async function render() {
	await act(async () =>
		root.render(
			<MantineProvider env="test">
				<UsageStatsView />
			</MantineProvider>,
		),
	);
}
async function clickTab(name: string) {
	await act(async () =>
		[...host.querySelectorAll<HTMLElement>('[role="tab"]')]
			.find((tab) => tab.textContent === name)
			?.click(),
	);
}
beforeEach(() => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	state.license = undefined;
	cost.mockClear();
});
afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
});
it("shows activity during account loading without mounting or flashing cost data", async () => {
	await render();
	expect(host.textContent).toContain("Activity data 30d");
	expect(host.textContent).not.toContain("Spend");
	expect(cost).not.toHaveBeenCalled();
	await clickTab("Models");
	expect(host.textContent).toContain("Models data");
	expect(host.textContent).not.toContain("Activity data");
});
it("allows Community spend but removes it immediately when managed access arrives", async () => {
	state.license = { status: "signed_out", tier: "community" };
	await render();
	await clickTab("Spend");
	expect(host.textContent).toContain("Cost estimates");
	expect(cost).toHaveBeenLastCalledWith({
		timeframe: "30d",
		kind: "all",
		sttModelKeys: [],
		llmModelKeys: [],
		excludeFreeTier: true,
	});
	state.license = { status: "authenticated", tier: "personal" };
	cost.mockClear();
	await render();
	expect(host.textContent).not.toContain("Spend");
	expect(host.textContent).not.toContain("Cost estimates");
	expect(host.textContent).toContain("Activity data");
	expect(cost).not.toHaveBeenCalled();
	state.license = { status: "authenticated", tier: "enterprise" };
	await render();
	expect(host.textContent).not.toContain("Spend");
	state.license = { status: "authenticated", tier: "community" };
	await render();
	expect(host.textContent).toContain("Spend");
});
it("changes the period for every tab", async () => {
	await render();
	await act(async () =>
		host
			.querySelector<HTMLInputElement>('input[aria-label="Usage period"]')
			?.click(),
	);
	await act(async () =>
		[...document.querySelectorAll<HTMLElement>('[role="option"]')]
			.find((option) => option.textContent === "All time")
			?.click(),
	);
	expect(host.textContent).toContain("Activity data all");
	await clickTab("Models");
	expect(host.textContent).toContain("Models data all");
});
it("applies and resets spend filters rather than only changing their appearance", async () => {
	state.license = { status: "signed_out", tier: "community" };
	await render();
	await clickTab("Spend");
	await act(async () =>
		host.querySelector<HTMLButtonElement>('[aria-label="Filters"]')?.click(),
	);
	await act(async () =>
		document.querySelector<HTMLInputElement>('input[value="stt"]')?.click(),
	);
	expect(cost).toHaveBeenLastCalledWith(
		expect.objectContaining({ kind: "stt" }),
	);
	await act(async () =>
		document.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(),
	);
	expect(cost).toHaveBeenLastCalledWith(
		expect.objectContaining({ excludeFreeTier: false }),
	);
	const inputs = [
		...document.querySelectorAll<HTMLInputElement>(
			'input[placeholder="All models"]',
		),
	];
	for (const [index, input] of inputs.entries()) {
		await act(async () => input.click());
		await act(async () =>
			[...document.querySelectorAll<HTMLElement>('[role="option"]')]
				.find(
					(option) =>
						option.textContent ===
						(index === 0 ? "Speech model" : "Text model"),
				)
				?.click(),
		);
	}
	expect(cost).toHaveBeenLastCalledWith(
		expect.objectContaining({
			sttModelKeys: expect.arrayContaining([expect.any(String)]),
			llmModelKeys: expect.arrayContaining([expect.any(String)]),
		}),
	);
	await act(async () =>
		[...document.querySelectorAll("button")]
			.find((button) => button.textContent === "Reset")
			?.click(),
	);
	expect(cost).toHaveBeenLastCalledWith({
		timeframe: "30d",
		kind: "all",
		sttModelKeys: [],
		llmModelKeys: [],
		excludeFreeTier: true,
	});
});
