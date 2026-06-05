# Agent Context: spriters

## Project identity

**spriters** is a Rust CLI + library for generating CSS sprite sheets. It is a fast reimplementation of the Python [Glue](http://glue.readthedocs.org/) tool.

- **Repository**: https://codeberg.org/slundi/spriters
- **Crate type**: both library (`spriters`) and binary (`spriters`)
- **Rust edition**: 2021
- **Roadmap**: `Roadmap.md` — consult before starting any new feature

---

## Source layout

```
src/
├── lib.rs          Core types (Config, Sprite, ImageAndPosition, Manager) + pipeline
├── main.rs         CLI entry point: arg parsing, watch loop
├── cli.rs          lexopt-based argument parser → Config
├── config.rs       spriters.toml + SPRITERS_* env-var support
├── output.rs       All file generators (CSS, LESS, SCSS, JSON, CAAT, cocos2d, HTML, sprite image)
├── algorithm.rs    Layout algorithms: vertical, horizontal, diagonal, vertical_right, horizontal_bottom
└── algorithm_square.rs  Square (BinPack) algorithm

tests/
├── misc.rs         Ordering enum tests (public API)
├── output.rs       Integration tests — generate sprites from tests/images/ into temp dirs

tests/images/
├── america/        us.png (250×132), ca.png (100×50)
├── europe/         de.png, fi.png, fr.png (250×167), no.png (120×100 with transparent border), se.png (100×63)
├── test.svg        64×48 SVG fixture (red rect + blue circle)
└── identical-fr.png  Duplicate of europe/fr.png (for dedup tests)
```

---

## Key types

### `Config` (lib.rs)

Holds every option. `Config::default()` gives sensible defaults (Square algorithm, maxside ordering, PNG format, `-` separator, `sprite` namespace). All fields are `pub`.

Notable fields for Phase 3+ work: `sprite_format: SpriteFormat`, `json_format: JsonFormat`, `caat: bool`, `cachebuster_filename_only_sprites: bool`.

### `Sprite` (lib.rs)

Built by `Sprite::new(config, path)`. `path` is the output name stem (e.g. `images/america`). Internally calls `discover_images()` then `process()` (layout algorithm).

### `ImageAndPosition` (lib.rs)

One image in a sprite, with computed `x`, `y`, `computed_width`, `computed_height`. Also carries `aliases: Vec<String>` — paths of identical images that share this region (consolidation, Phase 3.4).

### `Manager` (lib.rs)

`find_sprites()` populates `self.sprites`; `save()` calls `output::generate_files` for each. Both are parallelised with Rayon.

---

## Processing pipeline

```
Manager::find_sprites()
  └─ Sprite::new(config, path)
       └─ discover_images()
            Phase 1 (sequential): WalkDir → hash dedup → collect (path, bytes)
            Phase 2 (parallel, Rayon): decode/rasterize → resize → crop → ImageAndPosition
       └─ sort_images()         total order via metric + path tiebreaker
       └─ process()             layout algorithm sets x, y on each ImageAndPosition

Manager::save()
  └─ output::generate_files(config, sprite)  [parallel per sprite in project mode]
       ├─ generate_sprite_image   → PNG / WebP / AVIF
       ├─ generate_css / less / scss
       ├─ generate_json / caat / coco2d / html
       └─ save_metadata           → .{name}.spriters (hash for --force skip)
```

---

## Development conventions

### Running tests

```sh
cargo test                         # full suite (~80 tests, ~10 s due to AVIF)
cargo test --test output           # integration tests only
cargo test --lib                   # unit tests only
```

The integration test helper `make_config(source, output)` sets `no_img = true`, `no_css = true`, `force = true` by default — override what you need.

### Adding a feature

1. Add config field to `Config` in `lib.rs` + `Config::default()`
2. Add to `FileConfig` + `apply_file_config` + `apply_env_vars` in `config.rs`
3. Add to `parse_args()` in `cli.rs` (help string + match arm)
4. Implement logic in `lib.rs` or `output.rs`
5. Write integration tests in `tests/output.rs`

### Reproducibility invariant

All output must be byte-for-byte identical across runs on the same inputs:

- `sort_images()` uses a path tiebreaker — never leave `Equal` in a comparator
- Cache busters derive from `compute_sprite_hash` — never use `rand`
- Parallel `collect()` from Rayon is order-preserving for indexed iterators (our usage)

### Cache invalidation

`compute_sprite_hash` (output.rs) hashes: spriters version, relevant config fields, and per-image path + x + y + dimensions + aliases. If you add a field that affects the output image or layout, hash it here.

---

## Do not

- Do not use `rand` — cache busters must be deterministic (see `get_sprite_url` in output.rs)
- Do not add a new output format without updating `generate_files` and adding integration tests
- Do not add a new Config field without updating `config.rs` (FileConfig + apply functions) and `cli.rs`
- Do not break the existing test fixtures in `tests/images/` — the integration tests depend on their exact dimensions
- Do not commit without running `cargo test`

---

## Roadmap status (as of 2026-06-04)

Phase 1 (Glue parity) ✓, Phase 2 (performance) ✓, Phase 3 (beyond Glue) ✓ — all complete.

Remaining phases from `Roadmap.md`:

- **Phase 4.1**: publish to crates.io (API stabilisation, doc coverage)
- **Phase 4.2**: documentation site
- **Phase 4.3**: criterion benchmarks (`benches/`)
- **Phase 4.4**: package manager distribution (Homebrew, AUR, Nix, .deb/.rpm)
