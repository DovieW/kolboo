declare module "@mantine/code-highlight" {
	import type { ComponentType } from "react";

	export const CodeHighlight: ComponentType<Record<string, unknown>>;
	export const CodeHighlightTabs: ComponentType<Record<string, unknown>>;
	export const CodeHighlightProvider: ComponentType<Record<string, unknown>>;

	// Used in app bootstrap to wire highlight.js.
	export const CodeHighlightAdapterProvider: ComponentType<
		Record<string, unknown>
	>;
	export function createHighlightJsAdapter(hljs: unknown): unknown;
}
