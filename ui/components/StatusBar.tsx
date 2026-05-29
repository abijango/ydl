import type { DepInfo } from "@/lib/api";
import { cn } from "@/lib/utils";
import { IconButton } from "./ui";
import { Settings2, Download, CircleCheck, CircleAlert, Loader2 } from "lucide-react";

function DepPill({ dep }: { dep: DepInfo }) {
  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full border border-[var(--color-line)] bg-[var(--color-panel)] px-2.5 py-1 text-xs"
      title={dep.path ?? "not installed"}
    >
      {dep.installed ? (
        <CircleCheck className="h-3.5 w-3.5 text-[var(--color-ok)]" />
      ) : (
        <CircleAlert className="h-3.5 w-3.5 text-[var(--color-bad)]" />
      )}
      <span className="font-mono text-[var(--color-muted)]">{dep.name}</span>
      {dep.version && <span className="font-mono text-[var(--color-faint)]">{dep.version}</span>}
    </span>
  );
}

export function StatusBar({
  deps,
  installing,
  onInstall,
  onOpenSettings,
}: {
  deps: DepInfo[];
  installing: boolean;
  onInstall: () => void;
  onOpenSettings: () => void;
}) {
  const missing = deps.some((d) => !d.installed);

  return (
    <header
      className="flex items-center justify-between border-b border-[var(--color-line)] px-6 py-3.5"
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2.5" data-tauri-drag-region>
        <span className="grid h-7 w-7 place-items-center rounded-lg bg-[var(--color-accent)] text-[var(--color-accent-ink)]">
          <Download className="h-4 w-4" strokeWidth={2.5} />
        </span>
        <span className="font-display text-xl font-extrabold tracking-tight">ydl</span>
        <span className="ml-1 hidden font-mono text-[11px] text-[var(--color-faint)] sm:inline">
          youtube downloader
        </span>
      </div>

      <div className="flex items-center gap-3">
        <div className="hidden items-center gap-2 md:flex">
          {deps.map((d) => (
            <DepPill key={d.name} dep={d} />
          ))}
        </div>
        {missing && (
          <button
            onClick={onInstall}
            disabled={installing}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full border border-[var(--color-bad)]/40 bg-[var(--color-bad)]/10 px-3 py-1 text-xs font-medium text-[var(--color-bad)] transition-colors hover:bg-[var(--color-bad)]/20 disabled:opacity-60",
            )}
          >
            {installing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            {installing ? "Installing…" : "Install deps"}
          </button>
        )}
        <IconButton onClick={onOpenSettings} aria-label="Settings">
          <Settings2 className="h-5 w-5" />
        </IconButton>
      </div>
    </header>
  );
}
