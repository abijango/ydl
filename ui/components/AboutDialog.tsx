import { RELEASE_NOTES } from "@/lib/releaseNotes";
import { IconButton } from "./ui";
import { X } from "lucide-react";
import logo from "@/assets/logo.png";

export function AboutDialog({ version, onClose }: { version: string; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 grid place-items-center p-6">
      <div className="absolute inset-0 bg-[var(--color-scrim)] backdrop-blur-sm animate-fade-up" onClick={onClose} />
      <div className="animate-rise relative z-10 flex max-h-[76vh] w-full max-w-lg flex-col overflow-hidden rounded-2xl border border-[var(--color-line-strong)] bg-[var(--color-panel)] shadow-2xl">
        {/* Header */}
        <div className="flex items-start justify-between border-b border-[var(--color-line)] px-6 py-5">
          <div className="flex items-center gap-3">
            <img
              src={logo}
              alt="ydl"
              className="h-11 w-11 [filter:drop-shadow(0_2px_4px_rgba(0,0,0,0.15))]"
            />
            <div>
              <div className="flex items-baseline gap-2">
                <h2 className="font-display text-xl font-extrabold tracking-tight">ydl</h2>
                <span className="font-mono text-sm text-[var(--color-muted)]">v{version}</span>
              </div>
              <p className="text-xs text-[var(--color-faint)]">A friendlier way to download from YouTube.</p>
            </div>
          </div>
          <IconButton onClick={onClose} aria-label="Close">
            <X className="h-5 w-5" />
          </IconButton>
        </div>

        {/* What's new */}
        <div className="space-y-6 overflow-y-auto px-6 py-5">
          {RELEASE_NOTES.map((r, i) => (
            <section key={r.version}>
              <div className="mb-2.5 flex items-center gap-2">
                <span className="font-mono text-sm font-semibold text-[var(--color-ink)]">v{r.version}</span>
                {i === 0 && (
                  <span className="rounded-full bg-[var(--color-accent)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-[var(--color-accent-ink)]">
                    Latest
                  </span>
                )}
                <span className="text-xs text-[var(--color-faint)]">· {r.date}</span>
              </div>
              <p className="mb-2 text-sm font-medium text-[var(--color-ink)]">{r.headline}</p>
              <ul className="space-y-1.5">
                {r.notes.map((n, j) => (
                  <li key={j} className="text-sm leading-relaxed text-[var(--color-muted)]">
                    {n}
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
