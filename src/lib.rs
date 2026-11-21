//! Sprite sheet generator: the images of a directory are discovered (`discover`), decoded
//! (`decode`), placed (`layout`) into a [`Sprite`], then written with their stylesheets and
//! metadata (`output`). [`Manager`] runs that pipeline for a whole project.

pub mod completions;
pub mod config;
mod decode;
mod discover;
mod labels;
mod layout;
mod manager;
mod output;
mod sprite;
mod stats;
pub mod util;

pub use config::{Colors, Config, ConfigError, JsonFormat, Ordering, SpriteFormat};
pub use decode::{ImageCache, crop};
pub use discover::{OutputFilter, list_sprite_names};
pub use layout::Algorithm;
pub use manager::Manager;
pub use sprite::{ImageAndPosition, Sprite, get_greatest_quality, sprite_name};
pub use stats::{SpriteStats, StatsError, sprite_stats};
