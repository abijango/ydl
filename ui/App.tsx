import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addHistory,
  appVersion,
  cancelDownload,
  clearBusy,
  depsStatus,
  getConfig,
  getHistory,
  installDeps,
  onDownloadEvent,
  onRunError,
  onSummary,
  resolveOutputDir,
  saveConfig,
  startDownload,
  parseCommandError,
  type DepInfo,
  type HistoryEntry,
  type SummaryDto,
} from "@/lib/api";
import { applyEvent, type DownloadItem } from "@/lib/download";
import { QUALITY_PRESETS } from "@/lib/quality";
import { StatusBar } from "@/components/StatusBar";
import { UrlBar } from "@/components/UrlBar";
import { DownloadCard } from "@/components/DownloadCard";
import { SummaryBanner } from "@/components/SummaryBanner";
import { HistoryDialog } from "@/components/HistoryDialog";
import { AboutDialog } from "@/components/AboutDialog";
import { Button } from "@/components/ui";
import { FolderOpen, Inbox, Loader2, Settings2, SlidersHorizontal, TriangleAlert, X } from "lucide-react";

const SettingsDialog = lazy(() =>
  import("@/components/SettingsDialog").then((m) => ({ default: m.SettingsDialog })),
);

const ROW_HEIGHT = 88;

export default function App() {
  const [items, setItems] = useState<Map<number, DownloadItem>>(new Map());
  const [busy, setBusy] = useState(false);
  const [audioOnly, setAudioOnly] = useState(false);
  const [urlValue, setUrlValue] = useState("");
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
  const [playlistTitle, setPlaylistTitle] = useState<string | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [qualityLabel, setQualityLabel] = useState<string>("Best available");
  const [onboardOpen, setOnboardOpen] = useState(false);

  const itemsRef = useRef<Map<number, DownloadItem>>(new Map());
  const activeRunIdRef = useRef<number | null>(null);
  const rafRef = useRef<number | null>(null);
  const commitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastSubmittedUrlRef = useRef("");

  // url per download id (Completed events don't carry the url; Started ones do).
  const idUrl = useRef<Record<number, string>>({});

  const refreshDeps = useCallback(() => {
    depsStatus().then(setDeps).catch(() => {});
  }, []);

  const refreshOutputInfo = useCallback(() => {
    resolveOutputDir()
      .then(setOutputPath)
      .catch(() => {});
    getConfig()
      .then((c) => {
        const preset = QUALITY_PRESETS.find((p) => p.value === c.defaults.quality);
        setQualityLabel(preset?.label ?? "Custom");
      })
      .catch(() => {});
  }, []);

  const scheduleCommit = useCallback(() => {
    if (rafRef.current === null) {
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = null;
        if (commitTimerRef.current !== null) {
          clearTimeout(commitTimerRef.current);
          commitTimerRef.current = null;
        }
        setItems(new Map(itemsRef.current));
      });
    }
    if (commitTimerRef.current === null) {
      commitTimerRef.current = setTimeout(() => {
        commitTimerRef.current = null;
        if (rafRef.current !== null) {
          cancelAnimationFrame(rafRef.current);
          rafRef.current = null;
        }
        setItems(new Map(itemsRef.current));
      }, 100);
    }
  }, []);

  useEffect(() => {
    refreshDeps();
    appVersion().then(setVersion).catch(() => {});
    getHistory().then(setHistory).catch(() => {});
    getConfig()
      .then((c) => {
        setAudioOnly(c.defaults.audio_only);
        const preset = QUALITY_PRESETS.find((p) => p.value === c.defaults.quality);
        setQualityLabel(preset?.label ?? "Custom");
        const out = c.defaults.output_dir.trim();
        if ((!out || out === ".") && localStorage.getItem("yd.outputOnboarded") !== "1") {
          setOnboardOpen(true);
        }
      })
      .catch(() => {});
    refreshOutputInfo();

    const unsubs: Array<Promise<() => void>> = [
      onDownloadEvent((e) => {
        if (e.runId !== activeRunIdRef.current) return;
        if (e.type === "expanded") {
          setTotal(e.total);
          setPlaylistTitle(e.playlistTitle);
          return;
        }
        if (e.type === "started") idUrl.current[e.id] = e.url;
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
        itemsRef.current = applyEvent(itemsRef.current, e);
        scheduleCommit();
      }),
      onSummary((s) => {
        if (s.runId !== undefined && s.runId !== activeRunIdRef.current) return;
        setBusy(false);
        clearBusy().catch(() => {});
        setSummary(s);
        setUrlValue("");
        refreshDeps();
        refreshOutputInfo();
      }),
      onRunError((e) => {
        if (e.runId !== activeRunIdRef.current) return;
        setBusy(false);
        clearBusy().catch(() => {});
        setRunError(e.message);
      }),
    ];
    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
      if (commitTimerRef.current !== null) clearTimeout(commitTimerRef.current);
      unsubs.forEach((p) => p.then((fn) => fn()));
    };
  }, [refreshDeps, refreshOutputInfo, scheduleCommit]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        setSettingsOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const handleSubmit = (urls: string) => {
    itemsRef.current = new Map();
    setItems(new Map());
    setSummary(null);
    setRunError(null);
    setPlaylistTitle(null);
    lastSubmittedUrlRef.current = urls;
    setTotal(urls.split("\n").filter((l) => l.trim()).length);
    setBusy(true);
    startDownload(urls, audioOnly)
      .then((runId) => {
        activeRunIdRef.current = runId;
      })
      .catch((e) => {
        setBusy(false);
        setRunError(parseCommandError(e).message);
      });
  };

  const handleCancel = () => {
    cancelDownload().catch(() => {});
  };

  const handleInstall = () => {
    setInstalling(true);
    installDeps()
      .then(refreshDeps)
      .catch((e) => setRunError(parseCommandError(e).message))
      .finally(() => setInstalling(false));
  };

  const handleOnboardPick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;
    try {
      const cfg = await getConfig();
      await saveConfig({ ...cfg, defaults: { ...cfg.defaults, output_dir: dir } });
      localStorage.setItem("yd.outputOnboarded", "1");
      setOutputPath(dir);
      setOnboardOpen(false);
    } catch (e) {
      setRunError(parseCommandError(e).message);
    }
  };

  const handleOnboardSkip = () => {
    localStorage.setItem("yd.outputOnboarded", "1");
    setOnboardOpen(false);
  };

  const list = useMemo(
    () => [...items.values()].sort((a, b) => a.id - b.id),
    [items],
  );
  const finished = list.filter((i) => ["done", "skipped", "failed"].includes(i.state)).length;
  const virtualize = list.length > 40;

  const virtualizer = useVirtualizer({
    count: list.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 5,
  });

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

      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-7 overflow-hidden px-6 pt-8">
        <div className="flex flex-wrap items-center gap-2">
          {outputPath && (
            <button
              type="button"
              onClick={() => setSettingsOpen(true)}
              className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-[var(--color-line)] bg-[var(--color-panel)] px-3 py-1 text-xs text-[var(--color-muted)] transition-colors hover:border-[var(--color-line-strong)] hover:text-[var(--color-ink)]"
              title={outputPath}
            >
              <FolderOpen className="h-3.5 w-3.5 shrink-0" />
              <span className="truncate font-mono">{outputPath}</span>
            </button>
          )}
          <button
            type="button"
            onClick={() => setSettingsOpen(true)}
            className="inline-flex items-center gap-1.5 rounded-full border border-[var(--color-line)] bg-[var(--color-panel)] px-3 py-1 text-xs text-[var(--color-muted)] transition-colors hover:border-[var(--color-line-strong)] hover:text-[var(--color-ink)]"
          >
            <SlidersHorizontal className="h-3.5 w-3.5" />
            {qualityLabel}
          </button>
        </div>

        <UrlBar
          busy={busy}
          audioOnly={audioOnly}
          onAudioOnly={setAudioOnly}
          value={urlValue}
          onChange={setUrlValue}
          onSubmit={handleSubmit}
        />

        {(list.length > 0 || busy) && (
          <div className="flex items-center justify-between gap-3">
            <div className="min-w-0">
              <h2 className="text-xs font-semibold uppercase tracking-[0.16em] text-[var(--color-faint)]">
                Downloads
              </h2>
              {playlistTitle && (
                <p className="mt-0.5 truncate text-sm text-[var(--color-muted)]" title={playlistTitle}>
                  {playlistTitle}
                </p>
              )}
            </div>
            <div className="flex shrink-0 items-center gap-3">
              <span className="font-mono text-xs text-[var(--color-faint)]">
                {finished}/{total || list.length}
              </span>
              {busy && (
                <Button variant="outline" onClick={handleCancel} className="px-3 py-1.5 text-xs">
                  Cancel
                </Button>
              )}
            </div>
          </div>
        )}

        <div ref={scrollRef} className="-mr-2 flex-1 overflow-y-auto pr-2 pb-8">
          {list.length === 0 && !busy ? (
            <div className="mt-10 flex flex-col items-center gap-3 text-center">
              <span className="grid h-14 w-14 place-items-center rounded-2xl border border-[var(--color-line)] bg-[var(--color-panel)] text-[var(--color-faint)]">
                <Inbox className="h-6 w-6" />
              </span>
              <p className="text-sm text-[var(--color-faint)]">No downloads yet.</p>
            </div>
          ) : list.length === 0 && busy ? (
            <div className="mt-10 flex flex-col items-center gap-3 text-center">
              <Loader2 className="h-8 w-8 animate-spin text-[var(--color-accent)]" />
              <p className="text-sm text-[var(--color-muted)]">
                Preparing download…{total > 1 ? " (enumerating playlist)" : ""}
              </p>
            </div>
          ) : virtualize ? (
            <div
              className="relative w-full"
              style={{ height: `${virtualizer.getTotalSize()}px` }}
            >
              {virtualizer.getVirtualItems().map((vi) => {
                const item = list[vi.index];
                return (
                  <div
                    key={item.id}
                    className="absolute left-0 top-0 w-full pb-2.5"
                    style={{ transform: `translateY(${vi.start}px)` }}
                  >
                    <DownloadCard item={item} index={vi.index} />
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="space-y-2.5">
              {list.map((item, i) => (
                <DownloadCard key={item.id} item={item} index={i} />
              ))}
            </div>
          )}
        </div>
      </main>

      <div className="pointer-events-none fixed bottom-5 right-5 z-40 flex flex-col items-end gap-3">
        {runError && (
          <div className="animate-fade-up pointer-events-auto flex max-w-md items-start gap-3 rounded-2xl border border-[var(--color-bad)]/40 bg-[var(--color-panel-2)]/95 px-5 py-4 shadow-2xl backdrop-blur-xl">
            <TriangleAlert className="mt-0.5 h-5 w-5 shrink-0 text-[var(--color-bad)]" />
            <p className="flex-1 text-sm text-[var(--color-ink)] select-text">{runError}</p>
            <div className="flex shrink-0 items-center gap-2">
              {lastSubmittedUrlRef.current && (
                <Button
                  variant="outline"
                  onClick={() => {
                    setRunError(null);
                    handleSubmit(lastSubmittedUrlRef.current);
                  }}
                  className="px-3 py-1.5 text-xs"
                >
                  Retry
                </Button>
              )}
              <button
                onClick={() => setRunError(null)}
                className="text-[var(--color-faint)] hover:text-[var(--color-ink)]"
                aria-label="Dismiss error"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          </div>
        )}
        {summary && <SummaryBanner summary={summary} onClose={() => setSummary(null)} />}
      </div>

      {onboardOpen && (
        <div className="fixed inset-0 z-50 grid place-items-center p-6">
          <div className="absolute inset-0 bg-[var(--color-scrim)] backdrop-blur-sm" />
          <div className="relative z-10 w-full max-w-md rounded-2xl border border-[var(--color-line-strong)] bg-[var(--color-panel)] p-6 shadow-2xl">
            <div className="mb-1 flex items-center gap-2">
              <Settings2 className="h-5 w-5 text-[var(--color-accent)]" />
              <h2 className="font-display text-lg font-bold">Choose download folder</h2>
            </div>
            <p className="mt-2 text-sm text-[var(--color-muted)]">
              Pick where ydl saves your files. You can change this anytime in Settings.
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <Button variant="ghost" onClick={handleOnboardSkip}>
                Skip for now
              </Button>
              <Button onClick={handleOnboardPick}>
                <FolderOpen className="h-4 w-4" /> Choose folder
              </Button>
            </div>
          </div>
        </div>
      )}

      {settingsOpen && (
        <Suspense
          fallback={
            <div className="fixed inset-0 z-50 grid place-items-center bg-[var(--color-scrim)]">
              <Loader2 className="h-8 w-8 animate-spin text-[var(--color-accent)]" />
            </div>
          }
        >
          <SettingsDialog
            onClose={() => {
              setSettingsOpen(false);
              refreshOutputInfo();
              getConfig()
                .then((c) => setAudioOnly(c.defaults.audio_only))
                .catch(() => {});
            }}
            onDepsChanged={refreshDeps}
          />
        </Suspense>
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
