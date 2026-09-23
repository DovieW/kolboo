// @vitest-environment happy-dom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProfileConfigModal } from "./ProgramsModal";

const mocks = vi.hoisted(() => ({
	listOpenWindows: vi.fn<() => Promise<Array<{ title: string; process_path: string }>>>(),
	settings: {
		rewrite_program_prompt_profiles: [
			{ id: "browser", name: "Browser", program_paths: [], disabled: false },
		],
	},
}));

vi.mock("../../lib/queries", () => ({
	useSettings: () => ({
		isLoading: false,
		data: mocks.settings,
	}),
	useUpdateRewriteProgramPromptProfiles: () => ({
		isPending: false,
		mutate: vi.fn(),
	}),
}));
vi.mock("../../lib/tauri", () => ({
	tauriAPI: { listOpenWindows: mocks.listOpenWindows },
}));

let host: HTMLDivElement;
let root: Root;

beforeEach(async () => {
	Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true);
	host = document.createElement("div");
	document.body.append(host);
	root = createRoot(host);
	await act(async () => {
		root.render(
			<MantineProvider env="test">
				<ProfileConfigModal
					opened
					onClose={vi.fn()}
					editingProfileId="browser"
					onEditingProfileChange={vi.fn()}
				/>
			</MantineProvider>,
		);
	});
});

afterEach(async () => {
	await act(async () => root.unmount());
	host.remove();
	vi.clearAllMocks();
});

describe("open-program picker", () => {
	it("shows a desktop integration error instead of claiming no windows are open", async () => {
		mocks.listOpenWindows.mockRejectedValueOnce("GNOME extension is not enabled.");
		const button = document.querySelector<HTMLButtonElement>(
			'[aria-label="Pick from open programs"]',
		);
		expect(button).not.toBeNull();
		await act(async () => button?.click());
		expect(document.body.textContent).toContain(
			"GNOME extension is not enabled.",
		);
		expect(document.body.textContent).not.toContain("No windows found");
	});

	it("shows an open program when discovery succeeds", async () => {
		mocks.listOpenWindows.mockResolvedValueOnce([
			{ title: "", process_path: "/usr/bin/editor" },
		]);
		const button = document.querySelector<HTMLButtonElement>(
			'[aria-label="Pick from open programs"]',
		);
		await act(async () => button?.click());
		expect(mocks.listOpenWindows).toHaveBeenCalledOnce();
		expect(document.body.textContent).toContain("editor");
	});

	it("uses a generic message for an unexpected non-string rejection", async () => {
		mocks.listOpenWindows.mockRejectedValueOnce(new Error("internal details"));
		const button = document.querySelector<HTMLButtonElement>(
			'[aria-label="Pick from open programs"]',
		);
		await act(async () => button?.click());
		expect(document.body.textContent).toContain("Could not list open programs.");
		expect(document.body.textContent).not.toContain("internal details");
	});
});
