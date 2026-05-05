import { QueryClient } from "@tanstack/react-query";
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
	it("applies accent, reloads settings, and invalidates without duplicating runtime sync", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
		});

		await handler({ accent_color: "#ff0000" });

		expect(applyAccentColor).toHaveBeenCalledWith("#ff0000");
		expect(reloadSettingsFromDisk).toHaveBeenCalled();
		expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["settings"] });
	});

	it("still reloads and invalidates without accent payload", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
		});

		await handler({});

		expect(applyAccentColor).not.toHaveBeenCalled();
		expect(reloadSettingsFromDisk).toHaveBeenCalled();
		expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: ["settings"] });
	});

	it("handles malformed revision and accent payloads without skipping reload", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
		});

		await handler({ settings_revision: Number.POSITIVE_INFINITY as never });
		await handler({ accent_color: 42 as never });
		await handler({ accent_color: null });

		expect(applyAccentColor).toHaveBeenCalledTimes(1);
		expect(applyAccentColor).toHaveBeenCalledWith(null);
		expect(reloadSettingsFromDisk).toHaveBeenCalledTimes(3);
		expect(invalidateSpy).toHaveBeenCalledTimes(3);
	});

	it("ignores non-critical payload and reload failures", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => {
			throw new Error("reload failed");
		});
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);
		const payloadWithThrowingGetter = Object.defineProperty(
			{},
			"accent_color",
			{
				get() {
					throw new Error("bad payload");
				},
			},
		);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
		});

		await handler(payloadWithThrowingGetter as never);
		await handler(null as never);

		expect(applyAccentColor).not.toHaveBeenCalled();
		expect(reloadSettingsFromDisk).toHaveBeenCalledTimes(2);
		expect(invalidateSpy).toHaveBeenCalledTimes(2);
	});

	it("ignores stale settings_revision payloads", async () => {
		const applyAccentColor = vi.fn();
		const reloadSettingsFromDisk = vi.fn(async () => undefined);
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
		});

		await handler({ settings_revision: 2, accent_color: "#111111" });
		await handler({ settings_revision: 1, accent_color: "#222222" });

		// Accent is applied immediately only for the first (newest) event.
		expect(applyAccentColor).toHaveBeenCalledTimes(1);
		expect(applyAccentColor).toHaveBeenCalledWith("#111111");

		expect(reloadSettingsFromDisk).toHaveBeenCalledTimes(1);
		expect(invalidateSpy).toHaveBeenCalledTimes(1);
	});

	it("skips post-reload work when a newer revision arrives mid-reload", async () => {
		const applyAccentColor = vi.fn();
		const r1 = deferred<void>();
		const r2 = deferred<void>();
		const reloadSettingsFromDisk = vi
			.fn()
			.mockReturnValueOnce(r1.promise)
			.mockReturnValueOnce(r2.promise);
		const queryClient = new QueryClient();
		const invalidateSpy = vi
			.spyOn(queryClient, "invalidateQueries")
			.mockResolvedValue(undefined as never);

		const handler = createOverlaySettingsChangedHandler({
			applyAccentColor,
			reloadSettingsFromDisk,
			queryClient,
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
	});
});
