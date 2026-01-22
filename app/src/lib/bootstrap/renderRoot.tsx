import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { getRootElementOrThrow } from "./rootElement";

export function renderRoot(
	children: ReactNode,
	options?: {
		elementId?: string;
		strictMode?: boolean;
	},
): void {
	const rootElement = getRootElementOrThrow(options?.elementId);
	createRoot(rootElement).render(
		options?.strictMode === false ? children : <StrictMode>{children}</StrictMode>,
	);
}
