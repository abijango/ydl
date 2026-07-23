// Typed bridge to the Tauri (Rust) backend. Mirrors the serde shapes emitted by
// `ydl::event::DownloadEvent`, `ydl::summary::SummaryDto`, `ydl::config::Config`,
// and the commands in `src-tauri/src/commands.rs`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DownloadEvent =
  | { runId: number; type: "expanded"; total: number; playlistTitle: string | null }
  | { runId: number; type: "started"; id: number; url: string }
  | {
      runId: number;
      type: "progress";
      id: number;
      downloaded: number;
      total: number;
      speed: number;
      eta: number;
      title: string | null;
    }
  | { runId: number; type: "status"; id: number; message: string }
  | {
      runId: number;
      type: "completed";
      id: number;
      title: string | null;
      path: string | null;
      bytes: number;
      skipped: boolean;
    }
  | { runId: number; type: "failed"; id: number; error: string };

export interface SummaryRow {
  label: string;
  value: string;
}

export interface SummaryDto {
  runId?: number;
  directory: string;
  elapsedMs: number;
  dryRun: boolean;
  banner: string;
  rows: SummaryRow[];
  failures: number;
}

// Config mirrors the Rust TOML model (snake_case, no serde rename).
export interface Config {
  defaults: {
    output_dir: string;
    filename_template: string;
    quality: string;
    merge_format: string;
    audio_only: boolean;
  };
  parallel: { jobs: number };
  archive: { enabled: boolean; path: string };
  ytdlp: {
    binary: string;
    auto_install: boolean;
    auto_update: boolean;
    extra_args: string[];
  };
  ffmpeg: { binary: string };
}

export interface DepInfo {
  name: string;
  installed: boolean;
  path: string | null;
  version: string | null;
  managed: boolean;
  installHint?: string | null;
}

export type UrlMode = "single" | "playlist" | "batch";

// ── Commands ────────────────────────────────────────────────────────────────

export const getConfig = () => invoke<Config>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const depsStatus = () => invoke<DepInfo[]>("deps_status");
export const installDeps = () => invoke<void>("install_deps");
export const updateDep = (name: string) => invoke<void>("update_dep", { name });
export const startDownload = (urls: string, audioOnly: boolean) =>
  invoke<number>("start_download", { urls, audioOnly });
export const cancelDownload = () => invoke<void>("cancel_download");
export const clearBusy = () => invoke<void>("clear_busy");
export const openDownloadPath = (path: string) => invoke<void>("open_download_path", { path });
export const resolveOutputDir = () => invoke<string>("resolve_output_dir");

/** Reveal a file (highlighted in its folder) or open a directory in the OS file manager. */
export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

/** The app's own version (CalVer), baked in at build time. */
export const appVersion = () => invoke<string>("app_version");

// ── Download history ──────────────────────────────────────────────────────────

export interface HistoryEntry {
  id: string;
  title: string | null;
  path: string;
  url: string | null;
  bytes: number;
  ts: number; // epoch ms
}

export const getHistory = () => invoke<HistoryEntry[]>("get_history");
export const addHistory = (entry: HistoryEntry) => invoke<void>("add_history", { entry });
export const removeHistory = (id: string) => invoke<void>("remove_history", { id });
export const clearHistory = () => invoke<void>("clear_history");

// ── Events ──────────────────────────────────────────────────────────────────

export const onDownloadEvent = (cb: (e: DownloadEvent) => void): Promise<UnlistenFn> =>
  listen<DownloadEvent>("ydl://event", (e) => cb(e.payload));

export const onSummary = (cb: (s: SummaryDto) => void): Promise<UnlistenFn> =>
  listen<SummaryDto>("ydl://summary", (e) => cb(e.payload));

export interface RunErrorPayload {
  runId: number;
  message: string;
}

export const onRunError = (cb: (e: RunErrorPayload) => void): Promise<UnlistenFn> =>
  listen<RunErrorPayload>("ydl://error", (e) => cb(e.payload));

/** Parse a command error that may be plain text or JSON `{ code, message }`. */
export function parseCommandError(e: unknown): { code?: string; message: string } {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  try {
    const parsed = JSON.parse(raw) as { code?: string; message?: string };
    if (parsed && typeof parsed.message === "string") {
      return { code: parsed.code, message: parsed.message };
    }
  } catch {
    /* plain string */
  }
  return { message: raw };
}
