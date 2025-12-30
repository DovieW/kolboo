import fs from "node:fs";
import path from "node:path";

function fail(msg) {
  console.error(`Release notes generation failed: ${msg}`);
  process.exit(1);
}

const repoRoot = process.cwd();
const changelogDir = path.join(repoRoot, "changelog.d");
const outPath = process.argv[2] ? path.resolve(process.argv[2]) : path.join(repoRoot, "RELEASE_NOTES.md");

if (!fs.existsSync(changelogDir)) fail(`Missing directory: ${changelogDir}`);

const entries = fs
  .readdirSync(changelogDir)
  .filter((f) => f.endsWith(".md"))
  .filter((f) => f.toLowerCase() !== "readme.md")
  .sort((a, b) => a.localeCompare(b));

if (entries.length === 0) {
  fail("No changelog fragments found in changelog.d (add at least one *.md file)");
}

// IMPORTANT: Release notes are emitted verbatim from changelog fragment(s).
// This script must not add headings, filenames, templates, or any extra text.

if (entries.length === 1) {
  const only = fs.readFileSync(path.join(changelogDir, entries[0]), "utf8");
  fs.writeFileSync(outPath, only, "utf8");
  console.log(`Wrote ${outPath} from 1 fragment (verbatim).`);
  process.exit(0);
}

// If multiple fragments exist, concatenate them in a stable order with a single blank line between.
const parts = [];
for (const file of entries) {
  const content = fs.readFileSync(path.join(changelogDir, file), "utf8").trimEnd();
  if (content.trim().length === 0) continue;
  parts.push(content);
}

if (parts.length === 0) {
  fail("All changelog fragments were empty");
}

fs.writeFileSync(outPath, parts.join("\n\n"), "utf8");
console.log(`Wrote ${outPath} from ${parts.length} fragment(s) (verbatim concat).`);
