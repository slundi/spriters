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
| `-V`, `--version`   | Print the version                                                   |
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
| `-p`, `--padding`    | Transparent space around each image, part of its CSS box (`width`/`height`/`background-position`)       |
| `-m`, `--margin`     | Transparent space between images, outside their CSS box                                                 |
| `--ratios`           | Comma-separated ratios, e.g. `2,1` or `2,1.5,1`. Source images are at the highest ratio; one sprite is written per ratio (`name.png`, `name@2x.png`, …) and the CSS uses 1× px with `background-size` and media queries. Padding and margin are in CSS px |
| `--retina`           | Shortcut for `--ratios=2,1`                                                                             |
| `--png8`             | 8-bit palette PNG (up to 256 colours, transparency kept) instead of 32-bit RGBA                         |

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
| `--css`                               | Directory of the CSS and every other text output (default: `--output`); the image URL is relative to it |
| `--img`                               | Directory of the sprite images and of the `.{name}.spriters` metadata (default: `--output`)   |
| `--css-template`                      | Custom Jinja template for the CSS output (see [Templates](#templates))                        |
| `--less-template`                     | Custom Jinja template for the LESS output                                                      |
| `--scss-template`                     | Custom Jinja template for the SCSS output                                                      |
| `--css-vars`                          | Emit CSS custom properties (`<sprite>.vars.css`) with `--name-x/y/w/h` tokens                  |
| `--css-vars-template`                 | Custom Jinja template for the CSS custom properties output                                     |
| `--tailwind`                          | Emit `<sprite>.tailwind.json` safelist for Tailwind CSS purge/safelist config                  |
| `--dts`                               | Emit `<sprite>.d.ts` TypeScript union type of all sprite names                                 |
| `--cachebuster`                       | Add content hash as query param to sprite URL                                                  |
| `--cachebuster-filename`              | Embed content hash in sprite URL filename                                                      |
| `--cachebuster-filename-only-sprites` | Embed hash in sprite image filename; CSS URL stays clean                                       |
| `--namespace`                         | Global CSS namespace (default: `sprite`)                                                       |
| `--sprite-namespace`                  | Per-sprite CSS namespace; use `%(sprite)s` for the sprite name                                 |
| `--separator`                         | CSS class name separator (default: `-`); `camelcase` gives `.spriteIconsMyImage`               |
| `--pseudo-class-separator`            | Suffix separator for pseudo-classes, off by default (e.g. `__` → `button__hover.png` → `.sprite-icons-button:hover`); recognised: `link`, `visited`, `active`, `hover`, `focus`, `first-letter`, `first-line`, `first-child`, `before`, `after` |
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

Subcommands are only recognised as the first argument. To build a sprite from a directory
named like a subcommand, write it as a path or after `--`:

```sh
spriters ./stats dist/
spriters -- stats dist/
```

---

## Templates

`--css-template`, `--less-template`, `--scss-template` and `--css-vars-template` replace the
built-in output with a [Jinja](https://jinja.palletsprojects.com/) template, rendered with
[MiniJinja](https://docs.rs/minijinja). Nothing is HTML-escaped. A missing template or a
syntax error is reported before anything is generated, and editing a template regenerates
its output on the next run.

| Variable          | Content                                                                         |
| ----------------- | ------------------------------------------------------------------------------- |
| `name`            | Sprite name                                                                     |
| `sprite_path`     | URL of the sprite image, as in the built-in CSS (`--url`, cache buster…)        |
| `sprite_filename` | File name of the sprite image                                                   |
| `width`, `height` | Sprite size in CSS px                                                           |
| `hash`            | Cache buster of the sprite (8 hex digits)                                       |
| `images`          | One entry per image and per duplicate (alias), see below                        |
| `ratios`          | With `--ratios`, one entry per ratio above the smallest: `ratio` (`2`), `fraction` (`2`, `3/2`), `sprite_path`, `sprite_filename` |

Each entry of `images`:

| Variable          | Content                                                                         |
| ----------------- | ------------------------------------------------------------------------------- |
| `label`           | CSS class name (`sprite-icons-arrow`), without pseudo-class                     |
| `pseudo`          | Pseudo-classes from `--pseudo-class-separator` (`:hover`), usually empty        |
| `selector`        | `.label` followed by `pseudo`                                                   |
| `name`            | Image name without namespaces (`arrow`), as in the JSON output                  |
| `filename`, `path` | Source file name and path                                                      |
| `x`, `y`          | Position in the sprite in CSS px (positive: use `-{{ image.x }}px`)             |
| `width`, `height` | Size in CSS px, padding included                                                |
| `last`            | Whether it is the last entry (`loop.last` works too)                            |

```jinja
{% for image in images %}{{ image.selector }}{% if not loop.last %}, {% endif %}{% endfor %} {
  background-image: url('{{ sprite_path }}');
}
{% for image in images %}
{{ image.selector }} { background-position: -{{ image.x }}px -{{ image.y }}px; width: {{ image.width }}px; height: {{ image.height }}px; }
{%- endfor %}
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

Most CLI options are supported; directories (`--output`, `--css`, `--img`) and templates are given on the command line. The file holds project defaults: `SPRITERS_*` environment variables override it, and CLI flags override both.

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
