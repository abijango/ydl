# ydl

A Rust CLI for downloading YouTube videos, playlists, channels, or arbitrary URL lists. Built on top of `yt-dlp` for extraction and `ffmpeg` for muxing/conversion, with thick `indicatif` progress bars (percent, bytes, speed, ETA), a TOML config file for defaults, configurable parallelism, and incremental skipping of already-downloaded videos.

`ydl` manages its own copies of `yt-dlp` and `ffmpeg` — no separate installation step required.

---

## Table of contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
  - [Single video](#single-video)
  - [`playlist`](#playlist)
  - [`channel`](#channel)
  - [`batch`](#batch)
  - [`config`](#config)
  - [`deps`](#deps)
- [Common flags](#common-flags)
- [Configuration file](#configuration-file)
- [Filename templates](#filename-templates)
- [Incremental sync (resume / skip)](#incremental-sync-resume--skip)
- [How dependencies are resolved](#how-dependencies-are-resolved)
- [Troubleshooting](#troubleshooting)
- [Building from source](#building-from-source)

---

## Installation

### Option A — install from source (recommended)

You need a recent Rust toolchain (1.74+). Install [rustup](https://rustup.rs/) if you don't already have it.

```bash
git clone <this repo url> ydl
cd ydl
cargo install --path .
```

`cargo install` puts the `ydl` binary in `~/.cargo/bin/` (Windows: `%USERPROFILE%\.cargo\bin\`), which is on your `PATH` if you installed Rust via rustup.

### Option B — build a release binary and copy it manually

```bash
cargo build --release
# Windows
copy target\release\ydl.exe %USERPROFILE%\bin\ydl.exe
# macOS / Linux
install -m 0755 target/release/ydl ~/.local/bin/ydl
```

### One-time: install yt-dlp + ffmpeg

`ydl` will auto-install both binaries the first time you run a download, into:

| OS | Path |
|---|---|
| Windows | `%LOCALAPPDATA%\ydl\bin\` |
| macOS | `~/Library/Application Support/ydl/bin/` |
| Linux | `~/.local/share/ydl/bin/` |

You can also do it explicitly up front:

```bash
ydl deps install
```

If `yt-dlp` and/or `ffmpeg` are already on your `PATH` (e.g. installed via Chocolatey, Homebrew, apt), `ydl` will use those instead — no download needed.

> macOS note: there is no automated ffmpeg download for macOS. Install it with `brew install ffmpeg` (or set `[ffmpeg].binary` in the config to a custom path). `yt-dlp` does auto-install on macOS.

---

## Quick start

```bash
# Single video — just pass the URL
ydl https://www.youtube.com/watch?v=dQw4w9WgXcQ

# Whole playlist, 3 parallel workers, into ./music
ydl playlist https://www.youtube.com/playlist?list=PLxxx -j 3 -o ./music

# Whole channel, audio-only (m4a)
ydl channel https://www.youtube.com/@SomeChannel --audio-only

# A list of URLs in a file
ydl batch urls.txt
```

Re-running any of the commands above is safe: previously-finished videos are recorded in a `.ydl-archive` file inside the output directory and skipped on subsequent runs.

---

## Usage

```
ydl [OPTIONS] [URL] [COMMAND]
```

If you pass a URL with no subcommand, `ydl` treats it as a single video. All subcommands accept the same set of [common flags](#common-flags); CLI flags override values from the config file.

### Single video

```bash
ydl <URL> [flags]
```

Downloads exactly one video. This is the default — there is no separate `video` subcommand.

### `playlist`

```bash
ydl playlist <URL> [flags]
```

Downloads every video in the playlist. `ydl` first asks `yt-dlp --flat-playlist` for the list of video IDs so it can show an accurate `[done/total]` overall progress bar, then dispatches each one through the normal download path.

### `channel`

```bash
ydl channel <URL_OR_HANDLE> [flags]
```

Downloads every video on a channel. Accepts:

- Full channel URLs: `https://www.youtube.com/@MrBeast/videos`
- Handle URLs: `https://www.youtube.com/@MrBeast`
- `@handle` shorthand: `@MrBeast` (yt-dlp resolves this)
- Channel ID URLs: `https://www.youtube.com/channel/UCxxx`

Combine with `--archive` and parallelism for incremental channel sync (see [below](#incremental-sync-resume--skip)).

### `batch`

```bash
ydl batch <FILE> [flags]
```

Reads URLs one-per-line from `<FILE>`. `#` comments and blank lines are ignored. Each line can be any URL `yt-dlp` understands — single videos, playlists, or channels — and they are mixed and matched freely:

```text
# Big channels first
https://www.youtube.com/@SomeChannel/videos

# A specific playlist
https://www.youtube.com/playlist?list=PLxxx

# A handful of standalone videos
https://www.youtube.com/watch?v=aaa
https://www.youtube.com/watch?v=bbb
```

### `config`

```bash
ydl config show     # print the merged config
ydl config path     # print the config file path
ydl config init     # (re)generate the config file with defaults
ydl config edit     # open it in $EDITOR (notepad on Windows by default)
```

### `deps`

```bash
ydl deps status     # show resolved paths + versions of yt-dlp and ffmpeg
ydl deps install    # install any missing managed binary
ydl deps update     # re-download both managed binaries from upstream
```

The global `--update` flag on a download command does the same as `deps update` and then continues with the download:

```bash
ydl playlist <URL> --update
```

---

## Common flags

These apply to the bare-URL form, `playlist`, `channel`, and `batch`.

| Flag | What it does |
|---|---|
| `-o, --output-dir <DIR>` | Where to put the files. Default `.` (current directory). |
| `-j, --jobs <N>` | Parallel download workers. Default 3. |
| `-q, --quality <SELECTOR>` | yt-dlp format selector, e.g. `bv*+ba/b`, `bestvideo[height<=720]+bestaudio`, `worst`. |
| `--merge-format <EXT>` | Container for the merged output: `mp4` (default), `mkv`, `webm`. |
| `--audio-only` | Extract audio only, output `.m4a`. |
| `--filename-template <TPL>` | Override the filename template. See [below](#filename-templates). |
| `--archive <FILE>` | Custom path for the download-archive file. |
| `--no-archive` | Disable the download archive entirely (always download every URL). |
| `--dry-run` | Print the URLs that would be downloaded without actually fetching anything. |
| `--update` | Update `yt-dlp` and `ffmpeg` before running this command. |
| `-y, --yes` | Auto-accept any interactive prompt (e.g. first-run install). |
| `--no-autoinstall` | Refuse to auto-install missing binaries; error out instead. |
| `-v, --verbose` | More logging. Repeat for more (`-v` info, `-vv` debug, `-vvv` trace). |

---

## Configuration file

`ydl` reads defaults from a TOML file located at:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\ydl\config\config.toml` |
| macOS | `~/Library/Application Support/ydl/config.toml` |
| Linux | `~/.config/ydl/config.toml` |

Run `ydl config path` to print the exact location. The file is created with defaults the first time `ydl` runs, and you can regenerate it any time with `ydl config init`.

Default contents:

```toml
[defaults]
output_dir         = "."
filename_template  = "{upload_date}-{title}.{ext}"
quality            = "bv*+ba/b"      # yt-dlp -f selector
merge_format       = "mp4"           # passed to --merge-output-format
audio_only         = false

[parallel]
jobs               = 3

[archive]
enabled            = true
# Relative paths resolve under output_dir; absolute paths are used as-is.
path               = ".ydl-archive"

[ytdlp]
binary             = ""              # "" = use managed/PATH; absolute path overrides
auto_install       = true
auto_update        = false
extra_args         = []              # appended to every yt-dlp invocation

[ffmpeg]
binary             = ""              # "" = use managed/PATH; absolute path overrides
```

CLI flags are applied on top of whatever is in the config — so you can set sensible defaults once and override per-command.

### `extra_args` examples

```toml
[ytdlp]
extra_args = [
    "--cookies-from-browser", "firefox",   # use Firefox cookies for age-restricted videos
    "--write-subs", "--sub-langs", "en",   # also download English subtitles
    "--embed-thumbnail",                   # embed cover art into m4a
    "--limit-rate", "2M",                  # cap bandwidth at 2 MB/s
]
```

---

## Filename templates

The default template is:

```
{upload_date}-{title}.{ext}
```

which produces files like `20240101-Some Video Title.mp4`. The `{name}` placeholders are translated to yt-dlp's native `%(name)s` syntax under the hood, so any field listed in [yt-dlp's output template docs](https://github.com/yt-dlp/yt-dlp#output-template) works:

| Placeholder | Meaning |
|---|---|
| `{title}` | Video title |
| `{ext}` | File extension (chosen by yt-dlp / merge format) |
| `{upload_date}` | Upload date as `YYYYMMDD` |
| `{uploader}` | Display name of the uploader |
| `{channel}` | Channel name |
| `{id}` | YouTube video ID (always unique) |
| `{playlist_index}` | Position in the playlist (only meaningful for `playlist`) |
| `{duration_string}` | e.g. `1:23` |
| `{resolution}` | e.g. `1920x1080` |

Examples:

```bash
# Per-channel folders
ydl channel @SomeChannel --filename-template "{channel}/{upload_date}-{title}.{ext}"

# Numbered tracks for a music playlist
ydl playlist <URL> --filename-template "{playlist_index:03d}-{title}.{ext}"

# Use yt-dlp syntax directly if you prefer
ydl <URL> --filename-template "%(uploader)s - %(title)s.%(ext)s"
```

---

## Incremental sync (resume / skip)

Every successful download is appended to a `.ydl-archive` file (in the output directory by default). On subsequent runs, yt-dlp is told to consult this file via `--download-archive`, and any video listed there is silently skipped. This makes channel sync cheap:

```bash
# First run: downloads everything
ydl channel @SomeChannel -o ./archive -j 4

# A week later: only new videos are fetched
ydl channel @SomeChannel -o ./archive -j 4
```

Tips:

- **Per-channel archives** — give each channel its own output directory (or set `--archive ./<name>.archive`) to avoid one giant archive file across unrelated downloads.
- **Disable temporarily** — `--no-archive` skips the archive entirely if you really want to re-download something.
- **Reset for one video** — manually delete the matching `youtube <id>` line from `.ydl-archive` and rerun.

---

## How dependencies are resolved

For each invocation, `ydl` resolves the `yt-dlp` and `ffmpeg` paths in this order:

1. **Explicit absolute path** in `[ytdlp].binary` / `[ffmpeg].binary` in your config.
2. **Managed copy** under `<data_dir>/bin/` (installed via `ydl deps install`).
3. **System binary on `PATH`** (e.g. installed via Chocolatey, Homebrew, apt, winget).
4. **Auto-install** the managed copy and retry — unless `auto_install = false` or `--no-autoinstall` is set.

Versions of managed binaries are tracked in `<data_dir>/bin/versions.json` and shown by `ydl deps status`.

---

## Troubleshooting

**`yt-dlp not found ... auto-install is disabled`**
You set `[ytdlp].auto_install = false` (or passed `--no-autoinstall`) and there's no binary on PATH or in the managed dir. Either run `ydl deps install`, install yt-dlp through your package manager, or set `[ytdlp].auto_install = true` and re-run.

**`ffmpeg auto-install: no auto-install available for macOS`**
On macOS, install ffmpeg with `brew install ffmpeg`. yt-dlp does auto-install on macOS, but the BtbN ffmpeg builds do not ship for macOS, so you bring your own.

**The progress bar looks broken / shows escape sequences**
Use a modern terminal (Windows Terminal, iTerm2, GNOME Terminal). Older `cmd.exe` on Windows 7/8 may not render `█`/`▓` block characters correctly. The bar is drawn on stderr; redirecting stderr to a file produces noise but doesn't break the download itself.

**A specific video errors with HTTP 403 or "Sign in to confirm your age"**
The video requires authentication. Set yt-dlp's cookie support in your config:

```toml
[ytdlp]
extra_args = ["--cookies-from-browser", "firefox"]
```

(Replace `firefox` with `chrome`, `edge`, `brave`, etc.)

**Downloads are slow / `yt-dlp` is throttling**
Try `--update` to pick up the latest yt-dlp release — extractor fixes ship frequently. You can also pass `--limit-rate` via `extra_args` if YouTube is rate-limiting you in the other direction.

**Archive isn't skipping**
Check that `[archive].enabled = true` in the config and that the `.ydl-archive` file exists in the output directory you're using. The archive path is relative to the output dir by default; if you change `-o` between runs, you'll have a different archive file.

---

## Building from source

```bash
# Clone and build
git clone <this repo url> ydl
cd ydl
cargo build --release

# Run unit tests
cargo test

# Strip + LTO are already enabled in the release profile; the resulting
# `target/release/ydl(.exe)` is ~6 MB on Windows.
```

The crate has no `build.rs` and no native dependencies on Windows beyond what `rustls` and `xz2` need, both of which build cleanly from source.
