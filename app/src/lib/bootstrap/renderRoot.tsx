import { type ReactNode, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import {
	getSentryReactRootOptions,
	type SentrySurface,
} from "../telemetry/sentry";
import { getRootElementOrThrow } from "./rootElement";

export function renderRoot(
	children: ReactNode,
	options?: {
		elementId?: string;
		sentrySurface?: SentrySurface;
		sentryReady?: Promise<boolean>;
		strictMode?: boolean;
	},
): void {
	const rootElement = getRootElementOrThrow(options?.elementId);
	const rootOptions = options?.sentrySurface
		? getSentryReactRootOptions(options.sentrySurface, options.sentryReady)
		: undefined;

	createRoot(rootElement, rootOptions).render(
		options?.strictMode === false ? (
			children
		) : (
			<StrictMode>{children}</StrictMode>
		),
	);
}
