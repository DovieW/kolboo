#!/usr/bin/env node
import {
	configureRustBuildEnv,
	describeRustBuildEnv,
} from "./rust-build-env.mjs";

try {
	const { env, status } = configureRustBuildEnv(process.env, {
		requireTools: true,
	});
	console.log(
		`Rust build acceleration ready: ${describeRustBuildEnv(status, env)}`,
	);
} catch (error) {
	console.error(error instanceof Error ? error.message : String(error));
	process.exit(1);
}
