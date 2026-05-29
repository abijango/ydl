// Human-readable quality presets ↔ yt-dlp `-f` format selectors.
// The UI shows the label; the config stores the technical `value`.

export interface QualityPreset {
  label: string;
  value: string;
}

// `bv*` = best video (any container), `ba` = best audio, `/b` = fall back to a
// pre-muxed best. The height caps keep the resolution at-or-below the target.
export const QUALITY_PRESETS: QualityPreset[] = [
  { label: "Best available", value: "bv*+ba/b" },
  { label: "4K · 2160p", value: "bv*[height<=2160]+ba/b[height<=2160]" },
  { label: "1440p", value: "bv*[height<=1440]+ba/b[height<=1440]" },
  { label: "1080p · Full HD", value: "bv*[height<=1080]+ba/b[height<=1080]" },
  { label: "720p · HD", value: "bv*[height<=720]+ba/b[height<=720]" },
  { label: "480p", value: "bv*[height<=480]+ba/b[height<=480]" },
  { label: "360p", value: "bv*[height<=360]+ba/b[height<=360]" },
  { label: "Smallest file", value: "wv*+wa/w" },
];

/** Sentinel <option> value for the "Custom…" choice. */
export const CUSTOM_QUALITY = "__custom__";

/** True when `value` isn't one of the presets (so the raw editor should show). */
export function isCustomQuality(value: string): boolean {
  return !QUALITY_PRESETS.some((p) => p.value === value);
}
