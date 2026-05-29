import type { DownloadEvent } from "./api";

export type DownloadState =
  | "starting"
  | "downloading"
  | "merging"
  | "done"
  | "skipped"
  | "failed";

export interface DownloadItem {
  id: number;
  url: string;
  title?: string;
  downloaded: number;
  total: number;
  /** Final on-disk size of the merged file, from the Completed event. */
  bytes: number;
  speed: number;
  eta: number;
  state: DownloadState;
  message?: string;
  path?: string;
}

const FINAL: DownloadState[] = ["done", "skipped", "failed"];

function classifyStatus(message: string): DownloadState | null {
  const m = message.toLowerCase();
  if (m.includes("skipped")) return "skipped";
  if (m.includes("merging")) return "merging";
  if (m.includes("error")) return "failed";
  if (m.includes("starting")) return "starting";
  return null;
}

/**
 * Fold a single backend event into the id-keyed download map. Returns a new map
 * (immutable update) so React re-renders only what changed.
 */
export function applyEvent(
  prev: Map<number, DownloadItem>,
  e: DownloadEvent,
): Map<number, DownloadItem> {
  if (e.type === "expanded") return prev; // handled at the run level
  const next = new Map(prev);
  const cur = next.get(e.id);

  switch (e.type) {
    case "started":
      next.set(e.id, {
        id: e.id,
        url: e.url,
        downloaded: 0,
        total: 0,
        bytes: 0,
        speed: 0,
        eta: 0,
        state: "starting",
      });
      break;
    case "progress":
      if (cur && !FINAL.includes(cur.state)) {
        next.set(e.id, {
          ...cur,
          downloaded: e.downloaded,
          total: e.total,
          speed: e.speed,
          eta: e.eta,
          title: e.title ?? cur.title,
          state: cur.state === "merging" ? "merging" : "downloading",
        });
      }
      break;
    case "status":
      if (cur && !FINAL.includes(cur.state)) {
        const s = classifyStatus(e.message);
        next.set(e.id, { ...cur, message: e.message, state: s ?? cur.state });
      }
      break;
    case "completed":
      if (cur) {
        next.set(e.id, {
          ...cur,
          state: e.skipped ? "skipped" : "done",
          title: e.title ?? cur.title,
          path: e.path ?? cur.path,
          downloaded: e.skipped ? cur.downloaded : cur.total || cur.downloaded,
          bytes: e.bytes, // authoritative final size of the merged file
          speed: 0,
          eta: 0,
        });
      }
      break;
    case "failed":
      if (cur) next.set(e.id, { ...cur, state: "failed", message: e.error, speed: 0, eta: 0 });
      break;
  }
  return next;
}
