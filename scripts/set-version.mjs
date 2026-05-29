// Stamp a computed version into the files Tauri reads, at build time only.
// Usage: node scripts/set-version.mjs <version>
// Called by CI so the version is never committed back to the repo (no release loop).

import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!version) {
  console.error("usage: node scripts/set-version.mjs <version>");
  process.exit(1);
}
// Guard: Tauri requires valid semver MAJOR.MINOR.PATCH (no leading zeros).
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`invalid version: ${version} (expected MAJOR.MINOR.PATCH)`);
  process.exit(1);
}

for (const path of ["src-tauri/tauri.conf.json", "package.json"]) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  json.version = version;
  writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
  console.log(`set ${path} → ${version}`);
}
