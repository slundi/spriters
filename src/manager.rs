//! Builds the sprites of a run (one, or one per sub-directory with `--project`) and writes
//! or checks their files.

use std::path::PathBuf;

use log::{debug, error, info};

use crate::decode::ImageCache;
use crate::discover::OutputFilter;
use crate::{Config, Sprite, output, sprite_name};

#[derive(Debug, Default)]
pub struct Manager {
    pub project: bool,
    pub config: Config,
    pub sprites: Vec<Sprite>,
    /// Decoded images kept between runs (`--watch`), see [`ImageCache`].
    pub cache: Option<std::sync::Arc<ImageCache>>,
}

impl Manager {
    /// Build every sprite, keeping all of them (with their pixels) in `sprites`.
    ///
    /// To generate files, prefer [`Manager::generate`]: it does not hold every decoded
    /// sprite in memory at once.
    pub fn find_sprites(&mut self) {
        use rayon::prelude::*;

        self.sprites = self
            .sprite_sources()
            .into_par_iter()
            .map(|(config, path)| Sprite::with_cache(&config, path, self.cache.as_deref()))
            .collect();
        if self.sprites.is_empty() {
            error!("Input folder does not have images.");
        }
    }

    /// Build, save and release each sprite within the same task: peak memory is about one
    /// decoded sprite per thread instead of every sprite of the project.
    ///
    /// `sprites` keeps the layout of each sprite, without pixel data.
    ///
    /// Returns whether everything succeeded: each sprite has images, all of them could be
    /// loaded, and every file was written.
    pub fn generate(&mut self) -> bool {
        self.build_each(crate::output::generate_files)
    }

    /// Like [`Manager::is_up_to_date`] (used by `--check`), building and releasing each
    /// sprite in turn instead of holding all of them. Also `false` when a sprite has no
    /// images or one of them could not be loaded.
    pub fn check(&mut self) -> bool {
        self.build_each(|config, sprite| !output::staleness(config, sprite).any())
    }

    /// Build each sprite, run `action` on it, then drop its pixels. Returns whether there
    /// was at least one sprite, each one [complete](Sprite::is_complete), and `action`
    /// returned `true` for every sprite.
    fn build_each(&mut self, action: impl Fn(&Config, &Sprite) -> bool + Sync) -> bool {
        use rayon::prelude::*;

        let config = &self.config;
        let cache = self.cache.as_deref();
        let results: Vec<(Sprite, bool)> = self
            .sprite_sources()
            .into_par_iter()
            .map(|(sprite_config, path)| {
                let mut sprite = Sprite::with_cache(&sprite_config, path, cache);
                let result = action(config, &sprite) && sprite.is_complete();
                sprite.release_pixels();
                (sprite, result)
            })
            .collect();
        if results.is_empty() {
            error!("Input folder does not have images.");
        }
        let all = !results.is_empty() && results.iter().all(|(_, result)| *result);
        self.sprites = results.into_iter().map(|(sprite, _)| sprite).collect();
        all
    }

    /// The directory of each sprite to build, with its configuration: in project mode,
    /// `source` points to the sub-directory, since `discover_images` walks `config.source`.
    fn sprite_sources(&self) -> Vec<(Config, PathBuf)> {
        if self.project {
            // An output directory inside the project that only holds generated files
            // (e.g. `dist/`) is not a sprite.
            let outputs = OutputFilter::new(&self.config);
            // Collect subdirectory paths first (sequential — read_dir is not parallel).
            let entries = match std::fs::read_dir(&self.config.source) {
                Ok(entries) => entries,
                Err(e) => {
                    error!(
                        "Cannot read source directory {:?}: {}",
                        self.config.source, e
                    );
                    return Vec::new();
                }
            };
            entries
                .filter_map(|e| e.ok())
                .filter_map(|de| {
                    let p = de.path();
                    if !p.is_dir() || (p.is_symlink() && !self.config.follow_links) {
                        return None;
                    }
                    if p.file_name()?.to_string_lossy().starts_with('.') {
                        return None;
                    }
                    if outputs.is_output_dir(&p) && !outputs.has_source_images(&p) {
                        debug!("Skipping output directory: {:?}", p);
                        return None;
                    }
                    info!("Creating sprite: {}", sprite_name(&p));
                    let sprite_config = Config {
                        source: p.clone(),
                        ..self.config.clone()
                    };
                    Some((sprite_config, p))
                })
                .collect()
        } else {
            let p = PathBuf::from(&self.config.source);
            info!("Creating sprite: {}", sprite_name(&p));
            vec![(self.config.clone(), p)]
        }
    }

    /// Save all sprites inside this manager. Returns whether everything succeeded, as
    /// [`Manager::generate`].
    pub fn save(&self) -> bool {
        use rayon::prelude::*;
        // Counted rather than `all`: a failure must not stop the other sprites.
        let failures = self
            .sprites
            .par_iter()
            .filter(|sprite| {
                !(crate::output::generate_files(&self.config, sprite) && sprite.is_complete())
            })
            .count();
        !self.sprites.is_empty() && failures == 0
    }

    /// Returns `true` if every sprite in this manager is current (no regeneration needed).
    /// Used by `--check` mode.
    pub fn is_up_to_date(&self) -> bool {
        self.sprites
            .iter()
            .all(|s| !output::staleness(&self.config, s).any())
    }
}
