import { isTauriRuntimeAvailable } from "./lib/tauri/runtimeConfig";
import {
	initSentry,
	maybeCaptureSentrySmokeTest,
} from "./lib/telemetry/sentry";

void initSentry("main").finally(async () => {
	const ranSmoke = await maybeCaptureSentrySmokeTest("main");

	// Browser-only smoke verification should not continue into the full desktop
	// app boot path when the Tauri bridge is absent, otherwise the expected
	// `invoke`/event-listener failures drown the intentional smoke issue in noise.
	if (ranSmoke && !isTauriRuntimeAvailable()) {
		return;
	}

	// Keep the heavy app bootstrap behind the smoke guard so browser-only
	// verification does not even import the Tauri-dependent desktop surfaces.
	const [{ MainAppRoot }, { renderRoot }] = await Promise.all([
		import("./lib/bootstrap/MainAppRoot"),
		import("./lib/bootstrap/renderRoot"),
	]);

	renderRoot(<MainAppRoot />, { sentrySurface: "main" });
});
