# Roadmap

**spriters** is a fast Rust reimplementation of [glue](http://glue.readthedocs.org/), a Python CSS sprite generator. This roadmap tracks the work needed to reach full feature parity with Glue, then exceed it.

---

## Status vs. Glue 0.11.1

| Feature                                 |  Glue   |       spriters        |
| --------------------------------------- | :-----: | :-------------------: |
| CSS / LESS / SCSS output                |    ✓    |           ✓           |
| Custom Jinja2/template output           |    ✓    |           ✓           |
| JSON output                             |    ✓    |           ✓           |
| JSON hash format (`--json-format=hash`) |    ✓    |           ✗           |
| cocos2d XML output                      |    ✓    |           ✓           |
| CAAT output                             |    ✓    |           ✗           |
| HTML test page                          |    ✓    |           ✓           |
| Layout algorithms (6)                   |    ✓    |           ✓           |
| Image ordering (5 modes)                |    ✓    |           ✓           |
| Transparent crop                        |    ✓    |           ✓           |
| Padding & margin                        |    ✓    |           ✓           |
| PNG8 output                             |    ✓    |           ✓           |
| Retina / multi-ratio                    |    ✓    |           ✓           |
| Pseudo-class from filename              |    ✓    |           ✓           |
| Cache busting (query param)             |    ✓    |           ✓           |
| Cache busting (filename)                |    ✓    |           ✓           |
| Cache busting (sprite-only)             |    ✓    |           ✗           |
| Project mode                            |    ✓    |           ✓           |
| Recursive mode                          |    ✓    |           ✓           |
| Follow symlinks                         |    ✓    |           ✓           |
| Watch mode                              |    ✓    |           ✓           |
| Force regeneration                      |    ✓    |           ✓           |
| Config file support                     |    ✓    |           ✗           |
| Environment variable config             |    ✓    |           ✗           |
| Duplicate detection                     |    ✗    |           ✓           |
| Image resize (max-width/height)         |    ✗    |           ✓           |
| Multiple resize algorithms              |    ✗    |           ✓           |
| Multi-threaded processing               |    ✗    |       ✗ (TODO)        |
| Wide image format support               | limited | ✓ (via `image` crate) |

---

## Phase 1 — Glue parity gaps

These are the remaining features present in Glue but missing or incomplete in spriters.

### 1.1 JSON hash format

Glue supports two JSON output structures via `--json-format`:

- `array` (default): `[{x, y, width, height, name}, ...]`
- `hash`: `{name: {x, y, width, height}, ...}`

Add `--json-format=array|hash` option to the CLI and implement the hash variant in `output.rs`.

### 1.2 CAAT output format

[CAAT](https://thothbot.github.io/parallax/docs/demo/basic/scene01.html) (Canvas Advanced Animation Toolkit) is a JavaScript animation framework that consumes sprite metadata. Add `--caat` flag and generate the corresponding JSON/JS descriptor file.

### 1.3 Cache busting — sprite file only

Add `--cachebuster-filename-only-sprites`: embed the hash in the sprite image filename but keep the CSS filename clean. Useful when the CSS is cached separately from the image assets.

### 1.4 Configuration files

Glue supports hierarchical config files (project → sprite → image level) and environment variables (`GLUE_*`). Add:

- A `spriters.toml` config file parsed from the source directory
- Merging priority: image-level > sprite-level > project-level > CLI args > env vars (`SPRITERS_*`) > defaults

---

## Phase 2 — Performance

### 2.1 Multi-threaded pre-processing

Crop and resize operations per image are independent. Run them on a Rayon thread pool before layout. Expected 4–8× speedup on large sprite sets.

### 2.2 Multi-threaded output generation

When generating multiple output formats (CSS + JSON + XML + HTML) or multiple sprites in project mode, dispatch each to a thread pool. Output formats are independent of each other once the sprite is built.

### 2.3 Multi-threaded sprite building (project mode)

In `--project` mode each subdirectory produces an independent sprite. Build all sprites concurrently.

---

## Phase 3 — Beyond Glue

These features go further than Glue's current scope.

### 3.1 Resize images

Complete the `--max-width` / `--max-height` implementation. Resize before crop, using the configured `--resize-algorithm` (nearest, triangle, catmullrom, gaussian, lanczos3). Ratio-aware: at ratio 2 the source is already 2× so scale accordingly.

### 3.2 WebP and AVIF output

Allow `--format=png|webp|avif` for the output sprite image. WebP reduces sprite size by 25–35% on average vs PNG32. AVIF reduces it further but has wider encoder latency.

### 3.3 SVG source support

Allow `.svg` files in the source directory. Rasterize them at the target DPI before packing. Requires an SVG renderer (e.g. `resvg`).

### 3.4 Duplicate consolidation

Currently spriters detects and warns about duplicate images. Close the loop: when `--no-duplicate` is not set, automatically de-duplicate by pointing multiple CSS class names at the same sprite region rather than including the image twice.

### 3.5 Reproducible output

Make output byte-for-byte deterministic across runs (fixed ordering, deterministic hash seeds) to support content-addressed caching in CI pipelines.

### 3.6 File exclusion patterns

Add `--exclude <glob>` (repeatable) to skip matching files when walking the source directory. Useful for ignoring work-in-progress assets, drafts, or OS metadata files.

```sh
spriters icons/ dist/ --exclude "_draft_*" --exclude "*.tmp"
```

### 3.7 stdin / piped file list

Accept a newline-separated list of image paths on stdin (when no `--input` directory is given) so spriters can be composed with other tools:

```sh
fd -e png -e svg . | spriters --output dist/
```

### 3.8 CSS custom properties output

Emit an additional CSS file where each sprite entry is expressed as custom properties rather than (or alongside) class selectors:

```css
:root {
  --sprite-icon-home-x: 0px;
  --sprite-icon-home-y: -48px;
  --sprite-icon-home-w: 24px;
  --sprite-icon-home-h: 24px;
}
```

This fits design-system / CSS token workflows and avoids coupling consumers to specific class names. Add a `--css-vars` flag and a `--css-vars-template` for custom output.

---

## Phase 4 — CI & Developer Experience

### 4.1 Dry-run mode

Add `--dry-run`: print what would be generated (detected images, sprite dimensions, output file paths, CSS class names) without writing any files. Useful for debugging config or validating a CI setup without side-effects.

### 4.2 Incremental builds — up-to-date detection

Skip regeneration entirely when the source images haven't changed since the last run. Compare content hashes (already computed for cache-busting) of all inputs against a small lockfile written to the output directory. `--force` bypasses the check. Enables fast re-runs in watch mode and incremental CI caches.

### 4.3 Check mode

Add `--check`: exit with a non-zero status code if the current output is missing or stale (source changed since last generation), without regenerating. Intended for CI steps that assert sprites are committed and up-to-date:

```sh
spriters icons/ dist/ --check || { echo "Sprites out of date — run spriters"; exit 1; }
```

### 4.4 Stats subcommand

Add a `spriters stats <output-dir>` subcommand that reads the last generated sprite and reports:

- Packing efficiency (used area / total sprite area, %)
- Number of source images, duplicates collapsed
- Input total bytes vs output sprite bytes (compression ratio)
- Per-format breakdown when multiple outputs were generated

### 4.5 Shell completions

Add a `spriters completions <shell>` subcommand that writes a completion script to stdout for `bash`, `zsh`, or `fish`. Integrates with standard shell setup:

```sh
spriters completions zsh > ~/.zfunc/_spriters
```

---

## Phase 5 — Modern Web Integration

### 5.1 Tailwind-compatible JSON output

Emit a JSON file (`--tailwind`) structured as a Tailwind CSS plugin config or `safelist` entry, listing all generated class names. This lets Tailwind's treeshaker keep sprite classes in production builds without manual safelisting.

### 5.2 TypeScript type definitions

Alongside JSON output, emit a `.d.ts` file (`--dts`) exporting a union type of all sprite names:

```ts
export type SpriteName =
  | "icon-home"
  | "icon-user"
  | "icon-settings";
```

Useful for typed UI frameworks (React, Vue, Svelte) to get autocomplete and compile-time checks on sprite name strings.

---

## Phase 6 — Ecosystem

### 6.1 Publish to crates.io

- Stabilize the public API surface
- Write crate-level documentation (`///` and module docs)
- Add integration tests covering at least one image per output format
- Publish `spriters` as both a library and a binary crate

### 6.2 Documentation site

Generate docs with `cargo doc` and publish via Codeberg Pages or GitHub Pages. Include a quickstart guide, CLI reference, and template variable reference equivalent to Glue's ReadTheDocs site.

### 6.3 Benchmarks

Add a `benches/` directory using `criterion`. Compare spriters vs Glue on:

- Small set (10 images)
- Medium set (100 images)
- Large set (500 images)
- Project mode (10 sprites × 50 images each)

### 6.4 Package manager distribution

Publish to:

- Homebrew tap (macOS/Linux)
- AUR (Arch Linux)
- Nix flake (already has `shell.nix`)
- Debian/RPM via CI artifacts

---

## Completed

- [x] Crop transparent borders
- [x] Warn about duplicate images
- [x] Fail on duplicate images (`--no-duplicate`)
- [x] Six layout algorithms
- [x] Five ordering modes with reverse prefix
- [x] CSS / LESS / SCSS / JSON / cocos2d / HTML output
- [x] Custom templates
- [x] Padding and margin (CSS shorthand syntax)
- [x] Retina / multi-ratio with CSS media queries
- [x] Pseudo-class from filename suffix
- [x] Cache busting (query param and filename)
- [x] Project mode and recursive mode
- [x] Watch mode with debounce
- [x] PNG8 output
- [x] Image resize (`--max-width` / `--max-height`) with aspect-ratio preservation and ratio awareness
- [x] File exclusion patterns (`--exclude`) with glob support (`*`, `?`), repeatable, TOML config and `SPRITERS_EXCLUDE` env var
- [x] stdin / piped file list — newline-delimited paths bypass directory walking; compose with `fd`, `find`, etc.
- [x] CSS custom properties output (`--css-vars`) — emits `<sprite>.vars.css` with `--name-x/y/w/h` tokens in `:root {}`
- [x] `--dry-run` — discovers images and prints what would be generated without writing any files
- [x] Incremental builds / up-to-date detection — content-hash lockfile (`.{name}.spriters`) skips regeneration when inputs are unchanged; `--force` bypasses
- [x] `--check` — exits non-zero if output is missing or stale; used in CI via `Manager::is_up_to_date()`
- [x] `stats` subcommand — reads generated sprite image, reports dimensions, file size, and packing efficiency
- [x] `completions` subcommand — writes bash/zsh/fish completion scripts to stdout
