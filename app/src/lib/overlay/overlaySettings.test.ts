import { QueryClient } from "@tanstack/react-query";
import type { InvokeArgs, InvokeOptions } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import { createOverlaySettingsChangedHandler } from "./overlaySettings";

function deferred<T>() {
	let resolve!: (value: T | PromiseLike<T>) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

describe("createOverlaySettingsChangedHandler", () => {
	it("applies accent, reloads settings, invalidates, and syncs pipeline", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const invoke = vi.fn(async () => undefined) as unknown as <T>(
			command: string,
			args?: InvokeArgs,
			options?: InvokeOptions,
		) => Promise<T>;
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
			invoke,
		});

		await handler({ accent_color: "#ff0000" });

		expect(applyAccentColor).toHaveBeenCalledWith("#ff0000");
		expect(reloadSettingsFromDisk).toHaveBeenCalled();
		expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["settings"] });
		expect(invoke).toHaveBeenCalledWith("sync_pipeline_config");
	});

	it("still reloads and syncs without accent payload", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const invoke = vi.fn(async () => undefined) as unknown as <T>(
			command: string,
			args?: InvokeArgs,
			options?: InvokeOptions,
		) => Promise<T>;
		const queryClient = new QueryClient();
		vi.spyOn(queryClient, "invalidateQueries").mockResolvedValue(
			undefined as never,
		);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
			invoke,
		});

		await handler({});

		expect(applyAccentColor).not.toHaveBeenCalled();
		expect(reloadSettingsFromDisk).toHaveBeenCalled();
		expect(invoke).toHaveBeenCalledWith("sync_pipeline_config");
	});

	it("ignores stale settings_revision payloads", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const invoke = vi.fn(async () => undefined) as unknown as <T>(
			command: string,
			args?: InvokeArgs,
			options?: InvokeOptions,
		) => Promise<T>;
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
			invoke,
		});

		await handler({ settings_revision: 2, accent_color: "#111111" });
		await handler({ settings_revision: 1, accent_color: "#222222" });

		// Accent is applied immediately only for the first (newest) event.
		expect(applyAccentColor).toHaveBeenCalledTimes(1);
		expect(applyAccentColor).toHaveBeenCalledWith("#111111");

		expect(reloadSettingsFromDisk).toHaveBeenCalledTimes(1);
		expect(invalidateSpy).toHaveBeenCalledTimes(1);
		expect(invoke).toHaveBeenCalledTimes(1);
	});

	it("skips post-reload work when a newer revision arrives mid-reload", async () => {
		const applyAccentColor = vi.fn();
		const r1 = deferred<void>();
		const r2 = deferred<void>();
		const reloadSettingsFromDisk = vi
			.fn()
			.mockReturnValueOnce(r1.promise)
			.mockReturnValueOnce(r2.promise);
		const invoke = vi.fn(async () => undefined) as unknown as <T>(
			command: string,
			args?: InvokeArgs,
			options?: InvokeOptions,
		) => Promise<T>;
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
			invoke,
		});

		const p1 = handler({ settings_revision: 10, accent_color: "#101010" });
		const p2 = handler({ settings_revision: 11, accent_color: "#111111" });

		// Let the newer one finish first.
		r2.resolve();
		await p2;

		// Now finish the older one.
		r1.resolve();
		await p1;

		// Both accents applied immediately.
		expect(applyAccentColor).toHaveBeenCalledTimes(2);

		// But expensive follow-up should run once (for the latest revision).
		expect(invalidateSpy).toHaveBeenCalledTimes(1);
		expect(invoke).toHaveBeenCalledTimes(1);
	});
});
