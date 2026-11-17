import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
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
		{ sentrySurface: "overlay_hover" },
	);
});

const fallback = document.getElementById("overlay-hover-fallback");
if (fallback) {
	fallback.remove();
}
