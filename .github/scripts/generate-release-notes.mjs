import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

function fail(msg) {
  console.error(`Release notes generation failed: ${msg}`);
  process.exit(1);
}

function git(args) {
  try {
    return execSync(`git ${args}`, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  } catch (e) {
    const details = e?.stderr ? String(e.stderr).trim() : "";
    fail(`git ${args} failed${details ? `: ${details}` : ""}`);
  }
}

const repoRoot = process.cwd();
const changelogDir = path.join(repoRoot, "changelog.d");
const outPath = process.argv[2] ? path.resolve(process.argv[2]) : path.join(repoRoot, "RELEASE_NOTES.md");

if (!fs.existsSync(changelogDir)) fail(`Missing directory: ${changelogDir}`);

// IMPORTANT: Release notes are emitted verbatim from changelog fragment(s).
// This script must not add headings, filenames, templates, or any extra text.
//
// We consider a "release" to include the changelog fragments that changed since
// the previous version tag. This allows the repo to keep historical fragments
// without re-emitting them on every release.

const currentTagFromEnv = process.env.GITHUB_REF_NAME;
const currentTag = currentTagFromEnv
  ? currentTagFromEnv
  : (() => {
      try {
        // For local runs (or other environments), infer the latest reachable tag.
        // If HEAD is tagged, this will return that tag.
        return git('describe --tags --abbrev=0 --match "v*"');
      } catch {
        return null;
      }
    })();
const allTags = git('tag --list "v*" --sort=-v:refname')
  .split(/\r?\n/)
  .map((t) => t.trim())
  .filter(Boolean);

const previousTag = currentTag ? allTags.find((t) => t !== currentTag) ?? null : null;

let fragmentPaths = [];
if (previousTag) {
  fragmentPaths = git(`diff --name-only ${previousTag}..HEAD -- changelog.d`)
    .split(/\r?\n/)
    .map((p) => p.trim())
    .filter(Boolean)
    .filter((p) => p.startsWith("changelog.d/"))
    .filter((p) => p.toLowerCase() !== "changelog.d/readme.md")
    .filter((p) => p.endsWith(".md"))
    .sort((a, b) => a.localeCompare(b));
}

// If we couldn't determine a previous tag (e.g., first-ever release), fall back
// to including all fragments in changelog.d.
if (!previousTag) {
  fragmentPaths = fs
    .readdirSync(changelogDir)
    .filter((f) => f.endsWith(".md"))
    .filter((f) => f.toLowerCase() !== "readme.md")
    .map((f) => `changelog.d/${f}`)
    .sort((a, b) => a.localeCompare(b));
}

if (fragmentPaths.length === 0) {
  fail(
    previousTag
      ? `No changelog fragments changed since ${previousTag}. Did you forget to add one under changelog.d/?`
      : "No changelog fragments found in changelog.d (add at least one *.md file)"
  );
}

if (fragmentPaths.length === 1) {
  const only = fs.readFileSync(path.join(repoRoot, fragmentPaths[0]), "utf8");
  fs.writeFileSync(outPath, only, "utf8");
  console.log(
    `Wrote ${outPath} from 1 fragment (verbatim)${previousTag ? ` since ${previousTag}` : ""}.`
  );
  process.exit(0);
}

// If multiple fragments exist, concatenate them in a stable order with a single blank line between.
const parts = [];
for (const file of fragmentPaths) {
  const content = fs.readFileSync(path.join(repoRoot, file), "utf8").trimEnd();
  if (content.trim().length === 0) continue;
  parts.push(content);
}

if (parts.length === 0) {
  fail("All changelog fragments were empty");
}

fs.writeFileSync(outPath, parts.join("\n\n"), "utf8");
console.log(
  `Wrote ${outPath} from ${parts.length} fragment(s) (verbatim concat)${previousTag ? ` since ${previousTag}` : ""}.`
);
