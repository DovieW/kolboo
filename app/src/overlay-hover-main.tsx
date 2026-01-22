import { AppMantineProvider } from "./lib/bootstrap/AppMantineProvider";
import { renderRoot } from "./lib/bootstrap/renderRoot";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import OverlayHoverApp from "./OverlayHoverApp";

const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			refetchOnWindowFocus: false,
			retry: false,
		},
	},
});

renderRoot(
	<QueryClientProvider client={queryClient}>
		<AppMantineProvider>
			<OverlayHoverApp />
		</AppMantineProvider>
	</QueryClientProvider>,
);
