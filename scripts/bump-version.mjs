#!/usr/bin/env node
// Une seule version pour tout le projet : Cargo, npm, Tauri et l'extension Chromium.
//   node scripts/bump-version.mjs patch|minor|major|1.2.3   change la version et date le CHANGELOG
//   node scripts/bump-version.mjs --check                   vérifie que tous les fichiers sont alignés
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (f) => readFileSync(join(root, f), "utf8");
const write = (f, s) => writeFileSync(join(root, f), s);
const CARGO_VERSION = /(\[workspace\.package\][\s\S]*?\nversion = ")([^"]+)(")/;
const JSON_FILES = ["package.json", "src-tauri/tauri.conf.json", "extensions/chromium/manifest.json"];

const cargoVersion = () => read("Cargo.toml").match(CARGO_VERSION)[2];

function check() {
  const expected = cargoVersion();
  const wrong = JSON_FILES.filter((f) => JSON.parse(read(f)).version !== expected);
  if (wrong.length) {
    console.error(`Version ${expected} attendue, mais différente dans : ${wrong.join(", ")}`);
    process.exit(1);
  }
  console.log(`Version ${expected} partout.`);
}

function next(current, arg) {
  if (/^\d+\.\d+\.\d+$/.test(arg)) return arg;
  const [major, minor, patch] = current.split(".").map(Number);
  if (arg === "major") return `${major + 1}.0.0`;
  if (arg === "minor") return `${major}.${minor + 1}.0`;
  if (arg === "patch") return `${major}.${minor}.${patch + 1}`;
  console.error("Usage : bump-version.mjs patch|minor|major|x.y.z|--check");
  process.exit(1);
}

function bump(arg) {
  const current = cargoVersion();
  const version = next(current, arg);
  write("Cargo.toml", read("Cargo.toml").replace(CARGO_VERSION, `$1${version}$3`));
  for (const f of JSON_FILES) {
    write(f, read(f).replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
  }
  const date = new Date().toISOString().slice(0, 10);
  const base = "https://github.com/blacktermi/tribal-grab";
  write(
    "CHANGELOG.md",
    read("CHANGELOG.md")
      .replace("## [Non publié]\n", `## [Non publié]\n\n## [${version}] - ${date}\n`)
      .replace(/\[Non publié\]: .*\n/, `[Non publié]: ${base}/compare/v${version}...HEAD\n[${version}]: ${base}/compare/v${current}...v${version}\n`),
  );
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], { cwd: root, stdio: "ignore" });
  console.log(`${current} -> ${version}`);
  console.log(`Ensuite : git commit -am "chore(release): v${version}" && git tag v${version}`);
}

const arg = process.argv[2];
if (arg === "--check") check();
else bump(arg ?? "");
