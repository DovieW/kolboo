import fs from "node:fs";

import { resolveSchemasDir, schemaPath } from "../contractTestPaths";

export function hasSchemas(): boolean {
	const schemasDir = resolveSchemasDir();
	return fs.existsSync(schemasDir) && fs.readdirSync(schemasDir).length > 0;
}

export function readSchemaJson<T>(schemaFile: string): T {
	const resolvedPath = schemaPath(schemaFile);
	if (!fs.existsSync(resolvedPath)) {
		throw new Error(`Schema missing: ${schemaFile}`);
	}
	const rawSchema = fs
		.readFileSync(resolvedPath, "utf8")
		.replace(/^\uFEFF/, "");
	return JSON.parse(rawSchema) as T;
}
