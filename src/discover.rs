//! Discovery of the source images of a sprite: directory walk or file list, exclusions,
//! reading, deduplication and decoding. Generated files are never taken as sources (see
//! [`OutputFilter`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use log::{debug, error, info, warn};
use walkdir::WalkDir;

use crate::decode::{CachedImage, ImageCache, load_image, processing_key};
use crate::labels::remove_label_collisions;
use crate::{Config, ImageAndPosition, Sprite, output, sprite_name};

/// An image file found by discovery, before decoding.
struct Candidate {
    path: PathBuf,
    /// Size in bytes and content hash: duplicates share both.
    len: u64,
    hash: u64,
    /// Modification time and size of the file, used as cache key.
    stamp: Option<(std::time::SystemTime, u64)>,
    source: Source,
}

/// Where the pixels of a [`Candidate`] come from.
enum Source {
    /// Raw file content, still to decode.
    Bytes(Vec<u8>),
    /// Already decoded and processed, taken from an [`ImageCache`].
    Decoded(DynamicImage),
}

/// Length in 1× units of `pixels` taken at `ratio`, rounded up so the image always fits.
fn logical_length(pixels: u32, ratio: f32) -> u32 {
    (pixels as f32 / ratio).ceil() as u32
}

impl Sprite {
    /// Discover images: it return all valid images within a folder.
    ///
    /// All files not supported by [Image](https://crates.io/crates/image) or beginning with '.' will be ignored.
    ///
    /// If the folder doesn't contain any valid image it will exit with an error message.
    pub(crate) fn discover_images(&mut self, cache: Option<&ImageCache>) {
        use rayon::iter::Either;
        use rayon::prelude::*;

        // Phase 1a (sequential, cheap): list candidate paths, in walk order.
        // Phase 1b (parallel): read and hash every candidate.
        // Phase 1c (sequential, walk order): deduplicate. Duplicate images (same size + hash)
        // are tracked as aliases of the first one seen; their bytes are dropped right away
        // and only the primary is decoded and placed in the sprite.
        let exclude_patterns: Vec<glob::Pattern> = self
            .config
            .exclude
            .iter()
            .filter_map(|p| {
                glob::Pattern::new(p)
                    .map_err(|e| warn!("Invalid exclude pattern '{}': {}", p, e))
                    .ok()
            })
            .collect();

        let match_opts = glob::MatchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let outputs = OutputFilter::new(&self.config);
        let is_candidate = |path: &Path| -> bool {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                return false;
            };
            let Some(ext) = path.extension() else {
                return false;
            };
            if name.starts_with('.')
                || !is_image_extension(&ext.to_ascii_lowercase().to_string_lossy())
            {
                return false;
            }
            if outputs.is_generated_file(path) {
                debug!("Skipping generated file: {}", name);
                return false;
            }
            if exclude_patterns
                .iter()
                .any(|p| p.matches_with(&name, match_opts))
            {
                debug!("Excluding: {}", name);
                return false;
            }
            true
        };

        // Phase 1a: directory walk (default) or explicit file list (e.g. from stdin).
        let paths: Vec<PathBuf> = if self.config.files.is_empty() {
            let mut walker = WalkDir::new(std::path::Path::new(&self.config.source))
                .follow_links(self.config.follow_links);
            if !self.config.recursive {
                walker = walker.max_depth(1);
            }
            walker
                .into_iter()
                .filter_map(|e| e.ok())
                .map(walkdir::DirEntry::into_path)
                .filter(|path| is_candidate(path))
                .collect()
        } else {
            self.config
                .files
                .iter()
                .filter(|path| is_candidate(path))
                .cloned()
                .collect()
        };

        // The highest ratio represents the source resolution (e.g., 2.0 for --ratios=2,1).
        // max-width/height are specified in 1× units; scale up accordingly.
        let display_ratio = self.ratios.last().copied().unwrap_or(1.0);
        let processing = processing_key(&self.config, display_ratio);

        // Phase 1b: read and hash in parallel; `partition_map` keeps the walk order. Files
        // that are unchanged since a cached run (`--watch`) are neither read nor decoded again.
        let (read, mut failed): (Vec<Candidate>, Vec<String>) =
            paths.par_iter().partition_map(|path| {
                let stamp = std::fs::metadata(path)
                    .ok()
                    .and_then(|metadata| Some((metadata.modified().ok()?, metadata.len())));
                if let (Some(cache), Some((modified, len))) = (cache, stamp)
                    && let Some((hash, image)) = cache.get(path, modified, len, processing)
                {
                    return Either::Left(Candidate {
                        path: path.clone(),
                        len,
                        hash,
                        stamp,
                        source: Source::Decoded(image),
                    });
                }
                match std::fs::read(path) {
                    Ok(bytes) => Either::Left(Candidate {
                        path: path.clone(),
                        len: bytes.len() as u64,
                        hash: twox_hash::XxHash64::oneshot(713705, &bytes),
                        stamp,
                        source: Source::Bytes(bytes),
                    }),
                    Err(e) => {
                        error!("Cannot read {:?}: {}", path, e);
                        Either::Right(path.to_string_lossy().into_owned())
                    }
                }
            });

        // Phase 1c: deduplicate in walk order, so the primary is always the first one seen.
        let mut seen: HashMap<(u64, u64), String> = HashMap::new();
        let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut candidates: Vec<Candidate> = Vec::with_capacity(read.len());
        for candidate in read {
            let path = candidate.path.to_string_lossy().into_owned();
            let key = (candidate.len, candidate.hash);
            if let Some(primary) = seen.get(&key) {
                if self.config.no_duplicate {
                    // Left out instead of exiting: in project mode this runs in a rayon
                    // thread, and the other sprites must still be built and written. The
                    // failure gives the exit code.
                    error!("{path} is left out: identical to {primary} (--no-duplicate)");
                    failed.push(path);
                    continue;
                }
                warn!("{} and {} are identical — consolidating", path, primary);
                alias_map.entry(primary.clone()).or_default().push(path);
                continue; // the bytes of the duplicate are dropped here
            }
            seen.insert(key, path);
            candidates.push(candidate);
        }

        // Phase 2 (parallel): decode, resize, then crop each image independently.
        let config = &self.config;
        let (images, failed_decoding): (Vec<ImageAndPosition>, Vec<String>) = candidates
            .into_par_iter()
            .partition_map(|candidate| {
                let path = candidate.path.to_string_lossy().into_owned();
                let source_hash = candidate.hash;
                let load_result = match candidate.source {
                    Source::Decoded(image) => Ok((image, None)),
                    Source::Bytes(bytes) => {
                        load_image(&bytes, &path, config, display_ratio).map(|(image, original)| {
                            if let (Some(cache), Some((modified, len))) = (cache, candidate.stamp) {
                                cache.insert(
                                    candidate.path.clone(),
                                    CachedImage {
                                        modified,
                                        len,
                                        processing,
                                        source_hash,
                                        image: std::sync::Arc::new(image.clone()),
                                    },
                                );
                            }
                            (image, Some(original))
                        })
                    }
                };

                match load_result {
                    Ok((img2, original)) => {
                        let (final_w, final_h) = (img2.width(), img2.height());
                        // Pixels are at the highest ratio; the layout and every text output
                        // use 1× (CSS pixel) units.
                        let (logical_w, logical_h) = (
                            logical_length(final_w, display_ratio),
                            logical_length(final_h, display_ratio),
                        );
                        if display_ratio != 1.0
                            && (logical_w as f32 * display_ratio != final_w as f32
                                || logical_h as f32 * display_ratio != final_h as f32)
                        {
                            warn!(
                                "{} ({}x{}) is not a multiple of the {}x ratio: lower ratios may be blurry",
                                path, final_w, final_h, display_ratio
                            );
                        }
                        let aliases = alias_map.get(&path).cloned().unwrap_or_default();
                        let origin = match original {
                            Some((original_w, original_h)) => format!("{original_w}x{original_h}"),
                            None => String::from("cached"),
                        };
                        info!(
                            "\t Adding: {} ({} -> {}x{}){}",
                            path,
                            origin,
                            final_w,
                            final_h,
                            if aliases.is_empty() {
                                String::new()
                            } else {
                                format!(" [aliases: {}]", aliases.len())
                            }
                        );
                        Either::Left(ImageAndPosition {
                            path,
                            image: img2,
                            computed_width: logical_w,
                            computed_height: logical_h,
                            aliases,
                            source_hash,
                            ..Default::default()
                        })
                    }
                    Err(err) => {
                        error!("Cannot open image {}: {}", path, err);
                        Either::Right(path)
                    }
                }
            });
        self.images = images;
        failed.extend(failed_decoding);
        failed.extend(remove_label_collisions(
            &mut self.images,
            self.config.pseudo_class_separator.as_deref(),
        ));
        failed.sort();
        self.failed_images = failed;

        if self.images.is_empty() {
            error!("No images were found in the source directory");
        }
    }
}

/// Canonical form of `path`. Only the parent has to exist, so that it also works for files
/// that were just deleted (as reported by the watcher).
fn canonical_path(path: &Path) -> Option<PathBuf> {
    if let Ok(full) = path.canonicalize() {
        return Some(full);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Some(parent.canonicalize().ok()?.join(path.file_name()?))
}

/// Recognises the files spriters writes, so they are never read back as source images.
///
/// By default the source and output directories are the same: without this filter, the
/// sprite of a run would become an input image of the next one. Only generated files are
/// filtered: images the user keeps in an output directory are still sources.
#[derive(Debug, Default)]
pub struct OutputFilter {
    /// Canonical output directories: images (`--img`) and text outputs (`--css`).
    outputs: Vec<PathBuf>,
    /// Sprites whose generated files may be in the output directories.
    sprite_names: Vec<String>,
}

impl OutputFilter {
    /// Filter for the outputs of `config`: the sprite built from `config.source`, plus every
    /// sprite recorded by a metadata file in the image directory.
    pub fn new(config: &Config) -> Self {
        let mut sprite_names = list_sprite_names(config.image_dir());
        sprite_names.push(sprite_name(&config.source));
        let mut outputs: Vec<PathBuf> = [config.image_dir(), config.css_dir()]
            .into_iter()
            .filter_map(canonical_path)
            .collect();
        outputs.dedup();
        OutputFilter {
            outputs,
            sprite_names,
        }
    }

    /// Whether `dir` is an output directory.
    pub fn is_output_dir(&self, dir: &Path) -> bool {
        canonical_path(dir).is_some_and(|dir| self.outputs.contains(&dir))
    }

    /// Whether `path` is a file spriters generated in an output directory (sprite image,
    /// text output or metadata). `path` does not need to exist (deleted files).
    pub fn is_generated_file(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|name| {
            output::is_generated_file_name(&name.to_string_lossy(), &self.sprite_names)
        }) && canonical_path(path)
            .as_deref()
            .and_then(Path::parent)
            .is_some_and(|parent| self.outputs.iter().any(|output| output == parent))
    }

    /// Whether `dir` holds at least one source image (a non-hidden image that spriters did
    /// not generate).
    pub(crate) fn has_source_images(&self, dir: &Path) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.filter_map(|entry| entry.ok()).any(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            path.is_file()
                && !name.starts_with('.')
                && path.extension().is_some_and(|ext| {
                    is_image_extension(&ext.to_ascii_lowercase().to_string_lossy())
                })
                && !self.is_generated_file(&path)
        })
    }
}

/// Scan `output_dir` for `.*.spriters` metadata files and return the sprite names found.
pub fn list_sprite_names(output_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let file_name = e.file_name().to_string_lossy().into_owned();
            file_name
                .strip_prefix('.')
                .and_then(|rest| rest.strip_suffix(".spriters"))
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

fn is_image_extension(ext: &str) -> bool {
    matches!(
        ext,
        "jpg"
            | "jpeg"
            | "jpe"
            | "jif"
            | "jfif"
            | "jfi"
            | "png"
            | "gif"
            | "webp"
            | "tiff"
            | "tif"
            | "bmp"
            | "dib"
            | "jp2"
            | "j2k"
            | "jpf"
            | "jpx"
            | "jpm"
            | "mj2"
            | "svg"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_sprite_names_ignores_malformed_metadata_names() {
        // Review 3.7: a `.spriters` file panicked with "byte range starts at 1 but ends at 0".
        let dir = std::env::temp_dir().join(format!("spriters_meta_names_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for file in [
            ".spriters",
            "..spriters",
            ".icons.spriters",
            "icons.spriters",
        ] {
            std::fs::write(dir.join(file), "").unwrap();
        }
        let names = list_sprite_names(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(names, vec!["icons"]);
    }
}
