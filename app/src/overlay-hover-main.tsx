import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import OverlayHoverApp from "./OverlayHoverApp";
import { darkTheme } from "./theme";

const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			refetchOnWindowFocus: false,
			retry: false,
		},
	},
});

const rootElement = document.getElementById("root");
if (!rootElement) {
	throw new Error("Root element not found");
}

createRoot(rootElement).render(
	<StrictMode>
		<QueryClientProvider client={queryClient}>
			<MantineProvider theme={darkTheme} defaultColorScheme="dark">
				<OverlayHoverApp />
			</MantineProvider>
		</QueryClientProvider>
	</StrictMode>,
);
