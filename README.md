# spriters

Fast CSS sprite generator written in Rust — a high-performance reimplementation of [Glue](http://glue.readthedocs.org/) with modern additions: SVG source support, WebP/AVIF output, multi-threaded processing, and reproducible builds.

---

## Why sprites?

Packing many small images into a single file reduces HTTP requests and improves loading performance for web pages and games. spriters automates this: it finds your source images, packs them optimally, and emits the sprite image plus the CSS (or LESS, SCSS, JSON, XML…) that lets you display each image by name.

---

## Features

| Category               | Feature                                                                                                                                            |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Output formats**     | CSS, LESS, SCSS, JSON (array & hash), CAAT, cocos2d XML, HTML test page, custom Jinja-style templates                                              |
| **Image formats**      | PNG (default), WebP (lossless), AVIF — controlled with `--format`                                                                                  |
| **Source formats**     | All formats supported by the [`image`](https://crates.io/crates/image) crate, plus SVG (rasterised with [`resvg`](https://crates.io/crates/resvg)) |
| **Layout algorithms**  | square (default), vertical, horizontal, diagonal, vertical-right, horizontal-bottom                                                                |
| **Image ordering**     | maxside, width, height, area, filename — each with optional `-` prefix for reverse                                                                 |
| **Retina / multi-DPI** | `--ratios=2,1` scales everything and generates media-query CSS; SVGs rasterise at the correct resolution                                           |
| **Resize**             | `--max-width` / `--max-height` with nearest, triangle, CatmullRom, Gaussian, or Lanczos3 filter                                                    |
| **Crop**               | Remove transparent or white borders before packing                                                                                                 |
| **Padding / margin**   | CSS shorthand syntax (`10`, `5 10`, `1 2 3 4`)                                                                                                     |
| **Cache busting**      | Query param (`?hash`), filename (`_hash`), or image-file-only — all deterministic (content-addressed)                                              |
| **Deduplication**      | Identical images are consolidated: one sprite region, multiple CSS class names                                                                     |
| **Config files**       | `spriters.toml` in the source directory; `SPRITERS_*` environment variables                                                                        |
| **Project mode**       | `--project` generates one sprite per sub-directory                                                                                                 |
| **Reproducibility**    | Byte-for-byte identical output for the same inputs, suitable for content-addressed CI caches                                                       |
| **Performance**        | Rayon-parallel decode, crop, resize; parallel output generation; parallel project-mode builds                                                      |
| **File exclusion**     | `--exclude <glob>` (repeatable) skips matching files; supports `*` and `?`; configurable via `SPRITERS_EXCLUDE`                                    |
| **Stdin input**        | `--stdin` (or `-i -`) reads newline-separated image paths from stdin instead of walking a directory — compose with `fd`, `find`, etc.              |
| **CSS custom props**   | `--css-vars` emits `<sprite>.vars.css` with `--name-x/y/w/h` tokens in `:root {}` for design-system / CSS token workflows                          |
| **Dry-run**            | `--dry-run` prints detected images, sprite dimensions, and output paths without writing any files                                                  |
| **Incremental builds** | Content-hash lockfile (`.{name}.spriters`) skips regeneration when inputs are unchanged; `--force` bypasses                                        |
| **Check mode**         | `--check` exits non-zero when output is missing or stale — assert sprites are committed in CI without regenerating                                 |
| **Stats**              | `stats <output-dir>` subcommand reports dimensions, file size, packing efficiency, and duplicate count                                             |
| **Shell completions**  | `completions <shell>` subcommand writes bash/zsh/fish completion scripts to stdout                                                                 |
| **Tailwind safelist**  | `--tailwind` emits `<sprite>.tailwind.json` listing all CSS class names for Tailwind's purge/safelist config                                       |
| **TypeScript types**   | `--dts` emits `<sprite>.d.ts` with `export type SpriteName = "…"                                                                                   |

---

## Installation

Requires [Rust](https://www.rust-lang.org/tools/install) 1.75+.

```sh
cargo install spriters
```

Or build from source:

```sh
git clone https://codeberg.org/slundi/spriters
cd spriters
cargo build --release
# binary at target/release/spriters
```

---

## Quick start

```sh
# Generate sprite + CSS from all images in ./icons/, writing output to ./dist/
spriters icons/ dist/

# Retina sprite (2× and 1× CSS with media queries)
spriters icons/ dist/ --retina

# WebP output
spriters icons/ dist/ --format=webp

# SVG sources (rasterised at natural size)
spriters svg-icons/ dist/

# Project mode: one sprite per sub-folder
spriters images/ dist/ --project

# Exclude draft/temp files
spriters icons/ dist/ --exclude "_draft_*" --exclude "*.tmp"

# Compose with fd — pipe file list on stdin (-i still names the sprite)
fd -e png -e svg . icons/ | spriters --stdin -i icons/ --output dist/

# Emit CSS custom properties, Tailwind safelist, and TypeScript types
spriters icons/ dist/ --css-vars --tailwind --dts

# Dry-run: see what would be generated without writing files
spriters icons/ dist/ --dry-run

# CI check — fail if sprites are out of date
spriters icons/ dist/ --check
```

---

## CLI reference

### General

| Flag                | Description                                                         |
| ------------------- | ------------------------------------------------------------------- |
| `-i`, `--input`     | Source directory (default: CWD)                                     |
| `-o`, `--output`    | Output directory (default: CWD)                                     |
| `--project`         | Generate one sprite per sub-directory                               |
| `-r`, `--recursive` | Walk source directory recursively                                   |
| `--follow-links`    | Follow symbolic links                                               |
| `--force`           | Regenerate even if sprite is up to date                             |
| `--watch`           | Watch for changes and regenerate                                    |
| `-q`, `--quiet`     | Suppress console output                                             |
| `--exclude`         | Skip files matching glob pattern (repeatable, supports `*` and `?`) |
| `--dry-run`         | Print what would be generated without writing any files             |
| `--check`           | Exit non-zero if output is missing or stale (CI assertion)          |

### Image processing

| Flag                 | Description                                                                                             |
| -------------------- | ------------------------------------------------------------------------------------------------------- |
| `-a`, `--algorithm`  | Layout: `square` (default), `vertical`, `horizontal`, `diagonal`, `vertical-right`, `horizontal-bottom` |
| `--ordering`         | Sort order: `maxside` (default), `width`, `height`, `area`, `filename` — prefix with `-` to reverse     |
| `-c`, `--crop`       | Crop transparent/white borders                                                                          |
| `--max-width`        | Maximum image width (CSS px, scales with ratio)                                                         |
| `--max-height`       | Maximum image height (CSS px, scales with ratio)                                                        |
| `--resize-algorithm` | `nearest`, `triangle`, `catmullrom` (default), `gaussian`, `lanczos3`                                   |
| `-p`, `--padding`    | Padding inside each cell                                                                                |
| `-m`, `--margin`     | Margin outside each cell                                                                                |
| `--ratios`           | Comma-separated ratios, e.g. `2,1` or `2,1.5,1`                                                         |
| `--retina`           | Shortcut for `--ratios=2,1`                                                                             |
| `--png8`             | Grayscale PNG output                                                                                    |

### Output

| Flag                                  | Description                                                                                    |
| ------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `--format`                            | Sprite image format: `png` (default), `webp`, `avif`                                           |
| `--json`                              | Generate JSON metadata                                                                         |
| `--json-format`                       | JSON structure: `array` (default) or `hash`                                                    |
| `--caat`                              | Generate CAAT JSON descriptor                                                                  |
| `--cocos2d`                           | Generate cocos2d XML plist                                                                     |
| `--html`                              | Generate HTML test page                                                                        |
| `-l`, `--less`                        | Generate LESS stylesheet                                                                       |
| `--scss`                              | Generate SCSS stylesheet                                                                       |
| `--no-css`                            | Skip CSS generation                                                                            |
| `--no-img`                            | Skip sprite image generation                                                                   |
| `--css`                               | Custom directory for CSS files                                                                 |
| `--img`                               | Custom directory for sprite image                                                              |
| `--css-template`                      | Custom Jinja-style CSS template                                                                |
| `--less-template`                     | Custom Jinja-style LESS template                                                               |
| `--scss-template`                     | Custom Jinja-style SCSS template                                                               |
| `--css-vars`                          | Emit CSS custom properties (`<sprite>.vars.css`) with `--name-x/y/w/h` tokens                  |
| `--css-vars-template`                 | Custom template for CSS custom properties output                                               |
| `--tailwind`                          | Emit `<sprite>.tailwind.json` safelist for Tailwind CSS purge/safelist config                  |
| `--dts`                               | Emit `<sprite>.d.ts` TypeScript union type of all sprite names                                 |
| `--cachebuster`                       | Add content hash as query param to sprite URL                                                  |
| `--cachebuster-filename`              | Embed content hash in sprite URL filename                                                      |
| `--cachebuster-filename-only-sprites` | Embed hash in sprite image filename; CSS URL stays clean                                       |
| `--namespace`                         | Global CSS namespace (default: `sprite`)                                                       |
| `--sprite-namespace`                  | Per-sprite CSS namespace; use `%(sprite)s` for the sprite name                                 |
| `--separator`                         | CSS class name separator (default: `-`)                                                        |
| `--pseudo-class-separator`            | Suffix separator for pseudo-classes (e.g. `__` → `button__hover.png` → `.sprite-button:hover`) |
| `-u`, `--url`                         | Base URL for sprite images (e.g. `https://cdn.example.com/sprites`)                            |

### Subcommands

| Subcommand            | Description                                                             |
| --------------------- | ----------------------------------------------------------------------- |
| `stats <output-dir>`  | Report sprite dimensions, file size, packing efficiency, and duplicates |
| `completions <shell>` | Write completion script to stdout (`bash`, `zsh`, or `fish`)            |

```sh
# Install zsh completions
spriters completions zsh > ~/.zfunc/_spriters
```

---

## Configuration file

Create `spriters.toml` in your source directory:

```toml
crop = true
json = true
json_format = "hash"
format = "webp"
namespace = "icon"
separator = "_"
ratios = "2,1"
max_width = 200
```

All CLI options are supported. The file holds project defaults: `SPRITERS_*` environment variables override it, and CLI flags override both.

Priority order (highest → lowest): CLI flags > `SPRITERS_*` env vars > `spriters.toml` > defaults.

`exclude` patterns are the exception: those from all three sources are combined.

---

## Library usage

```toml
[dependencies]
spriters = "0.1"
```

```rust
use spriters::{Config, Manager};
use std::path::PathBuf;

let mut config = Config::default();
config.source = PathBuf::from("icons/");
config.output = PathBuf::from("dist/");
config.json   = true;

let mut manager = Manager { project: false, config, ..Default::default() };
manager.find_sprites();
manager.save();
```

---

## Compared to Glue

spriters targets full feature parity with [Glue 0.11.1](http://glue.readthedocs.org/) plus several additions:

| Feature                               |  Glue   | spriters |
| ------------------------------------- | :-----: | :------: |
| CSS / LESS / SCSS / custom templates  |    ✓    |    ✓     |
| JSON (array & hash), cocos2d, CAAT    |    ✓    |    ✓     |
| HTML test page                        |    ✓    |    ✓     |
| 6 layout algorithms, 5 orderings      |    ✓    |    ✓     |
| Transparent crop, padding, margin     |    ✓    |    ✓     |
| PNG8, retina / multi-ratio            |    ✓    |    ✓     |
| Pseudo-class from filename            |    ✓    |    ✓     |
| Cache busting (3 modes)               |    ✓    |    ✓     |
| Project mode, recursive, watch        |    ✓    |    ✓     |
| Config file + env vars                |    ✓    |    ✓     |
| Duplicate detection & consolidation   |    —    |    ✓     |
| SVG source support                    |    —    |    ✓     |
| WebP / AVIF output                    |    —    |    ✓     |
| Image resize (max-width/height)       | partial |    ✓     |
| Multi-threaded processing             |    —    |    ✓     |
| Reproducible output                   |    —    |    ✓     |
| File exclusion patterns (`--exclude`) |    —    |    ✓     |
| stdin / piped file list               |    —    |    ✓     |
| CSS custom properties (`--css-vars`)  |    —    |    ✓     |
| Dry-run mode (`--dry-run`)            |    —    |    ✓     |
| Incremental builds / check mode       |    —    |    ✓     |
| Tailwind safelist JSON (`--tailwind`) |    —    |    ✓     |
| TypeScript type definitions (`--dts`) |    —    |    ✓     |
| Stats subcommand                      |    —    |    ✓     |
| Shell completions subcommand          |    —    |    ✓     |
