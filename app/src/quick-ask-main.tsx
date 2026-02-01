import { AppMantineProvider } from "./lib/bootstrap/AppMantineProvider";
import { renderRoot } from "./lib/bootstrap/renderRoot";
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

import QuickAskApp from "./QuickAskApp";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("typescript", typescript);
const highlightAdapter = createHighlightJsAdapter(hljs);

renderRoot(
	<CodeHighlightAdapterProvider adapter={highlightAdapter}>
		<AppMantineProvider>
			<QuickAskApp />
		</AppMantineProvider>
	</CodeHighlightAdapterProvider>,
);

const fallback = document.getElementById("quick-ask-fallback");
if (fallback) {
	fallback.remove();
}
