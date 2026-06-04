# Roadmap

**spriters** is a fast Rust reimplementation of [glue](http://glue.readthedocs.org/), a Python CSS sprite generator. This roadmap tracks the work needed to reach full feature parity with Glue, then exceed it.

---

## Status vs. Glue 0.11.1

| Feature | Glue | spriters |
|---|:---:|:---:|
| CSS / LESS / SCSS output | ✓ | ✓ |
| Custom Jinja2/template output | ✓ | ✓ |
| JSON output | ✓ | ✓ |
| JSON hash format (`--json-format=hash`) | ✓ | ✗ |
| cocos2d XML output | ✓ | ✓ |
| CAAT output | ✓ | ✗ |
| HTML test page | ✓ | ✓ |
| Layout algorithms (6) | ✓ | ✓ |
| Image ordering (5 modes) | ✓ | ✓ |
| Transparent crop | ✓ | ✓ |
| Padding & margin | ✓ | ✓ |
| PNG8 output | ✓ | ✓ |
| Retina / multi-ratio | ✓ | ✓ |
| Pseudo-class from filename | ✓ | ✓ |
| Cache busting (query param) | ✓ | ✓ |
| Cache busting (filename) | ✓ | ✓ |
| Cache busting (sprite-only) | ✓ | ✗ |
| Project mode | ✓ | ✓ |
| Recursive mode | ✓ | ✓ |
| Follow symlinks | ✓ | ✓ |
| Watch mode | ✓ | ✓ |
| Force regeneration | ✓ | ✓ |
| Config file support | ✓ | ✗ |
| Environment variable config | ✓ | ✗ |
| Duplicate detection | ✗ | ✓ |
| Image resize (max-width/height) | ✗ | partial |
| Multiple resize algorithms | ✗ | ✓ |
| Multi-threaded processing | ✗ | ✗ (TODO) |
| Wide image format support | limited | ✓ (via `image` crate) |

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

---

## Phase 4 — Ecosystem

### 4.1 Publish to crates.io

- Stabilize the public API surface
- Write crate-level documentation (`///` and module docs)
- Add integration tests covering at least one image per output format
- Publish `spriters` as both a library and a binary crate

### 4.2 Documentation site

Generate docs with `cargo doc` and publish via Codeberg Pages or GitHub Pages. Include a quickstart guide, CLI reference, and template variable reference equivalent to Glue's ReadTheDocs site.

### 4.3 Benchmarks

Add a `benches/` directory using `criterion`. Compare spriters vs Glue on:
- Small set (10 images)
- Medium set (100 images)
- Large set (500 images)
- Project mode (10 sprites × 50 images each)

### 4.4 Package manager distribution

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
