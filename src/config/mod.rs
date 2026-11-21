//! The configuration of a run ([`Config`]), its option types, and its validation.

mod sources;

pub use sources::*;

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::util::{self, Spacing};
use crate::{Algorithm, output};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum JsonFormat {
    #[default]
    Array,
    Hash,
}

/// Output image format for the generated sprite sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpriteFormat {
    #[default]
    Png,
    WebP,
    Avif,
}
impl SpriteFormat {
    /// File extension (without the leading dot).
    pub fn extension(self) -> &'static str {
        match self {
            SpriteFormat::Png => "png",
            SpriteFormat::WebP => "webp",
            SpriteFormat::Avif => "avif",
        }
    }
    /// The corresponding `image::ImageFormat` value used when saving.
    pub fn image_format(self) -> image::ImageFormat {
        match self {
            SpriteFormat::Png => image::ImageFormat::Png,
            SpriteFormat::WebP => image::ImageFormat::WebP,
            SpriteFormat::Avif => image::ImageFormat::Avif,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Colors {
    /// Using the color
    #[default]
    MaxDetected,
    /// Black and white
    Monochrome,
    /// Grayscale, alias for Luma8,
    Grayscale,
    /// 8 bits color palette
    Ansi,
    /// 16 bits color palette
    Bits16,
    Luma8,
    Luma16,
    Luma32f,
    LumaAlpha8,
    LumaAlpha16,
    LumaAlpha32f,
}

/// Order possibilities before using the algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Ordering {
    #[default]
    Maxside,
    MaxsideReverse,
    Width,
    WidthReverse,
    Height,
    HeightReverse,
    Area,
    AreaReverse,
    Filename,
    FilenameReverse,
}
impl Ordering {
    pub fn is_reverse(&self) -> bool {
        matches!(
            self,
            Ordering::MaxsideReverse
                | Ordering::WidthReverse
                | Ordering::HeightReverse
                | Ordering::AreaReverse
                | Ordering::FilenameReverse
        )
    }
}
impl fmt::Display for Ordering {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ordering::Maxside => write!(f, "maxside"),
            Ordering::MaxsideReverse => write!(f, "-maxside"),
            Ordering::Width => write!(f, "width"),
            Ordering::WidthReverse => write!(f, "-width"),
            Ordering::Height => write!(f, "height"),
            Ordering::HeightReverse => write!(f, "-height"),
            Ordering::Area => write!(f, "area"),
            Ordering::AreaReverse => write!(f, "-area"),
            Ordering::Filename => write!(f, "filename"),
            Ordering::FilenameReverse => write!(f, "-filename"),
        }
    }
}
impl FromStr for Ordering {
    type Err = ();
    fn from_str(input: &str) -> Result<Ordering, Self::Err> {
        match input.to_lowercase().as_str() {
            "maxside" => Ok(Ordering::Maxside),
            "-maxside" => Ok(Ordering::MaxsideReverse),
            "width" => Ok(Ordering::Width),
            "-width" => Ok(Ordering::WidthReverse),
            "height" => Ok(Ordering::Height),
            "-height" => Ok(Ordering::HeightReverse),
            "area" => Ok(Ordering::Area),
            "-area" => Ok(Ordering::AreaReverse),
            "filename" => Ok(Ordering::Filename),
            "-filename" => Ok(Ordering::FilenameReverse),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub algorithm: Algorithm,
    /// Before processing the images using the algorithm **spriters** orders the images. The default ordering is `maxside` but you can configure it using the
    /// `--ordering` option.
    ///
    /// `$ spriters source output --ordering=[maxside|width|height|area|filename]`
    ///
    /// You can reverse how any of the available algorithms works prepending a `-`.
    ///
    /// `$ spriters source output --ordering=[-maxside|-width|-height|-area|-filename]`
    pub ordering: Ordering,
    /// Usually designers add some unnecessary transparent space around the images because it is easier for them to work with a larger
    /// canvas. **spriters** can optimize our sprite by croping all the unnecessary transparent spaces that the original images could have.
    pub crop: bool,
    /// If the width of the image is greater than the given value, the image will be resized before making the sprite
    pub max_width: Option<u32>,
    /// If the height of the image is greater than the given value, the image will be resized before making the sprite
    pub max_height: Option<u32>,
    pub resize_algorithm: image::imageops::FilterType,
    /// If you want to spread the images around the sprite but you don’t want to count this space as image width/height (as happens using
    /// `--padding`), you can use the --margin option followed by the margin you want to add:
    ///
    /// ```shell
    /// $ spriters source output --margin=10
    /// $ spriters source output --margin='10 20'
    /// $ spriters source output --margin='10 20 30 40'
    /// ```
    pub margin: Option<String>,
    /// If you want to add the same padding around all images you can use the `--padding` option:
    ///
    /// ```shell
    /// $ spriters source output --padding=10
    /// $ spriters source output --padding='10 20'
    /// $ spriters source output --padding='10 20 30 40'
    /// ```
    pub padding: Option<String>,
    /// **spriters** can automatically scale down your sprites to automatically fit them into low-dpi devices. **spriters** assumes that the source images are
    /// the biggests you want to serve, then **spriters** will create one sprite for each ratio you set in this command.
    ///
    /// ```shell
    /// $ spriters source output --ratios=2,1
    /// $ spriters source output --ratios=2,1.5,1
    /// ```
    pub ratios: Option<String>,
    /// If you decide to add an expires header to your static resources (and if you haven’t already you really should), you need to worry
    /// about cache busting these resources every time you change one of them.
    ///
    /// Cache busting is a technique that prevents a browser from reusing a resource that was already downloaded and cached. Cache in general
    /// is good, but in some situations could be annoying if it’s duration is too long and we want to update a resource now.
    ///
    /// This technique adds a flag to every url that links an external resource (PNG in this case). This flag usually is the last modified
    /// time or the hash of the file.
    ///
    /// **spriters** can use this technique to automatically add the hash of the PNG file to the CSS url, so as soon as the file change
    /// (add/remove an image) the hash will be different and the browser will re-download the image.
    ///
    /// ```shell
    /// $ spriters source output --cachebuster
    /// ```
    ///
    /// Original css:
    /// ```css
    /// .sprite-icons-zoom{ background:url('sprites/icons/icons.png'); top:0; left:0; no-repeat;}
    /// .sprite-icons-wrench_orange{ background:url('sprites/icons/icons.png'); top:0; left:-16; no-repeat;}
    /// // ...
    /// ```
    ///
    /// After `--cachebuster`:
    /// ```css
    /// .sprite-icons-zoom{ background:url('sprites/icons/icons.png?p3c54d'); top:0; left:0; no-repeat;}
    /// .sprite-icons-wrench_orange{ background:url('sprites/icons/icons.png?p3c54d'); top:0; left:-16; no-repeat;}
    /// // ...
    /// ```
    pub cachebuster: bool,
    /// This option has the same purpose than `--cachebuster `but instead of using the hash of the PNG as a queryarg it uses it as part of the filename.
    ///
    /// ```shell
    /// $ spriters source output --cachebuster-filename
    /// ```
    ///
    /// Original css:
    /// ```css
    /// .sprite-icons-zoom{ background:url('sprites/icons/icons.png'); top:0; left:0; no-repeat;}
    /// .sprite-icons-wrench_orange{ background:url('sprites/icons/icons.png'); top:0; left:-16; no-repeat;}
    /// // ...
    /// ```
    ///
    /// After `--cachebuster`:
    /// ```css
    /// .sprite-icons-zoom{ background:url('sprites/icons/icons_p3c54d.png'); top:0; left:0; no-repeat;}
    /// .sprite-icons-wrench_orange{ background:url('sprites/icons/icons_p3c54d.png'); top:0; left:-16; no-repeat;}
    /// // ...
    /// ```
    pub cachebuster_filename: bool,
    /// By default **spriters** store some metadata inside the generated sprites in order to not rebuild it again if the source images and settings are the same.
    /// **spriters** set two different keys, **spriters** with the version number the sprite was build and hash, generated using the source images data, name and all the
    /// relevant sprite settings like padding, margin etc...
    ///
    /// In order to avoid this behaviour you can use `--force` and **spriters** will always build the sprites.
    pub force: bool,
    /// Follow symbolic links.
    pub follow_links: bool,
    /// By default **spriters** adds the namespace sprite to all the generated CSS class names. If you want to use your own namespace you can override the default
    /// one using the --namespace option.
    ///
    /// `$ spriters source output --namespace=my-namespace`
    ///
    /// If you want to completely remove the namespace (both the global and the sprite part) you can use: `$ spriters source output --sprite-namespace= --namespace=`
    pub namespace: Option<String>,
    /// By default **spriters** adds the sprite’s name as past of the CSS class namespace. If you want to use your own namespace you can override the default one using
    /// the `--sprite-namespace` option: `$ spriters source output --sprite-namespace=custom`.
    ///
    /// As part of the new sprite namespace you can use the key %(sprite)s to refer to the original namespace.
    ///
    /// If you want to completely remove the namespace (both the global and the sprite part) you can use: `$ spriters source output --sprite-namespace= --namespace=`
    pub sprite_namespace: Option<String>,
    /// **spriters** by default uses - as separator for the CSS class names. If you want to customize this behaviour you can use `--separator` to specify your own one:
    /// `$ spriters source output --separator=_`.
    ///
    /// If you want to use camelCase instead of a separator, choose camelcase as separator. `$ spriters source output --separator=camelcase`
    /// gives `.spriteIconsMyImage` instead of `.sprite-icons-my-image`.
    pub separator: String,
    /// This flag will make **spriters** suppress all console output.
    pub quiet: bool,
    /// Read directories recursively and add all the images to the same sprite. Example structure:
    /// ```text
    /// source
    /// ├── actions
    /// │   ├── add.png
    /// │   └── remove.png
    /// ├── borders
    /// │   ├── top_left.png
    /// │   └── top_right.png
    /// └── icons
    ///     ├── comment.png
    ///     ├── new.png
    ///     ├── rss.png
    ///     └── blog
    ///         ├── rss.png
    ///         └── atom.png
    /// ```
    ///
    /// If you want to create only one sprite image you should use: `$ spriters source output --recursive`
    ///
    /// On the other hand if you want to create three different sprites (one per folder) you can combine `--project` and `--recursive`:
    /// `$ spriters source output --recursive --project`
    pub recursive: bool,
    /// By default **spriters** adds to the PNG file name the relative url between the CSS and the PNG file. If for any reason you need to change this behaviour,
    /// you can use `--url=<your-static-url-to-the-png-file>` and **spriters** will replace its suggested one with your url.
    pub url: Option<String>,
    /// Specify source directory.
    pub source: std::path::PathBuf,
    /// Specity output directory
    pub output: std::path::PathBuf,
    // output format
    /// Using the `--cocos2d` option, **Spriters** will generate both a sprite image and a xml metadata file compatible with cocos2d.
    pub cocos2d: bool,
    /// Using the --json option, **spriters** will generate both a sprite image and a json metadata file.
    pub json: bool,
    /// Using the `--html` option, **Spriters** will also generate a test html per sprite using all the available CSS classes. This option is only useful for
    /// testing purposes. **Spriters** generate the html file in the same directory as the CSS file.
    pub html: bool,
    /// Using the filename of the source images you can customize the pseudo class related to the images: with `--pseudo-class-separator=__`, if you simply
    /// append `__hover` to the filename **spriters** will add `:hover` to the CSS class name (`button__hover.png` gives `.sprite-icons-button:hover`).
    /// Disabled by default. Recognised pseudo-classes: link, visited, active, hover, focus, first-letter, first-line, first-child, before and after.
    pub pseudo_class_separator: Option<String>,
    /// Usually both CSS and PNG files reside on different folders, e.g. css and img. If you want to choose an individual folder for each type of file you can
    /// use the `--img=<dir> --css=<dir>` options together to customize where the output files will be created.
    ///
    /// `$ spriters source --img=images/compiled --css=css/compiled`
    ///
    /// Every text output (CSS, LESS, SCSS, JSON, HTML…) goes to the `--css` directory (see
    /// [`Config::css_dir`]), and the URL of the sprite image is relative to it, unless
    /// `--url` is given.
    pub css: Option<std::path::PathBuf>,
    /// While using `--css` you can use your own css template using `--css-template=<FILE>`.
    ///
    /// *Note: by default **spriters** will use it’s own internal css template, so this command is not required unless you want to super-customize **spriters**’s css output using
    /// your own template. The template uses the Jinja syntax; its variables are listed in the README.*
    ///
    /// `$ spriters source output --css-template=my_template.jinja`
    pub css_template: Option<std::path::PathBuf>,
    /// Directory of the sprite images and of the metadata files (see [`Config::image_dir`]),
    /// the output directory by default.
    pub img: Option<std::path::PathBuf>,
    /// By using the flag `png8` the output image format will be `png8` instead of `png32`:
    /// an 8-bit palette PNG of up to 256 colours, transparency included (PNG output only).
    pub png8: bool,
    /// **less** is a dynamic stylesheet language that extends CSS with dynamic behaviors. **spriters** can also create `.less` files adding the `--less` option. This
    /// files contain exactly the same CSS code. This option only changes the file format.
    pub less: bool,
    /// scss/sass is another dynamic stylesheet language that extends CSS with dynamic behaviors. **spriters** can also create `.scss` files adding the `--scss` option.
    /// This files contain exactly the same CSS code. This option only changes the file format.
    pub scss: bool,
    /// While using `--scss` you can use your own less template using `--scss-template=<FILE>`.
    pub scss_template: Option<std::path::PathBuf>,
    /// While using `--less` you can use your own less template using `--less-template=<FILE>`.
    ///
    /// *Note: By default **spriters** will use it’s own internal less template, so this command is not required unless you want to super-customize **spriters**’s less output
    /// using your own template. The template uses the Jinja syntax; its variables are listed in the README.
    ///
    /// `$ spriters source output --less-template=my_template.jinja`
    pub less_template: Option<std::path::PathBuf>,
    /// Don’t create any sprite image.
    pub no_img: bool,
    /// Don’t create any CSS file.
    pub no_css: bool,
    /// The default behaviour of **spriters** is to handle one unique sprite folder. If you need to generate several sprites for a project, you can use the `--project`
    /// option to handle multiple folders with only one command.
    ///
    /// The suggested setup is to create a new folder for every sprite, and add inside all the images you need for each one. **spriters** will create a new sprite for every folder:
    ///
    /// ```text
    /// images
    /// ├── actions
    /// │   ├── add.png
    /// │   └── remove.png
    /// ├── borders
    /// │   ├── top_left.png
    /// │   └── top_right.png
    /// └── icons
    ///     ├── comment.png
    ///     ├── new.png
    ///     └── rss.png
    /// ```
    pub project: bool,
    /// Disallow duplicate images: a file identical to another one is left out (see
    /// [`Sprite::failed_images`](crate::Sprite::failed_images)) instead of becoming an alias,
    /// and the run fails.
    pub no_duplicate: bool,
    /// Watch source directory for changes and regenerate sprites automatically
    pub watch: bool,
    /// JSON output format: array (default) or hash
    pub json_format: JsonFormat,
    /// Generate CAAT (Canvas Advanced Animation Toolkit) JSON descriptor
    pub caat: bool,
    /// Embed hash in sprite image filename but keep CSS filename unchanged
    pub cachebuster_filename_only_sprites: bool,
    /// Output format for the sprite image (png, webp, avif)
    pub sprite_format: SpriteFormat,
    /// Glob patterns (matched against filename) for files to skip during discovery.
    /// Repeatable: `--exclude "_draft_*" --exclude "*.tmp"`.
    pub exclude: Vec<String>,
    /// Explicit file list, read from stdin with `--stdin` (or `-i -`).
    /// When non-empty, directory walking is skipped entirely.
    pub files: Vec<PathBuf>,
    /// Emit a `<sprite>.vars.css` file with CSS custom properties for each image.
    pub css_vars: bool,
    /// Custom template file for CSS vars output.
    pub css_vars_template: Option<PathBuf>,
    /// Print what would be generated without writing any files.
    pub dry_run: bool,
    /// Exit non-zero if output is missing or stale; do not regenerate.
    pub check: bool,
    /// Emit a `<sprite>.tailwind.json` file listing all CSS class names for Tailwind's safelist.
    pub tailwind: bool,
    /// Emit a `<sprite>.d.ts` TypeScript definition file with a `SpriteName` union type.
    pub dts: bool,
}

/// Invalid configuration detected by [`Config::check`].
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("source directory does not exist or is not accessible: {}", .0.display())]
    SourceNotDir(PathBuf),
    #[error("output directory does not exist or is not accessible: {}", .0.display())]
    OutputNotDir(PathBuf),
    #[error("--{option} directory does not exist or is not accessible: {}", .path.display())]
    DirNotFound { option: &'static str, path: PathBuf },
    #[error("--{option} {}: {error}", .path.display())]
    InvalidTemplate {
        option: &'static str,
        path: PathBuf,
        error: String,
    },
    #[error("max-width must be greater than 0")]
    ZeroMaxWidth,
    #[error("max-height must be greater than 0")]
    ZeroMaxHeight,
    #[error("ratios: {0}")]
    InvalidRatios(String),
    #[error("invalid {option} '{value}': expected 1, 2, 3 or 4 non-negative integers")]
    InvalidSpacing { option: &'static str, value: String },
}

impl Config {
    /// Directory of the sprite images and of the metadata files: `--img`, or the output
    /// directory.
    pub fn image_dir(&self) -> &Path {
        self.img.as_deref().unwrap_or(&self.output)
    }

    /// Directory of the text outputs (CSS, LESS, SCSS, JSON, HTML…): `--css`, or the output
    /// directory.
    pub fn css_dir(&self) -> &Path {
        self.css.as_deref().unwrap_or(&self.output)
    }

    /// Custom template options, with the option name.
    fn templates(&self) -> [(&'static str, &Option<PathBuf>); 4] {
        [
            ("css-template", &self.css_template),
            ("less-template", &self.less_template),
            ("scss-template", &self.scss_template),
            ("css-vars-template", &self.css_vars_template),
        ]
    }

    /// Check mandatory arguments and some values
    pub fn check(&self) -> Result<(), ConfigError> {
        if self.files.is_empty() && !self.source.is_dir() {
            return Err(ConfigError::SourceNotDir(self.source.clone()));
        }
        if !self.output.is_dir() {
            return Err(ConfigError::OutputNotDir(self.output.clone()));
        }
        for (option, dir) in [("css", &self.css), ("img", &self.img)] {
            if let Some(path) = dir
                && !path.is_dir()
            {
                return Err(ConfigError::DirNotFound {
                    option,
                    path: path.clone(),
                });
            }
        }
        for (option, template) in self.templates() {
            if let Some(path) = template {
                output::check_template(path).map_err(|error| ConfigError::InvalidTemplate {
                    option,
                    path: path.clone(),
                    error,
                })?;
            }
        }
        if self.max_width == Some(0) {
            return Err(ConfigError::ZeroMaxWidth);
        }
        if self.max_height == Some(0) {
            return Err(ConfigError::ZeroMaxHeight);
        }
        if let Some(ratios) = &self.ratios {
            util::parse_ratios(ratios).map_err(ConfigError::InvalidRatios)?;
        }
        for (option, value) in [("padding", &self.padding), ("margin", &self.margin)] {
            if let Some(value) = value
                && value.parse::<Spacing>().is_err()
            {
                return Err(ConfigError::InvalidSpacing {
                    option,
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            resize_algorithm: image::imageops::FilterType::CatmullRom,
            algorithm: Algorithm::Square,
            ordering: Ordering::Maxside,
            crop: false,
            max_width: None,
            max_height: None,
            margin: None,
            padding: None,
            ratios: None,
            cachebuster: false,
            cachebuster_filename: false,
            force: false,
            follow_links: false,
            namespace: None,
            sprite_namespace: None,
            separator: String::from("-"),
            quiet: false,
            recursive: false,
            url: None,
            source: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            output: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cocos2d: false,
            json: false,
            html: false,
            pseudo_class_separator: None,
            css: None,
            css_template: None,
            img: None,
            png8: false,
            less: false,
            scss: false,
            scss_template: None,
            less_template: None,
            no_img: false,
            no_css: false,
            project: false,
            no_duplicate: false,
            watch: false,
            json_format: JsonFormat::Array,
            caat: false,
            cachebuster_filename_only_sprites: false,
            sprite_format: SpriteFormat::Png,
            exclude: vec![],
            files: vec![],
            css_vars: false,
            css_vars_template: None,
            dry_run: false,
            check: false,
            tailwind: false,
            dts: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordering_display() {
        assert_eq!(format!("{}", Ordering::Maxside), "maxside");
        assert_eq!(format!("{}", Ordering::MaxsideReverse), "-maxside");
        assert_eq!(format!("{}", Ordering::Width), "width");
        assert_eq!(format!("{}", Ordering::WidthReverse), "-width");
        assert_eq!(format!("{}", Ordering::Height), "height");
        assert_eq!(format!("{}", Ordering::HeightReverse), "-height");
        assert_eq!(format!("{}", Ordering::Area), "area");
        assert_eq!(format!("{}", Ordering::AreaReverse), "-area");
        assert_eq!(format!("{}", Ordering::Filename), "filename");
        assert_eq!(format!("{}", Ordering::FilenameReverse), "-filename");
    }

    #[test]
    fn test_ordering_from_str() {
        assert_eq!(Ordering::from_str("maxside"), Ok(Ordering::Maxside));
        assert_eq!(Ordering::from_str("MAXSIDE"), Ok(Ordering::Maxside));
        assert_eq!(Ordering::from_str("-maxside"), Ok(Ordering::MaxsideReverse));
        assert_eq!(Ordering::from_str("width"), Ok(Ordering::Width));
        assert_eq!(Ordering::from_str("-width"), Ok(Ordering::WidthReverse));
        assert_eq!(Ordering::from_str("invalid"), Err(()));
    }

    #[test]
    fn test_ordering_is_reverse() {
        assert!(!Ordering::Maxside.is_reverse());
        assert!(Ordering::MaxsideReverse.is_reverse());
        assert!(!Ordering::Width.is_reverse());
        assert!(Ordering::WidthReverse.is_reverse());
        assert!(!Ordering::Height.is_reverse());
        assert!(Ordering::HeightReverse.is_reverse());
        assert!(!Ordering::Area.is_reverse());
        assert!(Ordering::AreaReverse.is_reverse());
        assert!(!Ordering::Filename.is_reverse());
        assert!(Ordering::FilenameReverse.is_reverse());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(!config.crop);
        assert!(!config.cachebuster);
        assert!(!config.force);
        assert!(!config.recursive);
        assert!(!config.project);
        assert_eq!(config.separator, "-");
        assert!(config.namespace.is_none());
        assert!(config.max_width.is_none());
        assert!(config.max_height.is_none());
    }

    #[test]
    fn test_sprite_format_image_format() {
        assert_eq!(SpriteFormat::Png.image_format(), image::ImageFormat::Png);
        assert_eq!(SpriteFormat::WebP.image_format(), image::ImageFormat::WebP);
        assert_eq!(SpriteFormat::Avif.image_format(), image::ImageFormat::Avif);
    }

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("spriters_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn test_config_check_valid() {
        let dir = tmp_dir("chk_ok");
        let config = Config {
            source: dir.clone(),
            output: dir.clone(),
            ..Default::default()
        };
        assert!(config.check().is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_config_check_output_not_dir() {
        let dir = tmp_dir("chk_out");
        let config = Config {
            source: dir.clone(),
            output: PathBuf::from("/nonexistent/__spriters_test__"),
            ..Default::default()
        };
        let err = config.check();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            err,
            Err(ConfigError::OutputNotDir(PathBuf::from(
                "/nonexistent/__spriters_test__"
            )))
        );
    }

    #[test]
    fn test_config_check_source_not_dir() {
        let dir = tmp_dir("chk_src");
        let config = Config {
            source: PathBuf::from("/nonexistent/__spriters_test__"),
            output: dir.clone(),
            ..Default::default()
        };
        let err = config.check();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            err,
            Err(ConfigError::SourceNotDir(PathBuf::from(
                "/nonexistent/__spriters_test__"
            )))
        );
    }

    #[test]
    fn test_config_check_missing_source_is_fine_in_file_list_mode() {
        let dir = tmp_dir("chk_files");
        let config = Config {
            source: PathBuf::from("/nonexistent/__spriters_test__"),
            output: dir.clone(),
            files: vec![PathBuf::from("icon.png")],
            ..Default::default()
        };
        let result = config.check();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_config_error_messages() {
        let source = ConfigError::SourceNotDir(PathBuf::from("/missing/icons"));
        assert_eq!(
            source.to_string(),
            "source directory does not exist or is not accessible: /missing/icons"
        );
        assert_eq!(
            ConfigError::ZeroMaxWidth.to_string(),
            "max-width must be greater than 0"
        );
    }

    #[test]
    fn test_config_check_rejects_invalid_ratios_and_spacing() {
        let dir = tmp_dir("chk_values");
        let base = Config {
            source: dir.clone(),
            output: dir.clone(),
            ..Default::default()
        };
        let check = |config: Config| config.check();

        assert!(matches!(
            check(Config {
                ratios: Some(String::from("abc")),
                ..base.clone()
            }),
            Err(ConfigError::InvalidRatios(_))
        ));
        assert!(matches!(
            check(Config {
                ratios: Some(String::from("2,0")),
                ..base.clone()
            }),
            Err(ConfigError::InvalidRatios(_))
        ));
        assert_eq!(
            check(Config {
                padding: Some(String::from("abc")),
                ..base.clone()
            }),
            Err(ConfigError::InvalidSpacing {
                option: "padding",
                value: String::from("abc")
            })
        );
        assert_eq!(
            check(Config {
                margin: Some(String::from("1 2 3 4 5")),
                ..base.clone()
            }),
            Err(ConfigError::InvalidSpacing {
                option: "margin",
                value: String::from("1 2 3 4 5")
            })
        );
        assert_eq!(
            check(Config {
                ratios: Some(String::from("2,1")),
                padding: Some(String::from("1 2")),
                margin: Some(String::from("3")),
                ..base.clone()
            }),
            Ok(())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_config_check_max_width_zero() {
        let dir = tmp_dir("chk_mw0");
        let config = Config {
            source: dir.clone(),
            output: dir.clone(),
            max_width: Some(0),
            ..Default::default()
        };
        let err = config.check();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(err, Err(ConfigError::ZeroMaxWidth));
    }

    #[test]
    fn test_config_check_max_height_zero() {
        let dir = tmp_dir("chk_mh0");
        let config = Config {
            source: dir.clone(),
            output: dir.clone(),
            max_height: Some(0),
            ..Default::default()
        };
        let err = config.check();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(err, Err(ConfigError::ZeroMaxHeight));
    }
}
