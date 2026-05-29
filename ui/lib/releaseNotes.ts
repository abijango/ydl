// The in-app "What's new" notes. The SAME file (ui/release-notes.json) is read
// by the release workflow to build the GitHub release body, so the two never
// drift. Newest entry first; key each `version` to the CalVer it ships in
// (run `node scripts/next-version.mjs` to get the next version).

import data from "@/release-notes.json";

export interface ReleaseNote {
  version: string;
  date: string;
  headline: string;
  notes: string[];
}

export const RELEASE_NOTES: ReleaseNote[] = data as ReleaseNote[];
