declare module "@mantine/code-highlight" {
  import type { ComponentType } from "react";

  export const CodeHighlight: ComponentType<any>;
  export const CodeHighlightTabs: ComponentType<any>;
  export const CodeHighlightProvider: ComponentType<any>;

  // Used in app bootstrap to wire highlight.js.
  export const CodeHighlightAdapterProvider: ComponentType<any>;
  export function createHighlightJsAdapter(hljs: any): any;
}
