import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addHistory,
  appVersion,
  depsStatus,
  getHistory,
  installDeps,
  onDownloadEvent,
  onRunError,
  onSummary,
  startDownload,
  type DepInfo,
  type HistoryEntry,
  type SummaryDto,
} from "@/lib/api";
import { applyEvent, type DownloadItem } from "@/lib/download";
import { StatusBar } from "@/components/StatusBar";
import { UrlBar } from "@/components/UrlBar";
import { DownloadCard } from "@/components/DownloadCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import { SummaryBanner } from "@/components/SummaryBanner";
import { HistoryDialog } from "@/components/HistoryDialog";
import { AboutDialog } from "@/components/AboutDialog";
import { Inbox, Loader2, TriangleAlert, X } from "lucide-react";

export default function App() {
  const [items, setItems] = useState<Map<number, DownloadItem>>(new Map());
  const [busy, setBusy] = useState(false);
  const [audioOnly, setAudioOnly] = useState(false);
  const [deps, setDeps] = useState<DepInfo[]>([]);
  const [installing, setInstalling] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [version, setVersion] = useState("");
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [summary, setSummary] = useState<SummaryDto | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [total, setTotal] = useState(0);

  // url per download id (Completed events don't carry the url; Started ones do).
  const idUrl = useRef<Record<number, string>>({});

  const refreshDeps = useCallback(() => {
    depsStatus().then(setDeps).catch(() => {});
  }, []);

  // Subscribe once. We funnel events through a ref-free reducer on state.
  const busyRef = useRef(busy);
  busyRef.current = busy;

  useEffect(() => {
    refreshDeps();
    appVersion().then(setVersion).catch(() => {});
    getHistory().then(setHistory).catch(() => {});
    const unsubs: Array<Promise<() => void>> = [
      onDownloadEvent((e) => {
        if (e.type === "expanded") {
          setTotal(e.total);
          return;
        }
        if (e.type === "started") idUrl.current[e.id] = e.url;
        // Persist each successful (non-skipped) download to history.
        if (e.type === "completed" && !e.skipped && e.path) {
          const entry: HistoryEntry = {
            id: crypto.randomUUID(),
            title: e.title ?? null,
            path: e.path,
            url: idUrl.current[e.id] ?? null,
            bytes: e.bytes,
            ts: Date.now(),
          };
          addHistory(entry).catch(() => {});
          setHistory((h) => [entry, ...h]);
        }
        setItems((prev) => applyEvent(prev, e));
      }),
      onSummary((s) => {
        setBusy(false);
        setSummary(s);
        refreshDeps();
      }),
      onRunError((msg) => {
        setBusy(false);
        setRunError(msg);
      }),
    ];
    return () => {
      unsubs.forEach((p) => p.then((fn) => fn()));
    };
  }, [refreshDeps]);

  const handleSubmit = (urls: string) => {
    setItems(new Map());
    setSummary(null);
    setRunError(null);
    setTotal(urls.split("\n").filter((l) => l.trim()).length);
    setBusy(true);
    startDownload(urls, audioOnly).catch((e) => {
      setBusy(false);
      setRunError(String(e));
    });
  };

  const handleInstall = () => {
    setInstalling(true);
    installDeps()
      .then(refreshDeps)
      .catch((e) => setRunError(String(e)))
      .finally(() => setInstalling(false));
  };

  const list = useMemo(
    () => [...items.values()].sort((a, b) => a.id - b.id),
    [items],
  );
  const finished = list.filter((i) => ["done", "skipped", "failed"].includes(i.state)).length;

  return (
    <div className="relative z-10 flex h-full flex-col">
      <StatusBar
        deps={deps}
        installing={installing}
        onInstall={handleInstall}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenHistory={() => setHistoryOpen(true)}
        version={version}
        onOpenAbout={() => setAboutOpen(true)}
      />

      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-7 overflow-hidden px-6 pt-10">
        <UrlBar busy={busy} audioOnly={audioOnly} onAudioOnly={setAudioOnly} onSubmit={handleSubmit} />

        {/* Queue header */}
        {(list.length > 0 || busy) && (
          <div className="flex items-center justify-between">
            <h2 className="text-xs font-semibold uppercase tracking-[0.16em] text-[var(--color-faint)]">
              Downloads
            </h2>
            <span className="font-mono text-xs text-[var(--color-faint)]">
              {finished}/{total || list.length}
            </span>
          </div>
        )}

        {/* Scrollable list */}
        <div className="-mr-2 flex-1 space-y-2.5 overflow-y-auto pr-2 pb-8">
          {list.length === 0 ? (
            busy ? (
              <div className="mt-10 flex flex-col items-center gap-3 text-center">
                <Loader2 className="h-6 w-6 animate-spin text-[var(--color-accent)]" />
                <p className="text-sm text-[var(--color-faint)]">
                  Fetching details…{total > 1 ? " (enumerating playlist)" : ""}
                </p>
              </div>
            ) : (
              <div className="mt-10 flex flex-col items-center gap-3 text-center">
                <span className="grid h-14 w-14 place-items-center rounded-2xl border border-[var(--color-line)] bg-[var(--color-panel)] text-[var(--color-faint)]">
                  <Inbox className="h-6 w-6" />
                </span>
                <p className="text-sm text-[var(--color-faint)]">No downloads yet.</p>
              </div>
            )
          ) : (
            list.map((item, i) => <DownloadCard key={item.id} item={item} index={i} />)
          )}
        </div>
      </main>

      {/* Floating toasts */}
      <div className="pointer-events-none fixed bottom-5 right-5 z-40 flex flex-col items-end gap-3">
        {runError && (
          <div className="animate-fade-up pointer-events-auto flex max-w-md items-start gap-3 rounded-2xl border border-[var(--color-bad)]/40 bg-[var(--color-panel-2)]/95 px-5 py-4 shadow-2xl backdrop-blur-xl">
            <TriangleAlert className="mt-0.5 h-5 w-5 shrink-0 text-[var(--color-bad)]" />
            <p className="flex-1 text-sm text-[var(--color-ink)]">{runError}</p>
            <button onClick={() => setRunError(null)} className="text-[var(--color-faint)] hover:text-[var(--color-ink)]">
              <X className="h-4 w-4" />
            </button>
          </div>
        )}
        {summary && <SummaryBanner summary={summary} onClose={() => setSummary(null)} />}
      </div>

      {settingsOpen && (
        <SettingsDialog onClose={() => setSettingsOpen(false)} onDepsChanged={refreshDeps} />
      )}
      {historyOpen && (
        <HistoryDialog
          history={history}
          onClose={() => setHistoryOpen(false)}
          onChange={setHistory}
        />
      )}
      {aboutOpen && <AboutDialog version={version || "0.0.0"} onClose={() => setAboutOpen(false)} />}
    </div>
  );
}
