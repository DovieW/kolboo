import fs from "node:fs";
import path from "node:path";

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, "utf8"));
}

function fail(msg) {
  console.error(`Version check failed: ${msg}`);
  process.exit(1);
}

function parseCargoTomlVersion(cargoTomlText) {
  // Minimal parser: find the first `version = "..."` inside the [package] section.
  const lines = cargoTomlText.split(/\r?\n/);
  let inPackage = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inPackage = trimmed === "[package]";
      continue;
    }
    if (!inPackage) continue;

    const m = trimmed.match(/^version\s*=\s*"([^"]+)"\s*$/);
    if (m) return m[1];
  }
  return null;
}

const repoRoot = process.cwd();

const appPkgPath = path.join(repoRoot, "app", "package.json");
const tauriConfPath = path.join(repoRoot, "app", "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(repoRoot, "app", "src-tauri", "Cargo.toml");

if (!fs.existsSync(appPkgPath)) fail(`Missing ${appPkgPath}`);
if (!fs.existsSync(tauriConfPath)) fail(`Missing ${tauriConfPath}`);
if (!fs.existsSync(cargoTomlPath)) fail(`Missing ${cargoTomlPath}`);

const appPkg = readJson(appPkgPath);
const tauriConf = readJson(tauriConfPath);
const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");

const vApp = appPkg.version;
const vTauri = tauriConf.version;
const vCargo = parseCargoTomlVersion(cargoToml);

if (!vApp) fail("app/package.json has no version");
if (!vTauri) fail("app/src-tauri/tauri.conf.json has no version");
if (!vCargo) fail("Could not parse version from app/src-tauri/Cargo.toml [package] section");

if (vApp !== vTauri || vApp !== vCargo) {
  fail(`Versions do not match: app=${vApp}, tauri.conf.json=${vTauri}, Cargo.toml=${vCargo}`);
}

const refType = process.env.GITHUB_REF_TYPE;
const refName = process.env.GITHUB_REF_NAME;
if (refType === "tag" && refName) {
  const tagVersion = refName.startsWith("v") ? refName.slice(1) : refName;
  if (tagVersion !== vApp) {
    fail(`Tag version (${refName}) does not match app version (${vApp})`);
  }
}

console.log(`Version check OK: ${vApp}`);
