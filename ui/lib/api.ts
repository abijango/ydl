// Typed bridge to the Tauri (Rust) backend. Mirrors the serde shapes emitted by
// `ydl::event::DownloadEvent`, `ydl::summary::SummaryDto`, `ydl::config::Config`,
// and the commands in `src-tauri/src/commands.rs`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DownloadEvent =
  | { type: "expanded"; total: number; playlistTitle: string | null }
  | { type: "started"; id: number; url: string }
  | {
      type: "progress";
      id: number;
      downloaded: number;
      total: number;
      speed: number;
      eta: number;
      title: string | null;
    }
  | { type: "status"; id: number; message: string }
  | {
      type: "completed";
      id: number;
      title: string | null;
      path: string | null;
      bytes: number;
      skipped: boolean;
    }
  | { type: "failed"; id: number; error: string };

export interface SummaryRow {
  label: string;
  value: string;
}

export interface SummaryDto {
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
}

export type UrlMode = "single" | "playlist" | "batch";

// ── Commands ────────────────────────────────────────────────────────────────

export const getConfig = () => invoke<Config>("get_config");
export const saveConfig = (config: Config) => invoke<void>("save_config", { config });
export const classifyUrl = (url: string) => invoke<UrlMode>("classify_url", { url });
export const depsStatus = () => invoke<DepInfo[]>("deps_status");
export const installDeps = () => invoke<void>("install_deps");
export const updateDep = (name: string) => invoke<void>("update_dep", { name });
export const startDownload = (urls: string, audioOnly: boolean) =>
  invoke<void>("start_download", { urls, audioOnly });

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

export const onRunError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
  listen<string>("ydl://error", (e) => cb(e.payload));
