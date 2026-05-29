import { revealPath, type SummaryDto } from "@/lib/api";
import { IconButton } from "./ui";
import { cn, humanEta } from "@/lib/utils";
import { CheckCircle2, FolderOpen, X, XCircle } from "lucide-react";

export function SummaryBanner({ summary, onClose }: { summary: SummaryDto; onClose: () => void }) {
  const failed = summary.failures > 0;
  // The Rust banner is plain text with a leading glyph; strip it for our own icon.
  const headline = summary.banner.replace(/^[✔✗⚠]\s*/u, "");

  return (
    <div className="animate-fade-up pointer-events-auto w-full max-w-md overflow-hidden rounded-2xl border border-[var(--color-line-strong)] bg-[var(--color-panel-2)]/95 shadow-[0_24px_70px_-20px_rgba(0,0,0,0.85)] backdrop-blur-xl">
      <div className="flex items-start gap-3 px-5 py-4">
        {failed ? (
          <XCircle className="mt-0.5 h-5 w-5 shrink-0 text-[var(--color-bad)]" />
        ) : (
          <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-[var(--color-ok)]" />
        )}
        <div className="min-w-0 flex-1">
          <p className={cn("text-sm font-semibold", failed ? "text-[var(--color-bad)]" : "text-[var(--color-ink)]")}>
            {headline}
          </p>
          <dl className="mt-2 space-y-0.5 font-mono text-xs text-[var(--color-muted)]">
            {summary.rows.map((r) => (
              <div key={r.label} className="flex gap-2">
                <dt className="w-20 shrink-0 text-[var(--color-faint)]">{r.label}</dt>
                <dd className="truncate" title={r.value}>
                  {r.value}
                </dd>
              </div>
            ))}
            {!summary.dryRun && (
              <div className="flex gap-2">
                <dt className="w-20 shrink-0 text-[var(--color-faint)]">Elapsed</dt>
                <dd>{humanEta(summary.elapsedMs / 1000)}</dd>
              </div>
            )}
          </dl>
          {!summary.dryRun && (
            <button
              onClick={() => revealPath(summary.directory).catch(() => {})}
              className="mt-3 inline-flex items-center gap-1.5 text-xs font-medium text-[var(--color-accent)] transition-opacity hover:opacity-80"
            >
              <FolderOpen className="h-3.5 w-3.5" /> Open folder
            </button>
          )}
        </div>
        <IconButton onClick={onClose} aria-label="Dismiss" className="-mr-1.5 -mt-1">
          <X className="h-4 w-4" />
        </IconButton>
      </div>
    </div>
  );
}
