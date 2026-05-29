// Print the CalVer that the NEXT commit will produce: YYYY.M.<commit-count + 1>.
// Use it to key a new entry in ui/release-notes.json before committing a release.
//   node scripts/next-version.mjs   →   2026.5.8

import { execSync } from "node:child_process";

const count = Number(execSync("git rev-list --count HEAD").toString().trim()) + 1;
const d = new Date();
console.log(`${d.getUTCFullYear()}.${d.getUTCMonth() + 1}.${count}`);
