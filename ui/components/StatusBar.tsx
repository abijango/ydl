import type { DepInfo } from "@/lib/api";
import { cn } from "@/lib/utils";
import { IconButton } from "./ui";
import { Settings2, Download, CircleCheck, CircleAlert, Loader2, History } from "lucide-react";
import logo from "@/assets/logo.png";

function DepPill({ dep }: { dep: DepInfo }) {
  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full border border-[var(--color-line)] bg-[var(--color-panel)] px-2.5 py-1 text-xs"
      title={dep.installed ? (dep.path ?? "installed") : "not installed — update in Settings"}
    >
      {dep.installed ? (
        <CircleCheck className="h-3.5 w-3.5 text-[var(--color-ok)]" />
      ) : (
        <CircleAlert className="h-3.5 w-3.5 text-[var(--color-bad)]" />
      )}
      <span className="font-mono text-[var(--color-muted)]">{dep.name}</span>
    </span>
  );
}

export function StatusBar({
  deps,
  installing,
  onInstall,
  onOpenSettings,
  onOpenHistory,
  version,
  onOpenAbout,
}: {
  deps: DepInfo[];
  installing: boolean;
  onInstall: () => void;
  onOpenSettings: () => void;
  onOpenHistory: () => void;
  version: string;
  onOpenAbout: () => void;
}) {
  const missing = deps.some((d) => !d.installed);

  return (
    <header
      className="flex items-center justify-between border-b border-[var(--color-line)] px-6 py-3.5"
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2.5" data-tauri-drag-region>
        <img
          src={logo}
          alt="ydl"
          className="h-7 w-7 [filter:drop-shadow(0_1px_2px_rgba(0,0,0,0.15))]"
        />
        <span className="font-display text-xl font-extrabold tracking-tight">ydl</span>
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
        {version && (
          <button
            onClick={onOpenAbout}
            title="What's new"
            className="rounded-full border border-[var(--color-line)] px-2.5 py-1 font-mono text-xs text-[var(--color-muted)] transition-colors hover:border-[var(--color-line-strong)] hover:text-[var(--color-ink)]"
          >
            v{version}
          </button>
        )}
        <button
          onClick={onOpenHistory}
          className="inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-sm font-medium text-[var(--color-muted)] transition-colors hover:bg-[var(--color-hover)] hover:text-[var(--color-ink)]"
        >
          <History className="h-4 w-4" /> History
        </button>
        <IconButton onClick={onOpenSettings} aria-label="Settings">
          <Settings2 className="h-5 w-5" />
        </IconButton>
      </div>
    </header>
  );
}
