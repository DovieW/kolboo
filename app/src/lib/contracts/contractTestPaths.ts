import path from "node:path";
import { fileURLToPath } from "node:url";

const contractsDir = fileURLToPath(new URL(".", import.meta.url));
const appRoot = path.resolve(contractsDir, "../../..");
const schemasDir = path.join(appRoot, "src-tauri", "gen", "schemas");
const rustSrcDir = path.join(appRoot, "src-tauri", "src");
const rustLibPath = path.join(rustSrcDir, "lib.rs");
const tauriTsPath = path.join(appRoot, "src", "lib", "tauri.ts");

export function resolveSchemasDir(): string {
	return schemasDir;
}

export function schemaPath(schemaFile: string): string {
	return path.join(schemasDir, schemaFile);
}

export function resolveRustSrcDir(): string {
	return rustSrcDir;
}

export function resolveRustLibPath(): string {
	return rustLibPath;
}

export function resolveTauriTsPath(): string {
	return tauriTsPath;
}
