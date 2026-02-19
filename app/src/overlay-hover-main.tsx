import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { AppMantineProvider } from "./lib/bootstrap/AppMantineProvider";
import { renderRoot } from "./lib/bootstrap/renderRoot";
import { initSentry } from "./lib/telemetry/sentry";

import OverlayHoverApp from "./OverlayHoverApp";

const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			refetchOnWindowFocus: false,
			retry: false,
		},
	},
});

void initSentry("overlay_hover").finally(() => {
	renderRoot(
		<QueryClientProvider client={queryClient}>
			<AppMantineProvider>
				<OverlayHoverApp />
			</AppMantineProvider>
		</QueryClientProvider>,
	);
});

const notifyHoverReady = () => {
	invoke("overlay_hover_frontend_ready").catch((error) => {
		console.error("[OverlayHover] Failed to heartbeat frontend ready:", error);
	});
};
notifyHoverReady();

const hoverReadyInterval = window.setInterval(notifyHoverReady, 15_000);
window.addEventListener("beforeunload", () => {
	window.clearInterval(hoverReadyInterval);
});

const fallback = document.getElementById("overlay-hover-fallback");
if (fallback) {
	fallback.remove();
}
