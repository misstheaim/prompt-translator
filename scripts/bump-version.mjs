#!/usr/bin/env node
// Sets the app version everywhere. package.json is the source of truth.
//   node scripts/bump-version.mjs 0.1.1   set package.json, tauri.conf.json, Cargo.toml, Cargo.lock
//   node scripts/bump-version.mjs         re-sync the other files to package.json's version
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const path = (...p) => join(root, ...p);
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

const pkgPath = path("package.json");
const pkgText = readFileSync(pkgPath, "utf8");
const pkg = JSON.parse(pkgText);

const arg = process.argv[2];
if (arg !== undefined && !SEMVER.test(arg)) {
  console.error(`Not a valid version: "${arg}" (expected X.Y.Z, optionally with a -suffix)`);
  process.exit(1);
}
const version = arg ?? pkg.version;
if (!SEMVER.test(version)) {
  console.error(`package.json has an invalid version: "${version}"`);
  process.exit(1);
}

// Replaces exactly one match, so a changed file layout fails loudly instead of silently.
function replaceOnce(file, text, regex, replacement) {
  const matches = text.match(new RegExp(regex.source, "gm"));
  if (matches?.length !== 1) {
    console.error(
      `${file}: expected exactly one match for ${regex}, found ${matches?.length ?? 0}`,
    );
    process.exit(1);
  }
  return text.replace(regex, replacement);
}

function update(file, transform) {
  const full = path(file);
  const before = readFileSync(full, "utf8");
  const after = transform(before);
  if (after !== before) writeFileSync(full, after);
  console.log(`${file}: ${after === before ? "already" : "set to"} ${version}`);
}

update("package.json", (t) =>
  replaceOnce("package.json", t, /^(\s*"version":\s*)"[^"]*"/m, `$1"${version}"`),
);
update("src-tauri/tauri.conf.json", (t) =>
  replaceOnce("tauri.conf.json", t, /^(\s*"version":\s*)"[^"]*"/m, `$1"${version}"`),
);
update("src-tauri/Cargo.toml", (t) =>
  replaceOnce("Cargo.toml", t, /^(version\s*=\s*)"[^"]*"/m, `$1"${version}"`),
);
// Keep the lockfile in step so `cargo clippy --locked` keeps passing.
update("src-tauri/Cargo.lock", (t) =>
  replaceOnce(
    "Cargo.lock",
    t,
    /^(name = "prompt-translator"\r?\nversion = )"[^"]*"/m,
    `$1"${version}"`,
  ),
);
