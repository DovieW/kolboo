import path from "node:path";
import { fileURLToPath } from "node:url";

const contractsDir = fileURLToPath(new URL(".", import.meta.url));
const appRoot = path.resolve(contractsDir, "../../..");
const schemasDir = path.join(appRoot, "src-tauri", "gen", "schemas");

export function resolveSchemasDir(): string {
	return schemasDir;
}

export function schemaPath(schemaFile: string): string {
	return path.join(schemasDir, schemaFile);
}
