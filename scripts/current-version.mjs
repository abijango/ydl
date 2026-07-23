// Print the CalVer for the current HEAD (same formula CI uses at release time):
// YYYY.M.<commit-count>. Use before a local `tauri build` so the app shows
// the real version instead of the 0.1.0 placeholder.
//   node scripts/current-version.mjs   →   2026.7.13

import { execSync } from "node:child_process";

const count = Number(execSync("git rev-list --count HEAD").toString().trim());
const d = new Date();
console.log(`${d.getUTCFullYear()}.${d.getUTCMonth() + 1}.${count}`);
