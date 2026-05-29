// Curated, human-friendly release notes shown in the in-app "What's new" dialog.
// Keep these fun and non-technical — the GitHub releases hold the nerdy details.

export interface ReleaseNote {
  version: string;
  date: string;
  headline: string;
  notes: string[];
}

export const RELEASE_NOTES: ReleaseNote[] = [
  {
    version: "2026.5.7",
    date: "May 2026",
    headline: "The glow-up ✨",
    notes: [
      "🎬 A real desktop app — paste a link and watch it come down. No terminal required.",
      "🪶 Fresh light look with a hand-lettered “y”.",
      "⚡ Live progress with real speed and ETA, so you always know what’s left.",
      "🎚️ Pick quality in plain English — “1080p”, not cryptic codes.",
      "🗂️ One click to reveal a finished video in Finder.",
      "🕘 A history of everything you’ve grabbed — browse, reveal, or clear it.",
      "🧠 Smarter “you already have this” detection, so repeats don’t look broken.",
      "🛠️ yt-dlp & ffmpeg keep themselves up to date from Settings.",
    ],
  },
  {
    version: "2026.5.x",
    date: "May 2026",
    headline: "First light 🌱",
    notes: [
      "📦 The very first builds — auto-published for macOS and Windows.",
      "▶️ Download single videos, whole playlists, or entire channels.",
    ],
  },
];
