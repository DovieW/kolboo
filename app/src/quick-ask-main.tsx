import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import "@mantine/code-highlight/styles.css";
import {
  CodeHighlightAdapterProvider,
  createHighlightJsAdapter,
} from "@mantine/code-highlight";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import "highlight.js/styles/github-dark.css";
import "@fontsource/sora/index.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import QuickAskApp from "./QuickAskApp";
import { darkTheme } from "./theme";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("typescript", typescript);
const highlightAdapter = createHighlightJsAdapter(hljs);

const rootElement = document.getElementById("root");
if (!rootElement) {
	throw new Error("Root element not found");
}

createRoot(rootElement).render(
  <StrictMode>
    <CodeHighlightAdapterProvider adapter={highlightAdapter}>
      <MantineProvider theme={darkTheme} defaultColorScheme="dark">
        <QuickAskApp />
      </MantineProvider>
    </CodeHighlightAdapterProvider>
  </StrictMode>
);
