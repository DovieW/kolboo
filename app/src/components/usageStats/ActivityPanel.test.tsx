// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { tauriAPI } from "../../lib/tauri";
import { listenTyped } from "../../lib/tauri/events";
import { ActivityPanel } from "./ActivityPanel";

vi.mock("../../lib/tauri", () => ({
	tauriAPI: { getHistoryActivity: vi.fn() },
}));
vi.mock("../../lib/tauri/events", () => ({ listenTyped: vi.fn() }));
const totals = {
	words: 120,
	audio_seconds: 3600,
	recordings: 3,
	timed_recordings: 2,
	meetings: 1,
};
const result = {
	totals,
	days: [{ date: new Date().toISOString().slice(0, 10), totals }],
	models: [{ provider: "openai", model: "whisper-1", totals }],
};
let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
let changed: () => void;
const stop = vi.fn();
async function settle() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(1);
	});
}
async function render(modelsOnly = false, timeframe: "30d" | "90d" = "30d") {
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<ActivityPanel timeframe={timeframe} modelsOnly={modelsOnly} />
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
}
beforeEach(() => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	vi.mocked(tauriAPI.getHistoryActivity).mockResolvedValue(result);
	vi.mocked(listenTyped).mockImplementation(async (_event, callback) => {
		changed = () => callback(null);
		return stop;
	});
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	vi.useRealTimers();
});
it("groups long periods into bounded date ranges even when live events are unavailable", async () => {
	vi.mocked(listenTyped).mockRejectedValueOnce(new Error("Events unavailable"));
	await render(false, "90d");
	const bars = host.querySelectorAll('[role="img"]');
	expect(bars.length).toBeLessThanOrEqual(45);
	expect(
		[...bars].some((bar) => bar.getAttribute("aria-label")?.includes(" – ")),
	).toBe(true);
	expect(host.textContent).toContain("120");
});
it("displays real totals, switches chart metrics, and refreshes when history changes", async () => {
	await render();
	expect(host.textContent).toContain("120");
	expect(host.textContent).toContain("1h 0m");
	expect(host.textContent).toContain("2 dictations");
	expect(host.textContent).toContain("duration available for 2");
	expect(host.querySelector('[aria-label$="120 words"]')).not.toBeNull();
	await act(async () =>
		host
			.querySelector<HTMLInputElement>('input[value="audio_seconds"]')
			?.click(),
	);
	expect(host.querySelector('[aria-label$="1h 0m audio"]')).not.toBeNull();
	await act(async () =>
		host.querySelector<HTMLInputElement>('input[value="recordings"]')?.click(),
	);
	expect(host.querySelector('[aria-label$="3 recordings"]')).not.toBeNull();
	await act(async () => changed());
	await settle();
	expect(tauriAPI.getHistoryActivity).toHaveBeenCalledTimes(2);
	await render(true);
	expect(host.textContent).toContain("whisper-1");
	expect(host.textContent).not.toContain("Words transcribed");
});
it("does not turn loading or failures into zero activity, and retries failed reads", async () => {
	let finish!: (value: typeof result) => void;
	vi.mocked(tauriAPI.getHistoryActivity).mockImplementationOnce(
		() =>
			new Promise((resolve) => {
				finish = resolve;
			}),
	);
	await render();
	expect(host.textContent).not.toContain("Your activity will appear");
	await act(async () => finish(result));
	await settle();
	expect(host.textContent).toContain("120");
	vi.mocked(tauriAPI.getHistoryActivity).mockRejectedValueOnce(
		new Error("disk read failed"),
	);
	await act(async () => changed());
	await settle();
	expect(host.textContent).toContain("Couldn’t load activity");
	await act(async () =>
		[...host.querySelectorAll("button")]
			.find((el) => el.textContent === "Retry")
			?.click(),
	);
	await settle();
	expect(host.textContent).toContain("120");
	expect(host.textContent).not.toContain("Couldn’t load");
});
it("keeps the empty state useful and cleans up a late event subscription", async () => {
	let subscribe!: (value: () => void) => void;
	vi.mocked(listenTyped).mockImplementationOnce(
		() =>
			new Promise((resolve) => {
				subscribe = resolve;
			}),
	);
	vi.mocked(tauriAPI.getHistoryActivity).mockResolvedValue({
		totals: {
			words: 0,
			audio_seconds: 0,
			recordings: 0,
			timed_recordings: 0,
			meetings: 0,
		},
		days: [],
		models: [],
	});
	await render();
	expect(host.textContent).toContain("Your activity will appear");
	expect(host.textContent).not.toContain("duration available");
	await render(true);
	expect(host.textContent).toContain("No transcriptions");
	await act(async () => root.unmount());
	root = createRoot(host);
	await act(async () => subscribe(stop));
	expect(stop).toHaveBeenCalledOnce();
});
