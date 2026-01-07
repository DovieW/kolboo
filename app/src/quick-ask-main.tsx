import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "@fontsource/sora/index.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import QuickAskApp from "./QuickAskApp";
import { darkTheme } from "./theme";

const rootElement = document.getElementById("root");
if (!rootElement) {
	throw new Error("Root element not found");
}

createRoot(rootElement).render(
	<StrictMode>
		<MantineProvider theme={darkTheme} defaultColorScheme="dark">
			<QuickAskApp />
		</MantineProvider>
	</StrictMode>
);
