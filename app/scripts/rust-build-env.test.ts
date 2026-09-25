import { describe, expect, it } from "vitest";
import {
	configureRustBuildEnv,
	inspectRustBuildTools,
	rustComponentListIncludes,
	withUserLocalBin,
} from "./rust-build-env.mjs";

describe("Rust build acceleration environment", () => {
	it("finds required tools against the normalized PATH", () => {
		const seen: string[] = [];
		const status = inspectRustBuildTools({
			platform: "linux",
			env: { PATH: "/usr/bin" },
			commandExistsFn: (command: string) => {
				seen.push(command);
				return true;
			},
			moldLinkerWorksFn: () => true,
			rustComponentInstalledFn: () => true,
		});

		expect(seen.sort()).toEqual(["cargo-llvm-cov", "mold", "sccache"]);
		expect(status).toMatchObject({
			sccache: true,
			mold: true,
			moldRequired: true,
			cargoLlvmCov: true,
			llvmTools: true,
		});
	});

	it("rejects a mold binary that the compiler cannot invoke", () => {
		const status = inspectRustBuildTools({
			platform: "linux",
			env: { PATH: "/usr/bin" },
			commandExistsFn: () => true,
			moldLinkerWorksFn: () => false,
			rustComponentInstalledFn: () => true,
		});

		expect(status).toMatchObject({ mold: false, moldBinary: true });
	});

	it("recognizes rustup's target-qualified llvm-tools component name", () => {
		expect(
			rustComponentListIncludes(
				"llvm-tools-x86_64-unknown-linux-gnu\nrustfmt-x86_64-unknown-linux-gnu",
				"llvm-tools-preview",
			),
		).toBe(true);
	});

	it("prepends the user-local bin directory only once", () => {
		const first = withUserLocalBin("/usr/bin");
		const second = withUserLocalBin(first);

		expect(second).toBe(first);
		expect(first.endsWith("/usr/bin")).toBe(true);
	});

	it("preserves explicit caller settings", () => {
		const { env } = configureRustBuildEnv(
			{
				...process.env,
				CARGO_BUILD_JOBS: "3",
				CARGO_INCREMENTAL: "1",
				RUSTC_WRAPPER: "custom-wrapper",
				RUSTFLAGS: "-C target-cpu=native",
			},
			{ requireTools: false },
		);

		expect(env.CARGO_BUILD_JOBS).toBe("3");
		expect(env.CARGO_INCREMENTAL).toBe("1");
		expect(env.RUSTC_WRAPPER).toBe("custom-wrapper");
		expect(env.RUSTFLAGS).toContain("-C target-cpu=native");
	});

	it("disables Cargo incremental output when sccache owns Rust reuse", () => {
		const { env } = configureRustBuildEnv(
			{ PATH: process.env.PATH },
			{ requireTools: false },
		);

		if (env.RUSTC_WRAPPER === "sccache") {
			expect(env.CARGO_INCREMENTAL).toBe("0");
		}
	});

	it("does not change incremental mode for an explicit non-sccache wrapper", () => {
		const { env } = configureRustBuildEnv(
			{
				PATH: process.env.PATH,
				RUSTC_WRAPPER: "custom-wrapper",
			},
			{ requireTools: false },
		);

		expect(env.CARGO_INCREMENTAL).toBeUndefined();
	});
});
