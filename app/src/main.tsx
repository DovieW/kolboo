import { MainAppRoot } from "./lib/bootstrap/MainAppRoot";
import { renderRoot } from "./lib/bootstrap/renderRoot";
import { initSentry } from "./lib/telemetry/sentry";

void initSentry("main").finally(() => {
	renderRoot(<MainAppRoot />, { sentrySurface: "main" });
});
