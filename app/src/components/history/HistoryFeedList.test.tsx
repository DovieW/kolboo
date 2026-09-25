// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { groupHistoryForDisplay } from "../../lib/history/readModel";
import { tauriAPI } from "../../lib/tauri";
import type { RecordingPlayerControls } from "../../lib/useRecordingPlayer";
import { HistoryAudioPlayer } from "./HistoryAudioPlayer";
import { HistoryFeedList } from "./HistoryFeedList";
import { historyTextForCopy } from "./HistoryReader";

const waveformEvents = vi.hoisted(() => new Map<string, () => void>());
vi.mock("wavesurfer.js", () => ({
	default: {
		create: vi.fn(() => ({
			destroy: vi.fn(),
			on: vi.fn((event: string, listener: () => void) => {
				waveformEvents.set(event, listener);
			}),
		})),
	},
}));
vi.mock("../../lib/tauri", () => ({
	tauriAPI: {
		getHistoryDetail: vi.fn(),
		saveHistoryEdit: vi.fn(),
		emitHistoryChanged: vi.fn(async () => {}),
	},
}));
const original = "A complete transcript. ".repeat(100);
let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
const player = {
	id: "one",
	loading: false,
	ready: true,
	playing: false,
	position: 0,
	duration: 100,
	rate: 1,
	waveform: { duration_seconds: 100, peaks: [-1, 1] },
	error: null,
	media: null,
	prepare: vi.fn(async () => {}),
	toggle: vi.fn(async () => {}),
	seek: vi.fn(),
	setRate: vi.fn(),
	stop: vi.fn(),
	isPlaying: () => false,
	isLoading: () => false,
} satisfies RecordingPlayerControls;
const copy = vi.fn();
async function settle() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(1);
	});
}
async function click(element: Element | null) {
	expect(element).not.toBeNull();
	await act(async () => (element as HTMLElement).click());
	await settle();
}
function button(text: string) {
	return (
		[...document.querySelectorAll("button")].find((b) =>
			b.textContent?.includes(text),
		) ?? null
	);
}
beforeEach(async () => {
	vi.useFakeTimers();
	vi.clearAllMocks();
	waveformEvents.clear();
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	vi.mocked(tauriAPI.getHistoryDetail).mockImplementation(async (id) => ({
		entry: {
			id,
			text: original,
			timestamp: "2026-01-01T00:00:00Z",
			status: "success",
		},
		original_text: original,
		revision: 0,
		edited: false,
		edit_error: null,
	}));
	vi.mocked(tauriAPI.saveHistoryEdit).mockImplementation(async (input) => ({
		title: input.title,
		text: input.text,
		revision: input.expected_revision + 1,
	}));
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	await act(async () =>
		root.render(
			<QueryClientProvider client={client}>
				<MantineProvider env="test">
					<HistoryFeedList
						isInitialLoading={false}
						hasError={false}
						emptyState={null}
						groupedHistory={groupHistoryForDisplay(
							["one", "two"].map((id) => ({
								id,
								title: id === "two" ? "Team sync" : null,
								timestamp: "2026-01-01T00:00:00Z",
								text: "Short preview",
								recording_mode:
									id === "two" ? ("meeting" as const) : ("dictation" as const),
							})),
						)}
						copiedEntryId={null}
						onCopyEntry={copy}
						onRetryEntry={vi.fn()}
						isRetryPending={false}
						player={player}
						requestLogIds={new Set()}
						onDeleteEntry={vi.fn()}
						isDeleteDisabled={false}
					/>
				</MantineProvider>
			</QueryClientProvider>,
		),
	);
	await settle();
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	host.remove();
	vi.useRealTimers();
});

describe("History cards and reader", () => {
	it("expands a card instead of copying, loads full text on demand, and only keeps one open", async () => {
		expect(tauriAPI.getHistoryDetail).not.toHaveBeenCalled();
		await click(document.querySelector(".history-preview"));
		expect(document.querySelectorAll("[data-history-detail]")).toHaveLength(1);
		expect(copy).not.toHaveBeenCalled();
		expect(player.toggle).not.toHaveBeenCalled();
		expect(player.prepare).toHaveBeenCalledWith("one");
		expect(
			document.querySelector(".history-transcript-inline")?.textContent,
		).toBe(original);
		await click(document.querySelectorAll(".history-card-toggle")[1] ?? null);
		expect(document.querySelectorAll("[data-history-detail]")).toHaveLength(1);
		expect(document.querySelector("#history-detail-two")).not.toBeNull();
		expect(player.stop).toHaveBeenCalled();
		await click(document.querySelectorAll(".history-card-toggle")[1] ?? null);
		expect(document.querySelector("[data-history-detail]")).toBeNull();
	});
	it("uses quiet mode icons instead of generic recording labels", () => {
		expect(document.body.textContent).not.toContain("Voice recording");
		expect(document.querySelectorAll(".history-preview")).toHaveLength(2);
		expect(document.querySelector(".history-preview--error")).toBeNull();
		expect(
			document.querySelectorAll(".history-kind-icon--dictation"),
		).toHaveLength(1);
		expect(
			document.querySelectorAll(".history-kind-icon--meeting"),
		).toHaveLength(1);
		expect(
			document.querySelector('[aria-label^="Expand dictation from"]'),
		).not.toBeNull();
	});
	it("offers a retry on failed recordings and sends the entry id to the backend", async () => {
		const retry = vi.fn();
		const renderFailed = async (
			isRetryPending = false,
			retryPendingEntryId?: string,
		) =>
			act(async () =>
				root.render(
					<QueryClientProvider client={client}>
						<MantineProvider env="test">
							<HistoryFeedList
								isInitialLoading={false}
								hasError={false}
								emptyState={null}
								groupedHistory={groupHistoryForDisplay([
									{
										id: "failed-attempt",
										recording_request_id: "source-recording",
										timestamp: "2026-01-01T00:00:00Z",
										text: "",
										status: "error",
										error_message: "Transcription timeout after 20s",
									},
								])}
								copiedEntryId={null}
								onCopyEntry={copy}
								onRetryEntry={retry}
								isRetryPending={isRetryPending}
								retryPendingEntryId={retryPendingEntryId}
								player={player}
								requestLogIds={new Set()}
								onDeleteEntry={vi.fn()}
								isDeleteDisabled={false}
							/>
						</MantineProvider>
					</QueryClientProvider>,
				),
			);
		await renderFailed();
		const retryButton = button("Retry") as HTMLButtonElement;
		expect(document.querySelector("[data-history-detail]")).toBeNull();
		expect(retryButton.closest(".history-card-retry")).not.toBeNull();
		expect(retryButton.disabled).toBe(false);
		await click(retryButton);
		expect(retry).toHaveBeenCalledExactlyOnceWith("failed-attempt");
		expect(document.querySelector("[data-history-detail]")).toBeNull();
		await click(document.querySelector(".history-card-toggle"));
		expect(document.querySelector("[data-history-detail]")).not.toBeNull();
		expect(
			[...document.querySelectorAll("button")].filter(
				(b) => b.textContent === "Retry",
			),
		).toHaveLength(1);
		await click(document.querySelector('[aria-label="Recording actions"]'));
		expect(
			[...document.querySelectorAll('[role="menuitem"]')].some((item) =>
				item.textContent?.includes("Retry"),
			),
		).toBe(false);
		expect(copy).not.toHaveBeenCalled();
		await renderFailed(true, "failed-attempt");
		expect(button("Retry")?.getAttribute("data-loading")).toBe("true");
		await renderFailed(true, "another-attempt");
		expect(button("Retry")?.getAttribute("data-loading")).toBe(null);
		expect((button("Retry") as HTMLButtonElement).disabled).toBe(true);
	});
	it("keeps rerun in the menu for successful recordings and marks its pending request", async () => {
		const rerun = vi.fn();
		const renderSuccess = async (
			isRetryPending = false,
			retryPendingEntryId?: string,
		) =>
			act(async () =>
				root.render(
					<QueryClientProvider client={client}>
						<MantineProvider env="test">
							<HistoryFeedList
								isInitialLoading={false}
								hasError={false}
								emptyState={null}
								groupedHistory={groupHistoryForDisplay([
									{
										id: "successful-attempt",
										timestamp: "2026-01-01T00:00:00Z",
										text: "A successful transcript",
										status: "success",
									},
								])}
								copiedEntryId={null}
								onCopyEntry={copy}
								onRetryEntry={rerun}
								isRetryPending={isRetryPending}
								retryPendingEntryId={retryPendingEntryId}
								player={player}
								requestLogIds={new Set()}
								onDeleteEntry={vi.fn()}
								isDeleteDisabled={false}
							/>
						</MantineProvider>
					</QueryClientProvider>,
				),
			);
		await renderSuccess();
		expect(button("Retry")).toBeNull();
		await click(document.querySelector('[aria-label="Recording actions"]'));
		await click(button("Rerun as new result"));
		expect(rerun).toHaveBeenCalledExactlyOnceWith("successful-attempt");
		await renderSuccess(true, "successful-attempt");
		await click(document.querySelector('[aria-label="Recording actions"]'));
		expect((button("Rerunning…") as HTMLButtonElement).disabled).toBe(true);
		await renderSuccess(true, "another-attempt");
		expect((button("Rerun as new result") as HTMLButtonElement).disabled).toBe(
			true,
		);
	});
	it("renders the complete player shell while audio is loading", async () => {
		const loadingPlayer = {
			...player,
			id: "one",
			loading: true,
			ready: false,
			waveform: null,
			media: null,
		} satisfies RecordingPlayerControls;
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HistoryAudioPlayer player={loadingPlayer} recordingId="one" />
				</MantineProvider>,
			),
		);
		expect(
			document.querySelector(".history-audio-player--loading"),
		).not.toBeNull();
		expect(document.querySelector(".mantine-Loader-root")).toBeNull();
		expect(
			document.querySelector('[aria-label="Playback position"]'),
		).not.toBeNull();
		expect(
			(document.querySelector('[aria-label="Play audio"]') as HTMLButtonElement)
				.disabled,
		).toBe(true);
		expect(
			(
				document.querySelector(
					'[aria-label="Playback speed"]',
				) as HTMLInputElement
			).disabled,
		).toBe(true);
	});
	it("reveals the waveform after rendering and ignores stale redraws", async () => {
		const firstMedia = document.createElement("audio");
		const readyPlayer = { ...player, media: firstMedia };
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HistoryAudioPlayer player={readyPlayer} recordingId="one" />
				</MantineProvider>,
			),
		);
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).toBeNull();
		await act(async () => waveformEvents.get("redrawcomplete")?.());
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).not.toBeNull();
		const oldRedraw = waveformEvents.get("redrawcomplete");

		const secondMedia = document.createElement("audio");
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HistoryAudioPlayer
						player={{ ...readyPlayer, media: secondMedia }}
						recordingId="one"
					/>
				</MantineProvider>,
			),
		);
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).toBeNull();
		await act(async () => waveformEvents.get("redrawcomplete")?.());
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).not.toBeNull();
		await act(async () => oldRedraw?.());
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).not.toBeNull();
	});
	it("allows listening and seeking while optional waveform analysis is pending", async () => {
		const readyPlayer = {
			...player,
			media: document.createElement("audio"),
			waveform: null,
		};
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HistoryAudioPlayer player={readyPlayer} recordingId="one" />
				</MantineProvider>,
			),
		);
		expect(
			document.querySelector(".history-waveform-placeholder"),
		).not.toBeNull();
		expect(
			document.querySelector(".history-waveform-stage--revealed"),
		).toBeNull();
		expect(
			(document.querySelector('[aria-label="Play audio"]') as HTMLButtonElement)
				.disabled,
		).toBe(false);
		await click(document.querySelector('[aria-label="Play audio"]'));
		expect(player.toggle).toHaveBeenCalledWith("one");
		await click(document.querySelector('[aria-label="Forward 10 seconds"]'));
		expect(player.seek).toHaveBeenCalledWith(10);
	});
	it("copies full text explicitly and prepares the player without autoplay", async () => {
		await click(document.querySelector('[aria-label="Copy transcript"]'));
		expect(copy).toHaveBeenCalledWith("one", original);
		await click(document.querySelector(".history-card-toggle"));
		const transcript = document.querySelector(
			'[data-history-detail] [aria-label="Transcript"]',
		);
		const audioPlayer = document.querySelector(
			"[data-history-detail] .history-audio-player",
		);
		expect(transcript).not.toBeNull();
		expect(audioPlayer).not.toBeNull();
		if (!transcript || !audioPlayer)
			throw new Error("History detail is missing");
		expect(
			Boolean(
				transcript.compareDocumentPosition(audioPlayer) &
					Node.DOCUMENT_POSITION_FOLLOWING,
			),
		).toBe(true);
		expect(
			document.querySelector(
				'.history-audio-trailing [aria-label="Open full view"]',
			),
		).not.toBeNull();
		await click(document.querySelector('[aria-label="Open full view"]'));
		expect(document.querySelector('[role="dialog"]')).not.toBeNull();
		expect(document.querySelector('[aria-label="Recording title"]')).toBeNull();
		expect(
			document.querySelector('[role="dialog"]')?.textContent,
		).not.toContain("Saved");
		expect(
			document.querySelector('[aria-label="Full transcript"]')?.textContent,
		).toBe(original);
		expect(
			document.querySelector('[aria-label="Search transcript"]'),
		).not.toBeNull();
		expect(button("Edit")).not.toBeNull();
		expect(player.toggle).not.toHaveBeenCalled();
		expect(player.prepare).toHaveBeenCalledWith("one");
		expect(document.querySelector(".history-audio-buttons")).not.toBeNull();
		expect(
			document.querySelectorAll('[aria-label="Playback position"]'),
		).toHaveLength(1);
		// Portal events bubble through React's tree, but must not toggle the card.
		await click(document.querySelector('[aria-label="Full transcript"]'));
		expect(document.querySelector('[role="dialog"]')).not.toBeNull();
		expect(document.querySelector("#history-detail-one")).not.toBeNull();
		await click(document.querySelector(".mantine-Modal-close"));
		expect(player.stop).toHaveBeenCalled();
	});
	it("changes playback speed when the ready player's speed menu is selected", async () => {
		const readyPlayer = {
			...player,
			media: document.createElement("audio"),
			setRate: vi.fn(),
		};
		await act(async () =>
			root.render(
				<MantineProvider env="test">
					<HistoryAudioPlayer player={readyPlayer} recordingId="one" />
				</MantineProvider>,
			),
		);
		await click(document.querySelector('[aria-label="Playback speed"]'));
		await click(
			[...document.querySelectorAll('[role="option"]')].find(
				(option) => option.textContent === "1.5×",
			) ?? null,
		);
		expect(readyPlayer.setRate).toHaveBeenCalledWith(1.5);
	});
	it("flushes corrections when closing and keeps the reader open if saving fails", async () => {
		await click(document.querySelector(".history-card-toggle"));
		await click(document.querySelector('[aria-label="Open full view"]'));
		await click(button("Edit"));
		expect(
			document.querySelector('[aria-label="Recording title"]'),
		).not.toBeNull();
		const input = document.querySelector(
			'[aria-label="Edit transcript"]',
		) as HTMLTextAreaElement;
		await act(async () => {
			Object.getOwnPropertyDescriptor(
				HTMLTextAreaElement.prototype,
				"value",
			)?.set?.call(input, "Corrected meeting");
			input.dispatchEvent(new Event("input", { bubbles: true }));
		});
		vi.mocked(tauriAPI.saveHistoryEdit).mockRejectedValueOnce(
			new Error("Disk unavailable"),
		);
		await click(document.querySelector(".mantine-Modal-close"));
		expect(tauriAPI.saveHistoryEdit).toHaveBeenCalledWith(
			expect.objectContaining({
				text: "Corrected meeting",
				expected_revision: 0,
			}),
		);
		expect(document.querySelector('[role="dialog"]')).not.toBeNull();
		expect(input.value).toBe("Corrected meeting");
		expect(document.body.textContent).toContain("Your draft is still here");
		expect(await historyTextForCopy("one")).toBe("Corrected meeting");
		await click(document.querySelector(".mantine-Modal-close"));
		expect(tauriAPI.saveHistoryEdit).toHaveBeenCalledTimes(2);
		expect(document.querySelector('[role="dialog"]')).toBeNull();
	});
});
