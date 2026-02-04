import { AppMantineProvider } from "./lib/bootstrap/AppMantineProvider";
import { renderRoot } from "./lib/bootstrap/renderRoot";
import "@mantine/core/styles.css";
import "@fontsource/sora/index.css";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { Component } from "react";
import OverlayApp from "./OverlayApp";

// Styles are imported in OverlayApp.tsx via app.css

const queryClient = new QueryClient({
	defaultOptions: {
		queries: { retry: 2 },
		mutations: { retry: 1 },
	},
});

type ErrorBoundaryState = {
	hasError: boolean;
	message: string;
	stack?: string;
};

class OverlayErrorBoundary extends Component<
	{ children: React.ReactNode },
	ErrorBoundaryState
> {
	state: ErrorBoundaryState = { hasError: false, message: "" };

	static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
		const msg =
			error instanceof Error
				? error.message
				: typeof error === "string"
					? error
					: "Overlay crashed";
		const stack = error instanceof Error ? error.stack : undefined;
		return { hasError: true, message: msg, stack };
	}

	componentDidCatch(error: unknown) {
		// eslint-disable-next-line no-console
		console.error("[Overlay] Uncaught render error", error);
	}

	render() {
		if (!this.state.hasError) return this.props.children;
		const stackHint = this.state.stack
			? this.state.stack.split("\n").slice(0, 2).join("\n")
			: "";
		return (
			<div
				style={{
					width: 240,
					height: 56,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					background: "rgba(127, 29, 29, 0.92)",
					border: "1px solid rgba(255,255,255,0.10)",
					borderRadius: 16,
					color: "rgba(255,255,255,0.95)",
					fontFamily:
						"Sora, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
					padding: "10px 12px",
					boxSizing: "border-box",
					whiteSpace: "nowrap",
					overflow: "hidden",
					textOverflow: "ellipsis",
				}}
				role="alert"
				title={[this.state.message, stackHint ? `\n\n${stackHint}` : ""].join(
					"",
				)}
			>
				Overlay error (overlay-main v3): {this.state.message}
			</div>
		);
	}
}

renderRoot(
	<QueryClientProvider client={queryClient}>
		<AppMantineProvider>
			<OverlayErrorBoundary>
				<OverlayApp />
			</OverlayErrorBoundary>
		</AppMantineProvider>
	</QueryClientProvider>,
);

invoke("overlay_frontend_ready").catch((error) => {
	console.error("[Overlay] Failed to notify frontend ready:", error);
});

const fallback = document.getElementById("overlay-fallback");
if (fallback) {
	fallback.remove();
}
