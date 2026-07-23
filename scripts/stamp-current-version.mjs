// Stamp the current HEAD CalVer into package.json + tauri.conf.json for a
// local release build. CI uses set-version.mjs with its own computed version;
// this wraps that for `npm run tauri build` so you don't ship as 0.1.0.
// Changes are working-tree only — do not commit them (same rule as CI).

import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = execSync("node scripts/current-version.mjs", { cwd: root })
  .toString()
  .trim();
execSync(`node scripts/set-version.mjs ${version}`, { cwd: root, stdio: "inherit" });
