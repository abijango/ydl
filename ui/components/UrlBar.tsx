import { useEffect, useRef, useState } from "react";
import { classifyUrl, type UrlMode } from "@/lib/api";
import { Button, Toggle } from "./ui";
import { cn } from "@/lib/utils";
import { ArrowDownToLine, ListVideo, Video, Files } from "lucide-react";

const MODE_META: Record<UrlMode, { label: string; icon: typeof Video; color: string }> = {
  single: { label: "Single video", icon: Video, color: "text-[var(--color-accent)]" },
  playlist: { label: "Playlist / channel", icon: ListVideo, color: "text-[var(--color-warn)]" },
  batch: { label: "Batch", icon: Files, color: "text-[var(--color-cool)]" },
};

export function UrlBar({
  busy,
  audioOnly,
  onAudioOnly,
  onSubmit,
}: {
  busy: boolean;
  audioOnly: boolean;
  onAudioOnly: (v: boolean) => void;
  onSubmit: (urls: string) => void;
}) {
  const [value, setValue] = useState("");
  const [mode, setMode] = useState<UrlMode | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);

  // Live mode detection, debounced. Multiple lines → batch.
  useEffect(() => {
    const lines = value.split("\n").map((l) => l.trim()).filter(Boolean);
    if (lines.length === 0) {
      setMode(null);
      return;
    }
    if (lines.length > 1) {
      setMode("batch");
      return;
    }
    const t = setTimeout(() => {
      classifyUrl(lines[0]).then(setMode).catch(() => setMode(null));
    }, 160);
    return () => clearTimeout(t);
  }, [value]);

  const submit = () => {
    const trimmed = value.trim();
    if (!trimmed || busy) return;
    onSubmit(trimmed);
    setValue("");
    setMode(null);
  };

  const Meta = mode ? MODE_META[mode] : null;

  return (
    <div className="relative">
      <div
        className={cn(
          "rounded-2xl border bg-[var(--color-panel)] p-2.5 transition-colors",
          "border-[var(--color-line-strong)] focus-within:border-[var(--color-accent)]/70",
          "shadow-[0_12px_40px_-24px_rgba(0,0,0,0.3)]",
        )}
      >
        <textarea
          ref={ref}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          rows={value.includes("\n") ? Math.min(value.split("\n").length, 5) : 1}
          spellCheck={false}
          placeholder="Paste a YouTube URL — video, playlist, channel, or one per line…"
          className="block w-full resize-none bg-transparent px-3 py-2.5 text-[15px] leading-relaxed text-[var(--color-ink)] placeholder:text-[var(--color-faint)] outline-none select-text"
        />

        <div className="flex items-center justify-between gap-3 px-1.5 pt-1">
          <div className="flex items-center gap-4">
            <div className="flex h-7 items-center gap-1.5 text-xs">
              {Meta ? (
                <>
                  <Meta.icon className={cn("h-3.5 w-3.5", Meta.color)} />
                  <span className={cn("font-medium", Meta.color)}>{Meta.label}</span>
                </>
              ) : (
                <span className="text-[var(--color-faint)]">Awaiting URL…</span>
              )}
            </div>
            <span className="h-4 w-px bg-[var(--color-line-strong)]" />
            <Toggle checked={audioOnly} onChange={onAudioOnly} label="Audio only" />
          </div>

          <Button onClick={submit} disabled={!value.trim() || busy}>
            <ArrowDownToLine className="h-4 w-4" />
            {busy ? "Working…" : "Download"}
          </Button>
        </div>
      </div>
      <p className="mt-2 pl-2 text-xs text-[var(--color-faint)]">
        <kbd className="font-mono text-[var(--color-muted)]">Enter</kbd> to download ·{" "}
        <kbd className="font-mono text-[var(--color-muted)]">Shift+Enter</kbd> for a new line
      </p>
    </div>
  );
}
