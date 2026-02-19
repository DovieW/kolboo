import { AppMantineProvider } from "./lib/bootstrap/AppMantineProvider";
import { renderRoot } from "./lib/bootstrap/renderRoot";
import "@mantine/core/styles.css";
import "@mantine/code-highlight/styles.css";
import {
	CodeHighlightAdapterProvider,
	createHighlightJsAdapter,
} from "@mantine/code-highlight";
import hljs from "highlight.js/lib/core";
import json from "highlight.js/lib/languages/json";
import "highlight.js/styles/github-dark.css";
import "@fontsource/sora/index.css";
import "@fontsource/outfit/index.css";
import { Notifications } from "@mantine/notifications";
import "@mantine/notifications/styles.css";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Component, type ReactNode, useEffect, useState } from "react";
import App from "./App";
import { initSentry } from "./lib/telemetry/sentry";
import "./styles.css";

const queryClient = new QueryClient();

hljs.registerLanguage("json", json);
const highlightAdapter = createHighlightJsAdapter(hljs);

let panicHandlersInstalled = false;

type PanicKind = "react" | "error" | "unhandledrejection";

type PanicInfo = {
	kind: PanicKind;
	message: string;
	stack?: string | null;
	time_iso: string;
};

declare global {
	interface Window {
		__kolboo_panic__?: PanicInfo;
	}
}

function coerceErrorDetails(err: unknown): {
	message: string;
	stack?: string | null;
} {
	if (err instanceof Error) {
		return { message: err.message || String(err), stack: err.stack ?? null };
	}

	if (err && typeof err === "object" && "message" in err) {
		const message = String((err as { message: unknown }).message);
		const stack =
			"stack" in err ? String((err as { stack: unknown }).stack) : null;
		return { message, stack };
	}

	return { message: String(err), stack: null };
}

function emitPanic(info: PanicInfo) {
	// Keep the first panic only; follow-on errors are usually cascading noise.
	if (window.__kolboo_panic__) return;
	window.__kolboo_panic__ = info;
	window.dispatchEvent(
		new CustomEvent<PanicInfo>("kolboo-panic", { detail: info }),
	);
}

function installGlobalPanicHandlers() {
	// Only install once.
	if (panicHandlersInstalled) return;
	panicHandlersInstalled = true;

	window.addEventListener("error", (event) => {
		// Resource loading errors may not have an Error object; still record what we can.
		const err = (event as ErrorEvent).error;
		const details = coerceErrorDetails(err ?? (event as unknown));
		emitPanic({
			kind: "error",
			message: details.message || "Uncaught error",
			stack: details.stack ?? null,
			time_iso: new Date().toISOString(),
		});
	});

	window.addEventListener("unhandledrejection", (event) => {
		const details = coerceErrorDetails((event as PromiseRejectionEvent).reason);
		emitPanic({
			kind: "unhandledrejection",
			message: details.message || "Unhandled rejection",
			stack: details.stack ?? null,
			time_iso: new Date().toISOString(),
		});
	});
}

function PanicOverlay({ info }: { info: PanicInfo }) {
	const [copied, setCopied] = useState(false);
	const [copyError, setCopyError] = useState<string | null>(null);

	const diagnostics = {
		...info,
		user_agent: navigator.userAgent,
		location: window.location.href,
		build_time: new Date().toISOString(),
	};

	const doReload = () => {
		try {
			window.location.reload();
		} catch {
			// ignore
		}
	};

	const clearLocalCacheThenReload = () => {
		try {
			window.localStorage?.clear();
			window.sessionStorage?.clear();
		} catch {
			// ignore
		}
		doReload();
	};

	const copyDiagnostics = async () => {
		setCopyError(null);
		setCopied(false);
		const text = JSON.stringify(diagnostics, null, 2);
		try {
			await navigator.clipboard.writeText(text);
			setCopied(true);
			window.setTimeout(() => setCopied(false), 1200);
		} catch (e) {
			setCopyError(String(e));
		}
	};

	return (
		<div
			style={{
				position: "fixed",
				inset: 0,
				background: "#0b0d10",
				color: "#e7e9ee",
				zIndex: 2147483647,
				padding: 20,
				overflow: "auto",
				fontFamily:
					"ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial",
			}}
			role="dialog"
			aria-modal="true"
			aria-label="Application error"
		>
			<div style={{ maxWidth: 900, margin: "0 auto" }}>
				<h1 style={{ margin: "8px 0 6px", fontSize: 18, fontWeight: 700 }}>
					Kolboo hit a snag
				</h1>
				<p style={{ margin: "0 0 14px", opacity: 0.85, lineHeight: 1.45 }}>
					Something went wrong while starting up. Reloading usually fixes it; if
					it keeps happening, please copy the diagnostics below.
				</p>

				<div
					style={{
						display: "flex",
						gap: 10,
						flexWrap: "wrap",
						marginBottom: 14,
					}}
				>
					<button
						type="button"
						onClick={doReload}
						style={{
							background: "#f97316",
							color: "#0b0d10",
							border: "none",
							borderRadius: 8,
							padding: "10px 12px",
							fontWeight: 700,
							cursor: "pointer",
						}}
					>
						Reload app
					</button>

					<button
						type="button"
						onClick={clearLocalCacheThenReload}
						style={{
							background: "transparent",
							color: "#e7e9ee",
							border: "1px solid rgba(231, 233, 238, 0.18)",
							borderRadius: 8,
							padding: "10px 12px",
							fontWeight: 600,
							cursor: "pointer",
						}}
					>
						Clear local cache + reload
					</button>

					<button
						type="button"
						onClick={() => void copyDiagnostics()}
						style={{
							background: "transparent",
							color: "#e7e9ee",
							border: "1px solid rgba(231, 233, 238, 0.18)",
							borderRadius: 8,
							padding: "10px 12px",
							fontWeight: 600,
							cursor: "pointer",
						}}
					>
						{copied ? "Copied" : "Copy diagnostics"}
					</button>
				</div>

				{copyError ? (
					<p style={{ margin: "0 0 12px", color: "#fca5a5" }}>
						Copy failed: {copyError}
					</p>
				) : null}

				<div
					style={{
						border: "1px solid rgba(231, 233, 238, 0.14)",
						borderRadius: 10,
						padding: 12,
						background: "rgba(255,255,255,0.02)",
					}}
				>
					<div style={{ marginBottom: 8, opacity: 0.85, fontSize: 12 }}>
						<div>
							<strong>Kind:</strong> {info.kind}
						</div>
						<div>
							<strong>Time:</strong> {info.time_iso}
						</div>
					</div>
					<pre
						style={{
							margin: 0,
							whiteSpace: "pre-wrap",
							wordBreak: "break-word",
							fontSize: 12,
							lineHeight: 1.45,
							color: "#cdd3df",
						}}
					>
						{info.stack ? `${info.message}\n\n${info.stack}` : info.message}
					</pre>
				</div>
			</div>
		</div>
	);
}

class AppErrorBoundary extends Component<
	{ children: ReactNode },
	{ panic: PanicInfo | null }
> {
	state: { panic: PanicInfo | null } = { panic: null };

	static getDerivedStateFromError(error: unknown) {
		const details = coerceErrorDetails(error);
		return {
			panic: {
				kind: "react" as const,
				message: details.message || "React render error",
				stack: details.stack ?? null,
				time_iso: new Date().toISOString(),
			},
		};
	}

	componentDidCatch(error: unknown) {
		const details = coerceErrorDetails(error);
		emitPanic({
			kind: "react",
			message: details.message || "React render error",
			stack: details.stack ?? null,
			time_iso: new Date().toISOString(),
		});
	}

	render() {
		if (this.state.panic) {
			return <PanicOverlay info={this.state.panic} />;
		}
		return this.props.children;
	}
}

function PanicGate({ children }: { children: ReactNode }) {
	const [panic, setPanic] = useState<PanicInfo | null>(
		window.__kolboo_panic__ ?? null,
	);

	useEffect(() => {
		const onPanic = (ev: Event) => {
			const detail = (ev as CustomEvent<PanicInfo>).detail;
			if (!detail) return;
			setPanic(detail);
		};

		window.addEventListener("kolboo-panic", onPanic);
		return () => window.removeEventListener("kolboo-panic", onPanic);
	}, []);

	useEffect(() => {
		if (!panic) return;

		// Optional: try a single auto-reload for early-boot panics since that
		// tends to recover the webview/store on affected machines.
		const didBootReloadKey = "kolboo_panic_boot_reload_v1";
		const alreadyTried =
			window.sessionStorage?.getItem(didBootReloadKey) === "1";
		const isEarlyBoot = performance.now() < 5000;
		if (!alreadyTried && isEarlyBoot) {
			try {
				window.sessionStorage?.setItem(didBootReloadKey, "1");
			} catch {
				// ignore
			}

			// Give the overlay a moment to paint; then reload.
			const t = window.setTimeout(() => {
				try {
					window.location.reload();
				} catch {
					// ignore
				}
			}, 250);
			return () => window.clearTimeout(t);
		}

		return undefined;
	}, [panic]);

	return (
		<>
			{children}
			{panic ? <PanicOverlay info={panic} /> : null}
		</>
	);
}

installGlobalPanicHandlers();

void initSentry("main").finally(() => {
	renderRoot(
		<QueryClientProvider client={queryClient}>
			<CodeHighlightAdapterProvider adapter={highlightAdapter}>
				<AppMantineProvider>
					<Notifications position="top-right" />
					<PanicGate>
						<AppErrorBoundary>
							<App />
						</AppErrorBoundary>
					</PanicGate>
				</AppMantineProvider>
			</CodeHighlightAdapterProvider>
		</QueryClientProvider>,
	);
});
