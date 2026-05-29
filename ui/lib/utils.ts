import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Human-readable byte size, mirroring the CLI's `human_bytes`. */
export function humanBytes(b: number): string {
  if (!b || b < 0) return "0 B";
  const K = 1024;
  if (b < K) return `${b} B`;
  if (b < K * K) return `${(b / K).toFixed(1)} KB`;
  if (b < K * K * K) return `${(b / (K * K)).toFixed(1)} MB`;
  return `${(b / (K * K * K)).toFixed(2)} GB`;
}

/** Bytes-per-second readout. */
export function humanSpeed(bps: number): string {
  if (!bps || bps <= 0) return "—";
  return `${humanBytes(bps)}/s`;
}

/** Relative time like "just now", "5m ago", "3h ago", "2d ago", else a date. */
export function timeAgo(ts: number): string {
  const diff = Date.now() - ts;
  const s = Math.floor(diff / 1000);
  if (s < 45) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 7) return `${d}d ago`;
  return new Date(ts).toLocaleDateString();
}

/** Seconds → m:ss / h:mm:ss. */
export function humanEta(secs: number): string {
  if (!secs || secs <= 0) return "—";
  const s = Math.floor(secs % 60);
  const m = Math.floor((secs / 60) % 60);
  const h = Math.floor(secs / 3600);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}
