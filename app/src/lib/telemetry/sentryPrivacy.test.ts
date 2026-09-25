import type { ErrorEvent } from "@sentry/react";
import { describe, expect, it } from "vitest";
import { scrubSentryEvent } from "./sentryPrivacy";

describe("outgoing Sentry privacy boundary", () => {
	it("keeps symbolication and failure metadata but removes unmarked user content", () => {
		const privateText = "Jane discussed her medical appointment tomorrow";
		const event: ErrorEvent = {
			type: undefined,
			event_id: "event-id",
			level: "error",
			release: "kolboo@1.2.3",
			environment: "beta",
			platform: "javascript",
			dist: "linux-x64",
			timestamp: 123,
			sdk: { name: "sentry.javascript.react", version: "10.69.0" },
			message: privateText,
			user: { email: "private@example.test" },
			request: { url: "https://provider.test?key=secret" },
			extra: { response: privateText },
			contexts: { anything: { text: privateText } },
			breadcrumbs: [{ category: "console", message: privateText }],
			fingerprint: [privateText],
			transaction: privateText,
			tags: {
				service: "desktop",
				runtime: "webview",
				surface: "main",
				os: "linux",
				action: "license_refresh_entitlement",
				user_hash: "none",
				custom: privateText,
				tier: privateText,
				numeric: 123,
			},
			exception: {
				values: [
					{
						type: "TypeError",
						value: privateText,
						mechanism: {
							type: "auto.browser.global_handlers.onerror",
							handled: false,
							data: { payload: privateText },
						},
						stacktrace: {
							frames: [
								{
									filename:
										"http://tauri.localhost/assets/main-abc.js?private=yes#secret",
									abs_path: "/home/Jane/app.js",
									function: "startRecording",
									lineno: 42,
									colno: 10,
									in_app: true,
									vars: { body: privateText },
									context_line: privateText,
								},
							],
						},
					},
				],
			},
			debug_meta: {
				images: [
					{
						type: "sourcemap",
						debug_id: "123-debug-id",
						code_file:
							"http://tauri.localhost/assets/main-abc.js?private=yes#secret",
					},
				],
			},
		};
		const original = structuredClone(event);
		const safe = scrubSentryEvent(event);
		expect(event).toEqual(original);
		expect(JSON.stringify(safe)).not.toMatch(
			/Jane|private|provider\.test|payload|user_hash/,
		);
		expect(safe).toMatchObject({
			event_id: "event-id",
			release: "kolboo@1.2.3",
			environment: "beta",
			dist: "linux-x64",
			tags: {
				service: "desktop",
				runtime: "webview",
				surface: "main",
				os: "linux",
				action: "license_refresh_entitlement",
			},
		});
		expect(safe.exception?.values?.[0]).toEqual({
			type: "TypeError",
			value: "Error details withheld for privacy",
			mechanism: {
				type: "auto.browser.global_handlers.onerror",
				handled: false,
			},
			stacktrace: {
				frames: [
					{
						filename: "app:///main-abc.js",
						function: "startRecording",
						lineno: 42,
						colno: 10,
						in_app: true,
					},
				],
			},
		});
		expect(safe.debug_meta?.images?.[0]).toEqual({
			type: "sourcemap",
			debug_id: "123-debug-id",
			code_file: "app:///main-abc.js",
		});
	});

	it("handles partial events, unknown exception types and optional stack fields", () => {
		expect(
			scrubSentryEvent({
				type: undefined,
				tags: {
					action: "private body",
					constructor: "private",
					toString: "private",
				},
			}).tags,
		).toEqual({});
		expect(scrubSentryEvent({ type: undefined }).exception).toBeUndefined();
		expect(scrubSentryEvent({ type: undefined }).message).toBeUndefined();
		expect(
			scrubSentryEvent({ type: undefined, exception: {}, debug_meta: {} }),
		).toMatchObject({
			exception: { values: undefined },
			debug_meta: { images: undefined },
		});
		const safe = scrubSentryEvent({
			type: undefined,
			tags: {
				smoke_test: "true",
				smoke_trigger: "runtime-env",
				action: "smoke_test",
			},
			exception: {
				values: [
					{},
					{ type: "private response" },
					{ stacktrace: {} },
					{
						stacktrace: {
							frames: [{}, { filename: "C:\\Users\\Jane\\main.js" }],
						},
					},
				],
			},
			debug_meta: {
				images: [
					{ type: "sourcemap", debug_id: "abc", code_file: "" },
					{
						type: "macho",
						debug_id: "def",
						code_file: "private",
						image_addr: "0x0",
					},
				],
			},
		});
		expect(safe.exception?.values?.map((value) => value.type)).toEqual([
			"Error",
			"Error",
			"Error",
			"Error",
		]);
		expect(safe.exception?.values?.[3]?.stacktrace?.frames?.[1]?.filename).toBe(
			"app:///main.js",
		);
		expect(safe.debug_meta?.images).toEqual([
			{ type: "sourcemap", debug_id: "abc", code_file: "app:///" },
		]);
		expect(safe.tags).toEqual({
			smoke_test: "true",
			smoke_trigger: "runtime-env",
			action: "smoke_test",
		});
	});
});
