import { memo } from "react";
import { cn, humanBytes, humanEta, humanSpeed } from "@/lib/utils";
import type { DownloadItem, DownloadState } from "@/lib/download";
import { openDownloadPath } from "@/lib/api";
import { Check, FolderOpen, Loader2, SkipForward, TriangleAlert } from "lucide-react";

const STATE_META: Record<DownloadState, { label: string; color: string; dot: string }> = {
  starting: { label: "Starting", color: "text-[var(--color-muted)]", dot: "bg-[var(--color-muted)]" },
  downloading: { label: "Downloading", color: "text-[var(--color-accent)]", dot: "bg-[var(--color-accent)] live-dot" },
  merging: { label: "Merging", color: "text-[var(--color-warn)]", dot: "bg-[var(--color-warn)] live-dot" },
  done: { label: "Done", color: "text-[var(--color-ok)]", dot: "bg-[var(--color-ok)]" },
  skipped: {
    label: "Skipped (already in archive)",
    color: "text-[var(--color-cool)]",
    dot: "bg-[var(--color-cool)]",
  },
  failed: { label: "Failed", color: "text-[var(--color-bad)]", dot: "bg-[var(--color-bad)]" },
};

function pct(item: DownloadItem): number | null {
  if (item.state === "done" || item.state === "skipped") return 100;
  if (item.total > 0) return Math.min(100, (item.downloaded / item.total) * 100);
  return null;
}

export const DownloadCard = memo(function DownloadCard({
  item,
  index,
}: {
  item: DownloadItem;
  index: number;
}) {
  const meta = STATE_META[item.state];
  const p = pct(item);
  const heading = item.title || item.url;
  const isActive = item.state === "downloading" || item.state === "merging" || item.state === "starting";
  const animate = index <= 20;

  return (
    <div
      className={cn(
        "rounded-2xl border border-[var(--color-line)] bg-[var(--color-panel)]/80 px-5 py-4 transition-colors hover:border-[var(--color-line-strong)]",
        animate && "animate-rise",
      )}
      style={animate ? { animationDelay: `${Math.min(index * 45, 360)}ms` } : undefined}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className={cn("h-2 w-2 shrink-0 rounded-full", meta.dot)} />
            <span className={cn("text-xs font-semibold uppercase tracking-[0.14em]", meta.color)}>
              {meta.label}
            </span>
          </div>
          <p className="mt-1.5 truncate text-[15px] font-medium text-[var(--color-ink)]" title={heading}>
            {heading}
          </p>
        </div>

        <div className="shrink-0 text-right">
          {item.state === "done" && <Check className="ml-auto h-5 w-5 text-[var(--color-ok)]" />}
          {item.state === "skipped" && <SkipForward className="ml-auto h-5 w-5 text-[var(--color-cool)]" />}
          {item.state === "failed" && <TriangleAlert className="ml-auto h-5 w-5 text-[var(--color-bad)]" />}
          {isActive && <Loader2 className="ml-auto h-5 w-5 animate-spin text-[var(--color-accent)]" />}
        </div>
      </div>

      {item.state !== "failed" && (
        <div
          className="mt-3.5 h-1.5 w-full overflow-hidden rounded-full bg-[var(--color-track)]"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={p ?? undefined}
          aria-label={heading}
        >
          {p === null ? (
            <div className="h-full w-1/3 animate-sweep rounded-full bg-[var(--color-accent)]/70" />
          ) : (
            <div
              className={cn(
                "h-full rounded-full transition-[width] duration-300 ease-out",
                item.state === "done"
                  ? "bg-[var(--color-ok)]"
                  : item.state === "skipped"
                    ? "bg-[var(--color-cool)]"
                    : item.state === "merging"
                      ? "bg-[var(--color-warn)]"
                      : "bg-[var(--color-accent)]",
              )}
              style={{ width: `${p}%` }}
            />
          )}
        </div>
      )}

      <div className="mt-3 flex items-center justify-between font-mono text-xs text-[var(--color-muted)]">
        {item.state === "failed" ? (
          <span className="truncate text-[var(--color-bad)] select-text" title={item.message}>
            {item.message || "download failed"}
          </span>
        ) : (
          <>
            <span className="tabular-nums">
              {item.state === "done"
                ? humanBytes(item.bytes)
                : item.state === "skipped"
                  ? "already downloaded"
                  : item.total > 0
                    ? `${humanBytes(item.downloaded)} / ${humanBytes(item.total)}`
                    : humanBytes(item.downloaded)}
              {p !== null && item.state !== "done" && item.state !== "skipped" && (
                <span className="ml-2 text-[var(--color-faint)]">{p.toFixed(0)}%</span>
              )}
            </span>
            <span className="flex items-center gap-4 tabular-nums">
              {item.state === "downloading" && (
                <>
                  <span>{humanSpeed(item.speed)}</span>
                  <span className="text-[var(--color-faint)]">ETA {humanEta(item.eta)}</span>
                </>
              )}
              {item.path && (item.state === "done" || item.state === "skipped") && (
                <button
                  onClick={() => openDownloadPath(item.path!).catch(() => {})}
                  className="flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-[var(--color-faint)] transition-colors hover:text-[var(--color-accent)]"
                  title={item.path}
                >
                  <FolderOpen className="h-3.5 w-3.5" /> reveal
                </button>
              )}
            </span>
          </>
        )}
      </div>
    </div>
  );
});
