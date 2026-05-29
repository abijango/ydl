import { clearHistory, removeHistory, revealPath, type HistoryEntry } from "@/lib/api";
import { Button, IconButton } from "./ui";
import { humanBytes, timeAgo } from "@/lib/utils";
import { FolderOpen, History, Trash2, X } from "lucide-react";

export function HistoryDialog({
  history,
  onClose,
  onChange,
}: {
  history: HistoryEntry[];
  onClose: () => void;
  onChange: (next: HistoryEntry[]) => void;
}) {
  const remove = (id: string) => {
    removeHistory(id).catch(() => {});
    onChange(history.filter((h) => h.id !== id));
  };
  const clearAll = () => {
    clearHistory().catch(() => {});
    onChange([]);
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center p-6">
      <div className="absolute inset-0 bg-[var(--color-scrim)] backdrop-blur-sm animate-fade-up" onClick={onClose} />
      <div className="animate-rise relative z-10 flex max-h-[72vh] w-full max-w-lg flex-col overflow-hidden rounded-2xl border border-[var(--color-line-strong)] bg-[var(--color-panel)] shadow-2xl">
        <div className="flex items-center justify-between border-b border-[var(--color-line)] px-6 py-4">
          <h2 className="font-display text-lg font-bold tracking-tight">History</h2>
          <div className="flex items-center gap-1">
            {history.length > 0 && (
              <Button variant="ghost" onClick={clearAll} className="px-2.5 py-1.5 text-xs text-[var(--color-bad)]">
                <Trash2 className="h-3.5 w-3.5" /> Clear all
              </Button>
            )}
            <IconButton onClick={onClose} aria-label="Close">
              <X className="h-5 w-5" />
            </IconButton>
          </div>
        </div>

        {history.length === 0 ? (
          <div className="flex flex-col items-center gap-3 px-6 py-16 text-center">
            <span className="grid h-14 w-14 place-items-center rounded-2xl border border-[var(--color-line)] bg-[var(--color-panel-2)] text-[var(--color-faint)]">
              <History className="h-6 w-6" />
            </span>
            <p className="text-sm text-[var(--color-faint)]">No downloads yet.</p>
          </div>
        ) : (
          <div className="divide-y divide-[var(--color-line)] overflow-y-auto">
            {history.map((h) => (
              <div key={h.id} className="group flex items-center gap-3 px-6 py-3">
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-[var(--color-ink)]" title={h.title ?? h.path}>
                    {h.title ?? h.path.split("/").pop() ?? h.path}
                  </p>
                  <p className="mt-0.5 font-mono text-xs text-[var(--color-faint)]">
                    {humanBytes(h.bytes)} · {timeAgo(h.ts)}
                  </p>
                </div>
                <button
                  onClick={() => revealPath(h.path).catch(() => {})}
                  className="flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-xs text-[var(--color-muted)] transition-colors hover:text-[var(--color-accent)]"
                  title={h.path}
                >
                  <FolderOpen className="h-3.5 w-3.5" /> reveal
                </button>
                <IconButton
                  onClick={() => remove(h.id)}
                  aria-label="Remove"
                  className="h-7 w-7 shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <X className="h-4 w-4" />
                </IconButton>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
