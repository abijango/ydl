import { useCallback, useEffect, useState } from "react";
import {
  depsStatus,
  getConfig,
  installDeps,
  saveConfig,
  updateDep,
  type Config,
  type DepInfo,
} from "@/lib/api";
import { Button, IconButton, SelectField, TextField, Toggle } from "./ui";
import { cn } from "@/lib/utils";
import { CUSTOM_QUALITY, QUALITY_PRESETS, isCustomQuality } from "@/lib/quality";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, CircleAlert, CircleCheck, FolderSearch, Loader2, RefreshCw, X } from "lucide-react";

export function SettingsDialog({
  onClose,
  onDepsChanged,
}: {
  onClose: () => void;
  onDepsChanged?: () => void;
}) {
  const [cfg, setCfg] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [deps, setDeps] = useState<DepInfo[]>([]);
  const [depBusy, setDepBusy] = useState<string | null>(null);
  const [depError, setDepError] = useState<string | null>(null);
  const [depDone, setDepDone] = useState<{ name: string; msg: string } | null>(null);
  const [qualityCustom, setQualityCustom] = useState(false);

  const loadDeps = useCallback(() => {
    depsStatus()
      .then(setDeps)
      .catch(() => {});
  }, []);

  useEffect(() => {
    getConfig()
      .then((c) => {
        setCfg(c);
        setQualityCustom(isCustomQuality(c.defaults.quality));
      })
      .catch(() => onClose());
    loadDeps();
  }, [onClose, loadDeps]);

  const runDepAction = async (key: string, fn: () => Promise<void>) => {
    setDepBusy(key);
    setDepError(null);
    try {
      await fn();
      loadDeps();
      onDepsChanged?.();
    } catch (e) {
      setDepError(String(e));
    } finally {
      setDepBusy(null);
    }
  };

  // Per-tool update with explicit feedback: spinner → "Updated → x.y" or
  // "Up to date" (since re-pulling an already-latest tool changes nothing visible).
  const doUpdate = async (dep: DepInfo) => {
    setDepBusy(dep.name);
    setDepError(null);
    setDepDone(null);
    const before = dep.version;
    try {
      await updateDep(dep.name);
      const fresh = await depsStatus();
      setDeps(fresh);
      onDepsChanged?.();
      const after = fresh.find((d) => d.name === dep.name)?.version ?? null;
      const changed = before && after && before !== after;
      setDepDone({ name: dep.name, msg: changed ? `Updated → ${after}` : "Up to date" });
      window.setTimeout(
        () => setDepDone((cur) => (cur?.name === dep.name ? null : cur)),
        2600,
      );
    } catch (e) {
      setDepError(String(e));
    } finally {
      setDepBusy(null);
    }
  };

  const anyMissing = deps.some((d) => !d.installed);

  const patch = <K extends keyof Config>(key: K, value: Config[K]) =>
    setCfg((c) => (c ? { ...c, [key]: value } : c));

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string" && cfg) {
      patch("defaults", { ...cfg.defaults, output_dir: dir });
    }
  };

  const save = async () => {
    if (!cfg) return;
    setSaving(true);
    try {
      await saveConfig(cfg);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center p-6">
      <div className="absolute inset-0 bg-[var(--color-scrim)] backdrop-blur-sm animate-fade-up" onClick={onClose} />
      <div className="animate-rise relative z-10 w-full max-w-lg overflow-hidden rounded-2xl border border-[var(--color-line-strong)] bg-[var(--color-panel)] shadow-2xl">
        <div className="flex items-center justify-between border-b border-[var(--color-line)] px-6 py-4">
          <h2 className="font-display text-lg font-bold tracking-tight">Settings</h2>
          <IconButton onClick={onClose} aria-label="Close">
            <X className="h-5 w-5" />
          </IconButton>
        </div>

        {cfg ? (
          <div className="max-h-[60vh] space-y-5 overflow-y-auto px-6 py-5">
            <div>
              <span className="mb-1.5 block text-xs font-medium uppercase tracking-[0.14em] text-[var(--color-faint)]">
                Output directory
              </span>
              <div className="flex gap-2">
                <input
                  value={cfg.defaults.output_dir}
                  onChange={(e) => patch("defaults", { ...cfg.defaults, output_dir: e.target.value })}
                  className="w-full rounded-lg border border-[var(--color-line)] bg-[var(--color-panel-2)] px-3.5 py-2.5 text-sm outline-none transition-colors focus:border-[var(--color-accent)]/60 select-text"
                />
                <Button variant="outline" onClick={pickDir} className="shrink-0">
                  <FolderSearch className="h-4 w-4" /> Browse
                </Button>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <SelectField
                label="Quality"
                value={qualityCustom ? CUSTOM_QUALITY : cfg.defaults.quality}
                onChange={(e) => {
                  if (e.target.value === CUSTOM_QUALITY) {
                    setQualityCustom(true);
                  } else {
                    setQualityCustom(false);
                    patch("defaults", { ...cfg.defaults, quality: e.target.value });
                  }
                }}
              >
                {QUALITY_PRESETS.map((p) => (
                  <option key={p.value} value={p.value}>
                    {p.label}
                  </option>
                ))}
                <option value={CUSTOM_QUALITY}>Custom…</option>
              </SelectField>
              <SelectField
                label="Merge format"
                value={cfg.defaults.merge_format}
                onChange={(e) => patch("defaults", { ...cfg.defaults, merge_format: e.target.value })}
              >
                {["mp4", "mkv", "webm"].map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </SelectField>
            </div>

            {qualityCustom && (
              <TextField
                label="Custom format selector"
                value={cfg.defaults.quality}
                onChange={(e) => patch("defaults", { ...cfg.defaults, quality: e.target.value })}
                hint="Raw yt-dlp -f selector, e.g. bv*+ba/b"
                placeholder="bv*+ba/b"
                spellCheck={false}
              />
            )}

            <TextField
              label="Filename template"
              value={cfg.defaults.filename_template}
              onChange={(e) => patch("defaults", { ...cfg.defaults, filename_template: e.target.value })}
              hint="Tokens like {upload_date}, {title}, {ext}"
            />

            <div className="grid grid-cols-2 gap-4">
              <TextField
                label="Parallel jobs"
                type="number"
                min={1}
                max={16}
                value={cfg.parallel.jobs}
                onChange={(e) =>
                  patch("parallel", { jobs: Math.max(1, Number(e.target.value) || 1) })
                }
              />
              <div className="flex items-end gap-6 pb-1">
                <Toggle
                  checked={cfg.defaults.audio_only}
                  onChange={(v) => patch("defaults", { ...cfg.defaults, audio_only: v })}
                  label="Audio only"
                />
                <Toggle
                  checked={cfg.archive.enabled}
                  onChange={(v) => patch("archive", { ...cfg.archive, enabled: v })}
                  label="Archive"
                />
              </div>
            </div>

            {/* Dependencies */}
            <div className="border-t border-[var(--color-line)] pt-5">
              <div className="mb-3 flex items-center justify-between">
                <span className="text-xs font-medium uppercase tracking-[0.14em] text-[var(--color-faint)]">
                  Dependencies
                </span>
                <button
                  onClick={() => runDepAction("recheck", async () => {})}
                  disabled={depBusy !== null}
                  className="inline-flex items-center gap-1.5 text-xs text-[var(--color-muted)] transition-colors hover:text-[var(--color-ink)] disabled:opacity-50"
                >
                  <RefreshCw className={cn("h-3.5 w-3.5", depBusy === "recheck" && "animate-spin")} />
                  Re-check
                </button>
              </div>

              <div className="space-y-2">
                {deps.map((d) => (
                  <div
                    key={d.name}
                    className="flex items-center justify-between rounded-lg border border-[var(--color-line)] bg-[var(--color-panel-2)] px-3 py-2.5"
                  >
                    <div className="flex min-w-0 items-center gap-2">
                      {d.installed ? (
                        <CircleCheck className="h-4 w-4 shrink-0 text-[var(--color-ok)]" />
                      ) : (
                        <CircleAlert className="h-4 w-4 shrink-0 text-[var(--color-bad)]" />
                      )}
                      <span className="font-mono text-sm text-[var(--color-ink)]">{d.name}</span>
                      <span className="truncate font-mono text-xs text-[var(--color-faint)]">
                        {d.version ?? (d.installed ? "installed" : "not installed")}
                      </span>
                      {d.installed && !d.managed && (
                        <span className="rounded-full border border-[var(--color-line)] px-2 py-0.5 text-[10px] uppercase tracking-wider text-[var(--color-faint)]">
                          system
                        </span>
                      )}
                    </div>
                    {d.managed && (
                      <div className="flex w-[140px] shrink-0 justify-end">
                        {depDone?.name === d.name ? (
                          <span className="inline-flex items-center gap-1.5 text-xs font-medium text-[var(--color-ok)]">
                            <Check className="h-3.5 w-3.5" /> {depDone.msg}
                          </span>
                        ) : (
                          <Button
                            variant="outline"
                            onClick={() => doUpdate(d)}
                            disabled={depBusy !== null}
                            className="px-3 py-1.5 text-xs"
                          >
                            {depBusy === d.name ? (
                              <>
                                <Loader2 className="h-3.5 w-3.5 animate-spin" /> Updating…
                              </>
                            ) : (
                              "Update"
                            )}
                          </Button>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>

              {anyMissing && (
                <Button
                  variant="outline"
                  onClick={() => runDepAction("install", () => installDeps())}
                  disabled={depBusy !== null}
                  className="mt-3 w-full px-3 py-2 text-xs"
                >
                  {depBusy === "install" ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" /> Installing…
                    </>
                  ) : (
                    "Install missing"
                  )}
                </Button>
              )}

              {depError && (
                <p className="mt-2.5 text-xs text-[var(--color-bad)]">{depError}</p>
              )}
            </div>
          </div>
        ) : (
          <div className="px-6 py-12 text-center text-sm text-[var(--color-faint)]">Loading…</div>
        )}

        <div className="flex justify-end gap-2 border-t border-[var(--color-line)] px-6 py-4">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={save} disabled={!cfg || saving}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
        </div>
      </div>
    </div>
  );
}
