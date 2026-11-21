use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

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

use image::{ColorType, DynamicImage};
use log::*;
use util::{Spacing, absolute_length, horizontal_spacing, vertical_spacing};
use walkdir::WalkDir;

pub mod algorithm;
mod algorithm_square;
pub mod completions;
pub mod config;
mod output;
pub mod util;

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

/// Used to order the images before adding them to the canvas can be tuned. By default the algorithm is `Square``,
/// but in some situations using another ordering like vertical or horizontal could be useful depending on the kind of images you are spriting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Algorithm {
    /// The square algorithm was inspired by the Binary Tree Bin Packing Algorithm Article by Jake Gordon.
    #[default]
    Square,
    /// The vertical one allocates the images vertically aligning them to the left of the sprite.
    Vertical,
    /// The vertical-right one allocates the images vertically aligning them to the right of the sprite.
    Horizontal,
    /// The Horizontal one allocates the images aligning them to the top of the sprite.
    Diagonal,
    /// The Horizontal-bottom one allocates the images aligning them to the bottom of the sprite.
    VerticalRight,
    /// The diagonal one allocates the images diagonally. It was inspired by the Diagonal CSS Sprites Article by Aaron Barker.
    HorizontalBottom,
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
    /// [`Sprite::failed_images`]) instead of becoming an alias, and the run fails.
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

#[derive(Debug, Default)]
pub struct ImageAndPosition {
    pub path: String,
    /// Pixels at the highest ratio (the source resolution).
    pub image: DynamicImage,
    /// Position in the sprite, in 1× (CSS pixel) units. During layout, top-left corner of the
    /// cell (margin included); afterwards, of the CSS box (after the margin).
    pub x: u32,
    pub y: u32,
    /// Size in 1× (CSS pixel) units: the pixel size divided by the highest ratio, rounded up
    /// (equal to the pixel size without `--ratios`). After layout, the padding is included:
    /// it is the size of the CSS box.
    pub computed_width: u32,
    pub computed_height: u32,
    /// Paths of images with identical content that consolidate into this region.
    pub aliases: Vec<String>,
    /// XxHash64 of the source file bytes, used to detect pixel changes between runs.
    pub source_hash: u64,
}
/// CSS-safe label of the image file `path`: its slugified file stem. Used for the image
/// itself and for its aliases.
pub(crate) fn label_from_path(path: &str) -> String {
    slug::slugify(
        std::path::Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
    )
}

/// Pseudo-classes recognised at the end of a file name with `--pseudo-class-separator`.
pub(crate) const CSS_PSEUDO_CLASSES: [&str; 10] = [
    "link",
    "visited",
    "active",
    "hover",
    "focus",
    "first-letter",
    "first-line",
    "first-child",
    "before",
    "after",
];

/// CSS label and pseudo-classes of the image file `path`. With a pseudo-class `separator`,
/// known pseudo-classes are taken from the end of the file stem before it is slugified:
/// with `__`, `button__hover.png` gives `("button", ":hover")` and `link__focus__hover.png`
/// gives `("link", ":focus:hover")`. Otherwise the label is [`label_from_path`].
pub(crate) fn css_label_from_path(path: &str, separator: Option<&str>) -> (String, String) {
    let stem = Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let mut base: &str = &stem;
    let mut pseudo_classes = Vec::new();
    if let Some(separator) = separator.filter(|separator| !separator.is_empty()) {
        while let Some((rest, suffix)) = base.rsplit_once(separator) {
            let suffix = suffix.to_ascii_lowercase();
            if rest.is_empty() || !CSS_PSEUDO_CLASSES.contains(&suffix.as_str()) {
                break;
            }
            pseudo_classes.push(suffix);
            base = rest;
        }
    }
    let pseudo = pseudo_classes
        .iter()
        .rev()
        .map(|pseudo_class| format!(":{pseudo_class}"))
        .collect();
    (slug::slugify(base), pseudo)
}

/// The names under which `path` appears in the outputs: its label (JSON keys, class names),
/// plus its CSS selector key `label:pseudo` when it has pseudo-classes. The latter never
/// equals a label, which cannot contain `:`.
fn label_keys(path: &str, pseudo_separator: Option<&str>) -> Vec<String> {
    let mut keys = vec![label_from_path(path)];
    let (label, pseudo) = css_label_from_path(path, pseudo_separator);
    if !pseudo.is_empty() {
        keys.push(format!("{label}{pseudo}"));
    }
    keys
}

/// Leave out the images and aliases whose label is already taken by another image: both
/// would get the same class name (and JSON key) at different positions, e.g. `my_image.png`
/// and `my-image.png`, or `rss.png` in two folders with `--recursive`. With a
/// `pseudo_separator`, CSS selectors (`button:hover`) must be unique too (see
/// [`label_keys`]).
///
/// Images claim their label first, in path order, so the same one is kept whatever the walk
/// order; then aliases. An alias with the label of its own image adds nothing and is dropped
/// silently. Returns the paths left out.
fn remove_label_collisions(
    images: &mut Vec<ImageAndPosition>,
    pseudo_separator: Option<&str>,
) -> Vec<String> {
    images.sort_by(|a, b| a.path.cmp(&b.path));
    // Label -> path of the image whose position it designates.
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut left_out = Vec::new();

    images.retain(|image| {
        let keys = label_keys(&image.path, pseudo_separator);
        if let Some((key, owner)) = keys
            .iter()
            .find_map(|key| owners.get(key).map(|owner| (key, owner)))
        {
            error!(
                "{} is left out: its label '{}' is already used by {}, rename one of them",
                image.path, key, owner
            );
            left_out.push(image.path.clone());
            left_out.extend(image.aliases.iter().cloned());
            return false;
        }
        for key in keys {
            owners.insert(key, image.path.clone());
        }
        true
    });

    for image in images.iter_mut() {
        image.aliases.retain(|alias| {
            let keys = label_keys(alias, pseudo_separator);
            let taken = keys
                .iter()
                .find_map(|key| owners.get(key).map(|owner| (key, owner)));
            match taken {
                Some((_, owner)) if *owner == image.path => {
                    debug!("{alias}: same label as {}, nothing to add", image.path);
                    false
                }
                Some((key, owner)) => {
                    error!(
                        "{alias} is left out: its label '{key}' is already used by {owner}, rename one of them"
                    );
                    left_out.push(alias.clone());
                    false
                }
                None => {
                    for key in keys {
                        owners.insert(key, image.path.clone());
                    }
                    true
                }
            }
        });
    }
    left_out
}

impl ImageAndPosition {
    pub fn label(&self) -> String {
        label_from_path(&self.path)
    }

    /// Where the image pixels start, in 1× units: inside the padding of its CSS box.
    pub fn pixel_origin(&self, padding: &Spacing) -> (u32, u32) {
        (self.x + padding.left, self.y + padding.top)
    }
}

#[derive(Debug, Default)]
pub struct Sprite {
    pub config: Config,
    pub path: PathBuf,
    pub ratios: Vec<f32>,
    pub images: Vec<ImageAndPosition>,
    /// Paths of the image files left out of the sprite: they could not be read or decoded,
    /// their label is already used by another image, or they duplicate another image with
    /// `--no-duplicate`.
    pub failed_images: Vec<String>,
    pub margin: Spacing,
    pub padding: Spacing,
    /// Hash of the images and their layout, computed on first use by the outputs (they all
    /// need it: staleness, metadata, cache busters). The layout never changes after
    /// [`Sprite::new`].
    pub(crate) layout_hash: std::sync::OnceLock<u64>,
}
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

/// Hash of the options that shape a decoded image: an image cached with other values must
/// be processed again.
fn processing_key(config: &Config, display_ratio: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = twox_hash::XxHash64::with_seed(0);
    (config.max_width, config.max_height, config.crop).hash(&mut hasher);
    config.resize_algorithm.hash(&mut hasher);
    display_ratio.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// Decode an image file and process it for the sprite: resize to `max_width`/`max_height`
/// (ratio-aware), then crop. Returns the RGBA8 image and the original size.
///
/// Resize happens before crop so that max-width/height apply to the full image (including
/// any transparent border), and crop removes the leftover padding after.
fn load_image(
    bytes: &[u8],
    path: &str,
    config: &Config,
    display_ratio: f32,
) -> Result<(DynamicImage, (u32, u32)), String> {
    let is_svg = path.to_lowercase().ends_with(".svg");
    // SVG: rasterized directly to the target size (no post-load resize needed).
    // Raster: loaded then resized if max-width/height requires it.
    let img = if is_svg {
        rasterize_svg(bytes, display_ratio, config.max_width, config.max_height)?
    } else {
        image::load_from_memory(bytes).map_err(|e| e.to_string())?
    };
    let original = (img.width(), img.height());
    let mut img = if is_svg {
        img
    } else {
        let (width, height) = get_dimensions_from_max(
            img.width(),
            img.height(),
            config.max_width,
            config.max_height,
            display_ratio,
        );
        if width != img.width() || height != img.height() {
            img.resize_exact(width, height, config.resize_algorithm)
        } else {
            img
        }
    };
    if config.crop {
        img = crop(img, None);
    }
    // Stored as RGBA8 once (free when it already is), so that the sprite rendering borrows
    // it instead of converting it per ratio.
    Ok((DynamicImage::ImageRgba8(img.into_rgba8()), original))
}

/// Images decoded by earlier runs (used by `--watch`): a file whose size and modification
/// time did not change is neither read nor decoded again.
///
/// Entries also record the options that shape the decoded image (max width/height, crop,
/// resize filter, highest ratio); an image processed with other values is decoded again.
#[derive(Debug, Default)]
pub struct ImageCache {
    entries: std::sync::Mutex<HashMap<PathBuf, CachedImage>>,
}

#[derive(Debug)]
struct CachedImage {
    modified: std::time::SystemTime,
    len: u64,
    processing: u64,
    source_hash: u64,
    /// Shared, so that a lookup only clones the pointer under the lock.
    image: std::sync::Arc<DynamicImage>,
}

impl ImageCache {
    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<PathBuf, CachedImage>> {
        // A panic while holding the lock leaves a usable map: entries are inserted whole.
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Content hash and processed image of `path`, if cached for this exact file state.
    fn get(
        &self,
        path: &Path,
        modified: std::time::SystemTime,
        len: u64,
        processing: u64,
    ) -> Option<(u64, DynamicImage)> {
        let (source_hash, image) = {
            let entries = self.entries();
            let entry = entries.get(path)?;
            if entry.modified != modified || entry.len != len || entry.processing != processing {
                return None;
            }
            (entry.source_hash, std::sync::Arc::clone(&entry.image))
        };
        // The pixels are copied outside the lock, so parallel lookups do not wait on it.
        Some((source_hash, DynamicImage::clone(&image)))
    }

    fn insert(&self, path: PathBuf, image: CachedImage) {
        self.entries().insert(path, image);
    }

    /// Forget the images whose file no longer exists.
    pub fn retain_existing(&self) {
        self.entries().retain(|path, _| path.exists());
    }

    /// Number of cached images.
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    /// Whether no image is cached.
    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }
}

/// Length in 1× units of `pixels` taken at `ratio`, rounded up so the image always fits.
fn logical_length(pixels: u32, ratio: f32) -> u32 {
    (pixels as f32 / ratio).ceil() as u32
}

/// Name of the sprite built from the directory `path`, used for every output file name.
///
/// Paths without a final component (`.`, `..`) are resolved first; a directory that still
/// has no name (`/`) falls back to `sprite`.
pub fn sprite_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            path.canonicalize()
                .ok()?
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| String::from("sprite"))
}

impl Sprite {
    pub fn new(config: &Config, path: PathBuf) -> Sprite {
        Sprite::with_cache(config, path, None)
    }

    /// Like [`Sprite::new`], reusing the images of `cache` whose file did not change, and
    /// storing the newly decoded ones in it.
    pub fn with_cache(config: &Config, path: PathBuf, cache: Option<&ImageCache>) -> Sprite {
        let mut result = Sprite {
            config: config.clone(),
            path,
            margin: Spacing::from(config.margin.clone()),
            padding: Spacing::from(config.padding.clone()),
            ..Default::default()
        };
        // ratios: list & sort them (validated by `Config::check`, fall back to 1x otherwise)
        result.ratios = match config.ratios.as_deref().map(util::parse_ratios) {
            Some(Ok(ratios)) => ratios,
            Some(Err(e)) => {
                error!("{e}, using 1x");
                vec![1f32]
            }
            None => vec![1f32],
        };
        result.discover_images(cache);
        result.process();
        result
    }

    /// Name of the sprite, see [`sprite_name`].
    pub fn name(&self) -> String {
        sprite_name(&self.path)
    }

    /// Whether the sprite holds every image found: none was left out (see
    /// [`Sprite::failed_images`]), and it is not empty.
    pub fn is_complete(&self) -> bool {
        !self.images.is_empty() && self.failed_images.is_empty()
    }

    /// Drop the decoded pixels of every image once the outputs are written; the layout
    /// (positions and sizes) is kept.
    pub fn release_pixels(&mut self) {
        for img in self.images.iter_mut() {
            img.image = DynamicImage::default();
        }
    }

    /// Generate sprite map
    fn process(&mut self) {
        match self.config.algorithm {
            Algorithm::Square => Algorithm::square(self),
            Algorithm::Vertical => Algorithm::vertical(self),
            Algorithm::Horizontal => Algorithm::horizontal(self),
            Algorithm::Diagonal => Algorithm::diagonal(self),
            Algorithm::VerticalRight => Algorithm::vertical_right(self),
            Algorithm::HorizontalBottom => Algorithm::horizontal_bottom(self),
        }
        self.cells_to_boxes();
    }

    /// The algorithms place cells (image + padding + margin) at `x`/`y`. Turn each cell into
    /// the box seen by CSS: it starts after the margin and includes the padding, so that
    /// `x`/`y`/`computed_*` are what every output describes. Pixels are drawn inside the
    /// padding (see [`ImageAndPosition::pixel_origin`]).
    fn cells_to_boxes(&mut self) {
        for img in self.images.iter_mut() {
            img.x += self.margin.left;
            img.y += self.margin.top;
            img.computed_width += self.padding.left + self.padding.right;
            img.computed_height += self.padding.top + self.padding.bottom;
        }
    }
    /// Discover images: it return all valid images within a folder.
    ///
    /// All files not supported by [Image](https://crates.io/crates/image) or beginning with '.' will be ignored.
    ///
    /// If the folder doesn't contain any valid image it will exit with an error message.
    ///
    /// The list of images will be ordered using the desired ordering algorithm. The default is 'Maxside'.
    fn discover_images(&mut self, cache: Option<&ImageCache>) {
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
        self.sort_images();
    }

    /// Sort images depending on the `config.ordering` value.
    ///
    /// All orderings use `path` as a tiebreaker so the result is a total order —
    /// no two elements compare as Equal. This guarantees byte-for-byte reproducible
    /// output regardless of the order images were collected from the filesystem.
    fn sort_images(&mut self) {
        let vspace = vertical_spacing(
            self.padding.top,
            self.padding.bottom,
            self.margin.top,
            self.margin.bottom,
        );
        let hspace = horizontal_spacing(
            self.padding.left,
            self.padding.right,
            self.margin.left,
            self.margin.right,
        );
        // Sizes and spacing are both in 1× (CSS pixel) units.
        let ratio = 1.0;

        self.images.sort_by(|a, b| {
            let primary = match self.config.ordering {
                Ordering::Maxside | Ordering::MaxsideReverse => {
                    let b_max = absolute_length(b.computed_height, vspace, ratio)
                        .max(absolute_length(b.computed_width, hspace, ratio));
                    let a_max = absolute_length(a.computed_height, vspace, ratio)
                        .max(absolute_length(a.computed_width, hspace, ratio));
                    b_max.cmp(&a_max)
                }
                Ordering::Height | Ordering::HeightReverse => {
                    let b_h = absolute_length(b.computed_height, vspace, ratio);
                    let a_h = absolute_length(a.computed_height, vspace, ratio);
                    b_h.cmp(&a_h)
                }
                Ordering::Width | Ordering::WidthReverse => {
                    let b_w = absolute_length(b.computed_width, hspace, ratio);
                    let a_w = absolute_length(a.computed_width, hspace, ratio);
                    b_w.cmp(&a_w)
                }
                Ordering::Area | Ordering::AreaReverse => {
                    let b_area = absolute_length(b.computed_width, hspace, ratio)
                        * absolute_length(b.computed_height, vspace, ratio);
                    let a_area = absolute_length(a.computed_width, hspace, ratio)
                        * absolute_length(a.computed_height, vspace, ratio);
                    b_area.cmp(&a_area)
                }
                Ordering::Filename | Ordering::FilenameReverse => Path::new(&b.path)
                    .file_name()
                    .unwrap()
                    .cmp(Path::new(&a.path).file_name().unwrap()),
            };
            // Tiebreaker: full path ensures a total order independent of collection order.
            primary.then_with(|| a.path.cmp(&b.path))
        });
        if self.config.ordering.is_reverse() {
            self.images.reverse();
        }
    }

    #[cfg(test)]
    fn get_max_color_type(&self) -> ColorType {
        let mut current = ColorType::L8;
        for i in self.images.iter() {
            current = get_greatest_quality(&current, &i.image.color());
        }
        current
    }

    /// Calculate the sprite dimensions based on positioned images, including the right and
    /// bottom margin of the last ones.
    pub fn dimensions(&self) -> (u32, u32) {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        for img in self.images.iter() {
            width = width.max(img.x + img.computed_width + self.margin.right);
            height = height.max(img.y + img.computed_height + self.margin.bottom);
        }
        (width, height)
    }
}

/// Order color set by quality/number of colors/alpha
const SORTED_COLOR_TYPES: [ColorType; 10] = [
    ColorType::L8,
    ColorType::La8,
    ColorType::Rgb8,
    ColorType::Rgba8,
    ColorType::L16,
    ColorType::La16,
    ColorType::Rgb16,
    ColorType::Rgba16,
    ColorType::Rgb32F,
    ColorType::Rgba32F,
];

pub fn get_greatest_quality(a: &ColorType, b: &ColorType) -> ColorType {
    let mut index_a = 0usize;
    let mut index_b = 0usize;
    for c in SORTED_COLOR_TYPES.iter().enumerate() {
        if a == c.1 {
            index_a = c.0;
        }
        if b == c.1 {
            index_b = c.0;
        }
    }
    SORTED_COLOR_TYPES[index_a.max(index_b)]
}

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

// ─── Stats API ────────────────────────────────────────────────────────────────

/// Per-sprite statistics derived from the generated sprite image.
#[derive(Debug)]
pub struct SpriteStats {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Size of the sprite image file on disk, in bytes.
    pub file_size: u64,
    /// Number of pixels with alpha > 0.
    pub used_pixels: u64,
    /// Total pixel count (width × height).
    pub total_pixels: u64,
}

impl SpriteStats {
    /// Packing efficiency as an integer percentage (0–100).
    pub fn packing_efficiency_pct(&self) -> u64 {
        if self.total_pixels == 0 {
            return 0;
        }
        self.used_pixels * 100 / self.total_pixels
    }
}

/// Why [`sprite_stats`] could not report on a sprite.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("no sprite image found")]
    NotFound,
    #[error("the sprite image recorded in the metadata is missing: {}", .0.display())]
    Missing(PathBuf),
    #[error("cannot read {}: {error}", .path.display())]
    Unreadable { path: PathBuf, error: String },
}

/// Load stats for a named sprite in `output_dir`.
///
/// Uses the image file recorded in the sprite metadata, which accounts for cache-busted
/// names and the image format; when it is missing, that is the error, since any other file
/// may be left from an older run. Metadata that records no image (older versions) falls
/// back to the most recently written sprite image of that name.
pub fn sprite_stats(output_dir: &Path, name: &str) -> Result<SpriteStats, StatsError> {
    let recorded = output::read_metadata_file(&output_dir.join(output::meta_file_name(name)))
        .image_file
        // A bare file name only: never follow a path out of the output directory.
        .filter(|file| Path::new(file).file_name() == Some(std::ffi::OsStr::new(file)));
    let img_path = match recorded {
        Some(file) => {
            let path = output_dir.join(file);
            if !path.is_file() {
                return Err(StatsError::Missing(path));
            }
            path
        }
        None => output::find_sprite_image(output_dir, name).ok_or(StatsError::NotFound)?,
    };

    let unreadable = |error: String| StatsError::Unreadable {
        path: img_path.clone(),
        error,
    };
    let file_size = std::fs::metadata(&img_path)
        .map_err(|e| unreadable(e.to_string()))?
        .len();
    let img = image::open(&img_path).map_err(|e| unreadable(e.to_string()))?;
    let (w, h) = (img.width(), img.height());
    let total = w as u64 * h as u64;

    Ok(SpriteStats {
        name: name.to_owned(),
        width: w,
        height: h,
        file_size,
        used_pixels: count_visible_pixels(&img, total),
        total_pixels: total,
    })
}

/// Number of non-transparent pixels of `image` (`total` when it has no alpha channel),
/// counted in the decoded buffer instead of a converted RGBA copy.
fn count_visible_pixels(image: &DynamicImage, total: u64) -> u64 {
    let visible = |alpha: &[u8], stride: usize| {
        alpha
            .iter()
            .step_by(stride)
            .filter(|&&value| value > 0)
            .count() as u64
    };
    match image {
        DynamicImage::ImageRgba8(buffer) => visible(buffer.as_raw().get(3..).unwrap_or(&[]), 4),
        DynamicImage::ImageLumaA8(buffer) => visible(buffer.as_raw().get(1..).unwrap_or(&[]), 2),
        DynamicImage::ImageRgba16(buffer) => {
            buffer.pixels().filter(|pixel| pixel.0[3] > 0).count() as u64
        }
        DynamicImage::ImageLumaA16(buffer) => {
            buffer.pixels().filter(|pixel| pixel.0[1] > 0).count() as u64
        }
        other if !other.color().has_alpha() => total,
        other => other
            .to_rgba8()
            .pixels()
            .filter(|pixel| pixel.0[3] > 0)
            .count() as u64,
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
    fn has_source_images(&self, dir: &Path) -> bool {
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

#[inline]
pub fn add(left: usize, right: usize) -> usize {
    left + right
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

/// Rasterize an SVG at the appropriate pixel size.
///
/// The natural SVG size (in CSS px) is scaled by `display_ratio` for retina support,
/// then further constrained by `max_width`/`max_height` if set.
/// Returns a fully-decoded `DynamicImage` ready for the normal processing pipeline.
fn rasterize_svg(
    bytes: &[u8],
    display_ratio: f32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<DynamicImage, String> {
    use resvg::{tiny_skia, usvg};

    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|e| format!("SVG parse error: {e}"))?;

    let natural = tree.size().to_int_size();
    let natural_w = natural.width();
    let natural_h = natural.height();

    // Scale natural size by display ratio so the rasterized image is crisp at the
    // target DPI (e.g. 2× for --ratios=2,1).
    let render_w = (natural_w as f32 * display_ratio).round() as u32;
    let render_h = (natural_h as f32 * display_ratio).round() as u32;

    // Apply max-dimension constraints at the scaled size.
    // Never zero: at least 1 pixel per side.
    let (target_w, target_h) =
        get_dimensions_from_max(render_w, render_h, max_width, max_height, display_ratio);

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or("Failed to create pixmap")?;

    let scale_x = target_w as f32 / natural_w as f32;
    let scale_y = target_h as f32 / natural_h as f32;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    Ok(DynamicImage::ImageRgba8(pixmap_to_rgba(&pixmap)))
}

/// Straight-alpha RGBA image of a premultiplied tiny-skia pixmap: the same pixels as
/// `encode_png` followed by a PNG decode, without the zlib round trip.
fn pixmap_to_rgba(pixmap: &resvg::tiny_skia::Pixmap) -> image::RgbaImage {
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), rgba)
        .expect("a pixmap holds width × height RGBA pixels")
}

/// Result of cropping an image with nothing left to keep (fully transparent, or entirely
/// the crop colour): a 1×1 pixel taken from the image, so the icon keeps a sprite entry
/// (e.g. a placeholder) instead of producing a zero-sized one.
fn blank_placeholder(image: &DynamicImage) -> DynamicImage {
    warn!(
        "Image ({}x{}) has no pixel left after crop, keeping a 1x1 placeholder",
        image.width(),
        image.height()
    );
    image.crop_imm(0, 0, 1, 1)
}

/// Crop an image. When no color is provided, it crops white or transparent areas (when alpha channel is available).
///
/// An image with nothing to keep is reduced to a 1×1 placeholder.
pub fn crop(image: DynamicImage, color: Option<u32>) -> DynamicImage {
    let (crop_alpha, cropable_color) = match color {
        Some(color) => (false, color),
        None => (image.color().has_alpha(), 0xffffffff), // default is white
    };

    // Common 8-bit types are scanned in place, with the same RGBA conversion as
    // `GenericImageView::get_pixel`; other types are converted once.
    let keep =
        |rgba: [u8; 4]| !is_color_or_transparent(image::Rgba(rgba), crop_alpha, cropable_color);
    let bounds = match &image {
        DynamicImage::ImageRgba8(buffer) => content_bounds(buffer, |pixel| keep(pixel.0)),
        DynamicImage::ImageRgb8(buffer) => content_bounds(buffer, |pixel| {
            let [red, green, blue] = pixel.0;
            keep([red, green, blue, 255])
        }),
        DynamicImage::ImageLumaA8(buffer) => content_bounds(buffer, |pixel| {
            let [luma, alpha] = pixel.0;
            keep([luma, luma, luma, alpha])
        }),
        DynamicImage::ImageLuma8(buffer) => content_bounds(buffer, |pixel| {
            let [luma] = pixel.0;
            keep([luma, luma, luma, 255])
        }),
        other => content_bounds(&other.to_rgba8(), |pixel| keep(pixel.0)),
    };
    let Some((left, top, right, bottom)) = bounds else {
        return blank_placeholder(&image);
    };
    let (width, height) = (right - left + 1, bottom - top + 1);
    debug!(
        "\t\t{}x{}, left={}, top={} => {}x{}",
        image.width(),
        image.height(),
        left,
        top,
        width,
        height,
    );
    image.crop_imm(left, top, width, height)
}

/// Inclusive bounding box `(left, top, right, bottom)` of the pixels to `keep`, `None` when
/// there is none.
///
/// Only the croppable border is visited: rows are scanned from the top and from the bottom
/// until content is found, then each row in between from the left up to the current left
/// bound and from the right down to the current right bound.
fn content_bounds<P: image::Pixel>(
    buffer: &image::ImageBuffer<P, Vec<P::Subpixel>>,
    keep: impl Fn(&P) -> bool,
) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = buffer.dimensions();
    let row_has_content = |y: u32| (0..width).any(|x| keep(buffer.get_pixel(x, y)));

    let top = (0..height).find(|&y| row_has_content(y))?;
    let bottom = (top..height).rev().find(|&y| row_has_content(y))?;
    let mut left = width;
    let mut right: Option<u32> = None;
    for y in top..=bottom {
        if let Some(x) = (0..left).find(|&x| keep(buffer.get_pixel(x, y))) {
            left = x;
        }
        let start = right.map_or(0, |right| right + 1);
        if let Some(x) = (start..width).rev().find(|&x| keep(buffer.get_pixel(x, y))) {
            right = Some(x);
        }
    }
    Some((left, top, right?, bottom))
}

fn is_color_or_transparent(pixel: image::Rgba<u8>, crop_alpha: bool, color: u32) -> bool {
    if crop_alpha {
        return pixel.0[3] == 0;
    }
    u32::from_be_bytes(pixel.0) == color
}

/// Get the `(width, height)` tuple after applying max-width/max-height constraints.
///
/// `display_ratio` scales the limits: at ratio 2 the source is already 2× so the
/// effective ceiling is `max_width * display_ratio` (and similarly for height).
/// Pass `1.0` when ratios are not in use.
///
/// The limited side gets the (whole) limit; the other one is scaled with it and rounded to
/// the nearest pixel, never below 1: a 1000×1 image at max-width 10 is 10×1, not 10×0.
fn get_dimensions_from_max(
    width: u32,
    height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
    display_ratio: f32,
) -> (u32, u32) {
    let ratio = display_ratio as f64;
    let (mut new_width, mut new_height) = (width as f64, height as f64);
    if let Some(max) = max_width {
        // Floored: the result must not exceed the limit.
        let limit = (max as f64 * ratio).floor();
        if limit < new_width {
            new_height *= limit / new_width;
            new_width = limit;
        }
    }
    if let Some(max) = max_height {
        let limit = (max as f64 * ratio).floor();
        if limit < new_height {
            new_width *= limit / new_height;
            new_height = limit;
        }
    }
    // The limits are whole numbers and the other side only shrinks, so rounding never
    // exceeds a limit (except to keep 1 pixel).
    let to_pixels = |length: f64| (length.round() as u32).max(1);
    (to_pixels(new_width), to_pixels(new_height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn test_is_croppable_color() {
        // not croppable color
        // transparent one
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            false,
            0xffffff00
        ));
        // full opacity
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 255]),
            false,
            0xffffff00
        ));

        // transparent but not set
        assert!(!is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            false,
            0xffffff00
        ));

        // transparent cropable
        assert!(is_color_or_transparent(
            image::Rgba::from([128u8, 128, 128, 0]),
            true,
            0xffffff00
        ));

        // croppable color no alpha
        assert!(is_color_or_transparent(
            image::Rgba::from([255u8, 255, 255, 255]),
            false,
            0xffffffff
        ));
    }

    #[test]
    fn test_greatest_quality() {
        assert_eq!(
            get_greatest_quality(&ColorType::L8, &ColorType::Rgb16),
            ColorType::Rgb16
        );
        assert_eq!(
            get_greatest_quality(&ColorType::La8, &ColorType::La8),
            ColorType::La8
        );
        assert_eq!(
            get_greatest_quality(&ColorType::Rgba32F, &ColorType::Rgb16),
            ColorType::Rgba32F
        );
    }

    #[test]
    fn test_max_dimensions() {
        // ratio 1.0 — identical to pre-ratio behaviour
        assert_eq!(get_dimensions_from_max(10, 20, None, None, 1.0), (10, 20));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), None, 1.0), (5, 10));
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(15), None, 1.0),
            (10, 20)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, None, Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, None, Some(40), 1.0),
            (10, 20)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(15), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(10), Some(10), 1.0),
            (5, 10)
        );
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(2), Some(10), 1.0),
            (2, 4)
        );
        // 10×20 → 5×10 (max-width) → 2.5×5 (max-height): rounded, not truncated.
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(5), 1.0),
            (3, 5)
        );
    }

    #[test]
    fn test_max_dimensions_never_zero() {
        // Review 6.7: the scaled side was truncated, so a very elongated image lost its
        // only row or column.
        assert_eq!(
            get_dimensions_from_max(1000, 1, Some(10), None, 1.0),
            (10, 1)
        );
        assert_eq!(
            get_dimensions_from_max(1, 1000, None, Some(10), 1.0),
            (1, 10)
        );
        assert_eq!(
            get_dimensions_from_max(1000, 1, Some(10), Some(10), 2.0),
            (20, 1)
        );
        // Even when the limit itself rounds down to nothing (max 1 at a 0.5 ratio).
        assert_eq!(get_dimensions_from_max(10, 10, Some(1), None, 0.5), (1, 1));
    }

    #[test]
    fn test_max_dimensions_round_to_nearest() {
        // Truncation gave the lower value in each case.
        // 8 × 10/30 = 2.67 → 3.
        assert_eq!(get_dimensions_from_max(30, 8, Some(10), None, 1.0), (10, 3));
        // 30 × 20/80 = 7.5 → 8.
        assert_eq!(
            get_dimensions_from_max(30, 80, None, Some(20), 1.0),
            (8, 20)
        );
        // 1.5 ratio: limit 100 × 1.5 = 150, 200 × 150/301 = 99.67 → 100.
        assert_eq!(
            get_dimensions_from_max(301, 200, Some(100), None, 1.5),
            (150, 100)
        );
    }

    #[test]
    fn test_max_dimensions_never_exceed_the_limit() {
        for width in 1..60 {
            for height in 1..60 {
                for max in 1..20 {
                    let (new_width, new_height) =
                        get_dimensions_from_max(width, height, Some(max), Some(max), 1.0);
                    assert!(new_width <= max.max(1) && new_height <= max.max(1));
                    assert!(new_width >= 1 && new_height >= 1);
                    assert!(new_width <= width && new_height <= height);
                }
            }
        }
    }

    #[test]
    fn test_max_dimensions_ratio_aware() {
        // ratio 2.0: effective max = limit * 2, so a 400px image is within a 200px limit at 2×
        assert_eq!(
            get_dimensions_from_max(400, 300, Some(200), None, 2.0),
            (400, 300)
        );
        // 500px > 200*2=400 → resize to 400; height = 300 * (400/500) = 240
        assert_eq!(
            get_dimensions_from_max(500, 300, Some(200), None, 2.0),
            (400, 240)
        );
        // max-height only: 600px > 100*2=200 → resize; width = 400 * (200/600) = 133
        assert_eq!(
            get_dimensions_from_max(400, 600, None, Some(100), 2.0),
            (133, 200)
        );
        // ratio 1.5: effective max = 100*1.5=150; 200 > 150 → resize to 150; h = 100*(150/200)=75
        assert_eq!(
            get_dimensions_from_max(200, 100, Some(100), None, 1.5),
            (150, 75)
        );
    }

    #[test]
    fn test_crop() {
        let img = image::open("tests/images/europe/no.png").unwrap();
        assert_eq!(img.width(), 120);
        assert_eq!(img.height(), 100);

        // crop transparency: must match the bounding box of non-transparent pixels
        let (bbox_width, bbox_height) = opaque_bounding_box(&img);
        let mut img2 = img.clone();
        img2 = crop(img2, None);
        assert_eq!((img2.width(), img2.height()), (bbox_width, bbox_height));
        assert_eq!((img2.width(), img2.height()), (100, 73));

        // crop white... will be same size
        let mut img2 = img.clone();
        img2 = crop(img2, Some(0xffffffff));
        assert_eq!(img2.width(), 120);
        assert_eq!(img2.height(), 100);
    }

    /// Reference implementation: size of the bounding box of all non-transparent pixels.
    fn opaque_bounding_box(image: &DynamicImage) -> (u32, u32) {
        let rgba = image.to_rgba8();
        let opaque: Vec<(u32, u32)> = rgba
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel.0[3] != 0)
            .map(|(x, y, _)| (x, y))
            .collect();
        let min_x = opaque.iter().map(|point| point.0).min().unwrap();
        let max_x = opaque.iter().map(|point| point.0).max().unwrap();
        let min_y = opaque.iter().map(|point| point.1).min().unwrap();
        let max_y = opaque.iter().map(|point| point.1).max().unwrap();
        (max_x - min_x + 1, max_y - min_y + 1)
    }

    /// Transparent `width`×`height` image with the given rectangles painted opaque red.
    fn transparent_with(width: u32, height: u32, opaque: &[(u32, u32, u32, u32)]) -> DynamicImage {
        let mut canvas = image::RgbaImage::new(width, height);
        for &(left, top, rect_width, rect_height) in opaque {
            for y in top..top + rect_height {
                for x in left..left + rect_width {
                    canvas.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
                }
            }
        }
        DynamicImage::ImageRgba8(canvas)
    }

    #[test]
    fn test_crop_single_row_near_top() {
        // Review 3.1: a single opaque row at y=1 gave 10x8 instead of 10x1.
        let cropped = crop(transparent_with(10, 10, &[(0, 1, 10, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (10, 1));
    }

    #[test]
    fn test_crop_single_row_near_bottom() {
        let cropped = crop(transparent_with(10, 10, &[(0, 8, 10, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (10, 1));
    }

    #[test]
    fn test_crop_off_center_block_keeps_only_content() {
        let cropped = crop(transparent_with(10, 10, &[(2, 1, 3, 2)]), None);
        assert_eq!((cropped.width(), cropped.height()), (3, 2));
        assert!(
            cropped.to_rgba8().pixels().all(|pixel| pixel.0[3] == 255),
            "only the opaque block must remain"
        );
    }

    #[test]
    fn test_crop_bounding_box_of_scattered_pixels() {
        // Two single pixels: the result is their bounding box, gaps included.
        let cropped = crop(transparent_with(12, 9, &[(3, 1, 1, 1), (7, 6, 1, 1)]), None);
        assert_eq!((cropped.width(), cropped.height()), (5, 6));
    }

    /// Reference bounding box `(left, top, width, height)` of the pixels `crop` must keep,
    /// found by testing every pixel; `None` when there is none.
    fn brute_force_bounds(
        image: &DynamicImage,
        color: Option<u32>,
    ) -> Option<(u32, u32, u32, u32)> {
        let (crop_alpha, cropable) = match color {
            Some(color) => (false, color),
            None => (image.color().has_alpha(), 0xffffffff),
        };
        let kept: Vec<(u32, u32)> = image
            .pixels()
            .filter(|(_, _, pixel)| !is_color_or_transparent(*pixel, crop_alpha, cropable))
            .map(|(x, y, _)| (x, y))
            .collect();
        let left = kept.iter().map(|point| point.0).min()?;
        let right = kept.iter().map(|point| point.0).max()?;
        let top = kept.iter().map(|point| point.1).min()?;
        let bottom = kept.iter().map(|point| point.1).max()?;
        Some((left, top, right - left + 1, bottom - top + 1))
    }

    #[test]
    fn test_crop_matches_brute_force_on_random_images() {
        let mut state: u32 = 0x9e37_79b9;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for case in 0..300 {
            let (width, height) = (1 + next() % 24, 1 + next() % 24);
            let mut canvas = image::RgbaImage::new(width, height);
            // From blank to dense; some cases draw a solid block instead of scattered pixels.
            let density = case % 7;
            for (_, _, pixel) in canvas.enumerate_pixels_mut() {
                if density > 0 && next() % 40 < density {
                    *pixel = image::Rgba([next() as u8, 0, 0, 1 + (next() % 255) as u8]);
                }
            }
            let image = DynamicImage::ImageRgba8(canvas);
            for color in [None, Some(0x0000_0000)] {
                let cropped = crop(image.clone(), color);
                match brute_force_bounds(&image, color) {
                    Some((left, top, width, height)) => {
                        assert_eq!(
                            cropped.to_rgba8(),
                            image.crop_imm(left, top, width, height).to_rgba8(),
                            "case {case}, color {color:?}"
                        );
                    }
                    None => assert_eq!(cropped.dimensions(), (1, 1), "case {case}: placeholder"),
                }
            }
        }
    }

    /// Semi-transparent gradient and shapes: demultiplication rounding matters here.
    const TRANSLUCENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48">
  <defs><linearGradient id="g"><stop offset="0" stop-color="#f80" stop-opacity="0.1"/>
  <stop offset="1" stop-color="#08f" stop-opacity="0.9"/></linearGradient></defs>
  <rect width="64" height="48" fill="url(#g)"/>
  <circle cx="20" cy="24" r="15" fill="#3c3" fill-opacity="0.37"/>
</svg>"##;

    #[test]
    fn test_pixmap_to_rgba_matches_png_round_trip() {
        // Review 5.3: same pixels as the former `encode_png` + `load_from_memory`.
        use resvg::{tiny_skia, usvg};
        let tree =
            usvg::Tree::from_data(TRANSLUCENT_SVG.as_bytes(), &usvg::Options::default()).unwrap();
        let mut pixmap = tiny_skia::Pixmap::new(64, 48).unwrap();
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        let round_trip = image::load_from_memory(&pixmap.encode_png().unwrap())
            .unwrap()
            .to_rgba8();
        let direct = pixmap_to_rgba(&pixmap);
        assert_eq!(direct, round_trip);
        // The test is meaningful: there are semi-transparent pixels.
        assert!(
            direct
                .pixels()
                .any(|pixel| pixel.0[3] > 0 && pixel.0[3] < 255)
        );
    }

    #[test]
    fn test_rasterize_svg_returns_rgba8_at_the_target_size() {
        let image = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 1.0, None, None).unwrap();
        assert_eq!(image.color(), ColorType::Rgba8);
        assert_eq!(image.dimensions(), (64, 48));

        let retina = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 2.0, None, None).unwrap();
        assert_eq!(retina.dimensions(), (128, 96));
        let limited = rasterize_svg(TRANSLUCENT_SVG.as_bytes(), 1.0, Some(32), None).unwrap();
        assert_eq!(limited.dimensions(), (32, 24));
    }

    #[test]
    fn test_crop_keeps_the_image_type() {
        // No alpha channel: white is cropped.
        let mut rgb = image::RgbImage::from_pixel(12, 10, image::Rgb([255, 255, 255]));
        rgb.put_pixel(4, 3, image::Rgb([10, 20, 30]));
        rgb.put_pixel(7, 5, image::Rgb([10, 20, 30]));
        let cropped = crop(DynamicImage::ImageRgb8(rgb), None);
        assert_eq!(cropped.color(), ColorType::Rgb8);
        assert_eq!(cropped.dimensions(), (4, 3));

        // 16-bit images go through the RGBA conversion and keep their type.
        let mut rgba16 = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::new(9, 9);
        rgba16.put_pixel(2, 6, image::Rgba([65535, 0, 0, 65535]));
        let cropped = crop(DynamicImage::ImageRgba16(rgba16), None);
        assert_eq!(cropped.color(), ColorType::Rgba16);
        assert_eq!(cropped.dimensions(), (1, 1));

        // Grayscale with an explicit colour (white).
        let mut luma = image::GrayImage::from_pixel(8, 8, image::Luma([255]));
        luma.put_pixel(1, 1, image::Luma([0]));
        luma.put_pixel(2, 4, image::Luma([0]));
        let cropped = crop(DynamicImage::ImageLuma8(luma), Some(0xffffffff));
        assert_eq!(cropped.color(), ColorType::L8);
        assert_eq!(cropped.dimensions(), (2, 4));
    }

    #[test]
    fn test_crop_fully_transparent_gives_placeholder() {
        // Review 3.2: left = right = width overflowed `width - left - right`.
        let cropped = crop(transparent_with(10, 10, &[]), None);
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(
            cropped.get_pixel(0, 0).0[3],
            0,
            "placeholder stays transparent"
        );
    }

    #[test]
    fn test_crop_transparent_single_column_or_row() {
        // One-pixel-wide images skip one axis: the other axis must not overflow either.
        let column = crop(transparent_with(1, 10, &[]), None);
        assert_eq!((column.width(), column.height()), (1, 1));
        let row = crop(transparent_with(10, 1, &[]), None);
        assert_eq!((row.width(), row.height()), (1, 1));
    }

    #[test]
    fn test_crop_single_pixel_is_unchanged() {
        let transparent = crop(transparent_with(1, 1, &[]), None);
        assert_eq!((transparent.width(), transparent.height()), (1, 1));
        let opaque = crop(transparent_with(1, 1, &[(0, 0, 1, 1)]), None);
        assert_eq!(opaque.get_pixel(0, 0), image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_crop_image_entirely_of_crop_color() {
        let canvas = image::RgbaImage::from_pixel(8, 6, image::Rgba([255, 255, 255, 255]));
        let cropped = crop(DynamicImage::ImageRgba8(canvas), Some(0xffffffff));
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(cropped.get_pixel(0, 0), image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_crop_explicit_color() {
        let mut canvas = image::RgbaImage::from_pixel(10, 10, image::Rgba([255, 255, 255, 255]));
        canvas.put_pixel(7, 6, image::Rgba([0, 0, 0, 255]));
        let cropped = crop(DynamicImage::ImageRgba8(canvas), Some(0xffffffff));
        assert_eq!((cropped.width(), cropped.height()), (1, 1));
        assert_eq!(cropped.get_pixel(0, 0), image::Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn test_image_and_position_label() {
        let iap = ImageAndPosition {
            path: String::from("/path/to/my_image.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap.label(), "my-image");

        // Test with spaces and special characters
        let iap2 = ImageAndPosition {
            path: String::from("/path/to/My Image File.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap2.label(), "my-image-file");

        // Test with underscores
        let iap3 = ImageAndPosition {
            path: String::from("icon_arrow_left.png"),
            image: DynamicImage::new_rgba8(10, 10),
            x: 0,
            y: 0,
            computed_width: 10,
            computed_height: 10,
            ..Default::default()
        };
        assert_eq!(iap3.label(), "icon-arrow-left");
    }

    /// Image `path` with `aliases`, for the label collision tests.
    fn labelled(path: &str, aliases: &[&str]) -> ImageAndPosition {
        ImageAndPosition {
            path: String::from(path),
            aliases: aliases.iter().map(|alias| String::from(*alias)).collect(),
            ..Default::default()
        }
    }

    fn paths(images: &[ImageAndPosition]) -> Vec<(&str, Vec<&str>)> {
        images
            .iter()
            .map(|image| {
                let aliases = image.aliases.iter().map(String::as_str).collect();
                (image.path.as_str(), aliases)
            })
            .collect()
    }

    #[test]
    fn test_label_collisions_keep_the_first_path() {
        // Review 6.4: both images got the class `my-image` at different positions.
        // Given in reverse order: the kept image must not depend on the walk order.
        let mut images = vec![
            labelled("icons/rss.png", &[]),
            labelled("icons/my_image.png", &[]),
            labelled("icons/blog/rss.png", &[]),
            labelled("icons/my-image.png", &[]),
            labelled("icons/other.png", &[]),
        ];
        let left_out = remove_label_collisions(&mut images, None);

        assert_eq!(
            paths(&images),
            [
                ("icons/blog/rss.png", vec![]),
                ("icons/my-image.png", vec![]),
                ("icons/other.png", vec![]),
            ]
        );
        assert_eq!(left_out, ["icons/my_image.png", "icons/rss.png"]);
    }

    #[test]
    fn test_label_collisions_with_aliases() {
        let mut images = vec![
            // Same content in two folders: same label, same position.
            labelled("icons/rss.png", &["icons/blog/rss.png", "icons/feed.png"]),
            // The alias `home` collides with an image: images claim labels first.
            labelled("icons/house.png", &["icons/old/home.png"]),
            labelled("icons/home.png", &[]),
            // The alias `feed` was claimed by the alias of `rss.png`.
            labelled("icons/stream.png", &["icons/blog/feed.png"]),
            // A left-out image takes its aliases with it.
            labelled("icons/zz/other.png", &["icons/zz/other-copy.png"]),
            labelled("icons/other.png", &[]),
        ];
        let mut left_out = remove_label_collisions(&mut images, None);
        left_out.sort();

        assert_eq!(
            paths(&images),
            [
                ("icons/home.png", vec![]),
                ("icons/house.png", vec![]),
                ("icons/other.png", vec![]),
                ("icons/rss.png", vec!["icons/feed.png"]),
                ("icons/stream.png", vec![]),
            ]
        );
        assert_eq!(
            left_out,
            [
                "icons/blog/feed.png",
                "icons/old/home.png",
                "icons/zz/other-copy.png",
                "icons/zz/other.png",
            ]
        );
    }

    #[test]
    fn test_unique_labels_are_untouched() {
        let mut images = vec![labelled("b.png", &["b-copy.png"]), labelled("a.png", &[])];
        assert!(remove_label_collisions(&mut images, None).is_empty());
        assert_eq!(
            paths(&images),
            [("a.png", vec![]), ("b.png", vec!["b-copy.png"])]
        );
    }

    #[test]
    fn test_css_label_from_path() {
        // Review §7: `--pseudo-class-separator` was ignored, and slugify turned
        // `button__hover` into `button-hover`.
        let separator = Some("__");
        for (path, label, pseudo) in [
            ("icons/button__hover.png", "button", ":hover"),
            ("icons/my_link__focus__HOVER.png", "my-link", ":focus:hover"),
            ("icons/arrow__first-child.png", "arrow", ":first-child"),
            // Not a pseudo-class: part of the label.
            ("icons/menu__open.png", "menu-open", ""),
            ("icons/menu__open__hover.png", "menu-open", ":hover"),
            // A pseudo-class alone is a label.
            ("icons/__hover.png", "hover", ""),
            ("icons/hover.png", "hover", ""),
            ("icons/plain.png", "plain", ""),
        ] {
            assert_eq!(
                css_label_from_path(path, separator),
                (String::from(label), String::from(pseudo)),
                "{path}"
            );
        }
        // Without separator (the default), or an empty one, the file stem is the label.
        for separator in [None, Some("")] {
            assert_eq!(
                css_label_from_path("icons/button__hover.png", separator),
                (String::from("button-hover"), String::new())
            );
        }
        assert_eq!(
            css_label_from_path("icons/nav-active.png", Some("-")),
            (String::from("nav"), String::from(":active"))
        );
    }

    #[test]
    fn test_label_collisions_with_pseudo_classes() {
        let mut images = vec![
            labelled("icons/button.png", &[]),
            labelled("icons/button__hover.png", &[]),
            // Same selector `.button:hover`.
            labelled("icons/zz/button__hover.png", &[]),
            // Same JSON key `button-hover`.
            labelled("icons/zz/button-hover.png", &[]),
            labelled("icons/link__focus.png", &["icons/link__active.png"]),
        ];
        let mut left_out = remove_label_collisions(&mut images, Some("__"));
        left_out.sort();
        assert_eq!(
            paths(&images),
            [
                ("icons/button.png", vec![]),
                ("icons/button__hover.png", vec![]),
                ("icons/link__focus.png", vec!["icons/link__active.png"]),
            ]
        );
        assert_eq!(
            left_out,
            ["icons/zz/button-hover.png", "icons/zz/button__hover.png"]
        );
    }

    #[test]
    fn test_sprite_dimensions() {
        let config = Config::default();
        let mut sprite = Sprite {
            config: config.clone(),
            path: PathBuf::from("test"),
            ratios: vec![1.0],
            images: Vec::new(),
            margin: util::Spacing::default(),
            padding: util::Spacing::default(),
            failed_images: Vec::new(),
            layout_hash: Default::default(),
        };

        // Empty sprite should have zero dimensions
        assert_eq!(sprite.dimensions(), (0, 0));

        // Add an image at origin
        sprite.images.push(ImageAndPosition {
            path: String::from("test1.png"),
            image: DynamicImage::new_rgba8(50, 30),
            x: 0,
            y: 0,
            computed_width: 50,
            computed_height: 30,
            ..Default::default()
        });
        assert_eq!(sprite.dimensions(), (50, 30));

        // Add another image that extends the bounds
        sprite.images.push(ImageAndPosition {
            path: String::from("test2.png"),
            image: DynamicImage::new_rgba8(40, 40),
            x: 60,
            y: 20,
            computed_width: 40,
            computed_height: 40,
            ..Default::default()
        });
        // Width should be 60 + 40 = 100, Height should be 20 + 40 = 60
        assert_eq!(sprite.dimensions(), (100, 60));
    }

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
    fn test_add_function() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(0, 0), 0);
        assert_eq!(add(100, 200), 300);
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
    fn test_sprite_name_never_panics() {
        assert_eq!(sprite_name(Path::new("icons")), "icons");
        assert_eq!(sprite_name(Path::new("assets/icons/")), "icons");
        // `.` and `..` have no final component: resolved through the real path.
        let cwd = std::env::current_dir().unwrap();
        let cwd_name = cwd.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(sprite_name(Path::new(".")), cwd_name);
        let parent_name = cwd
            .parent()
            .and_then(Path::file_name)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(sprite_name(Path::new("..")), parent_name);
        assert_eq!(sprite_name(Path::new("/")), "sprite");
    }

    #[test]
    fn test_count_visible_pixels_matches_brute_force() {
        let brute_force = |image: &DynamicImage| {
            image
                .to_rgba8()
                .pixels()
                .filter(|pixel| pixel.0[3] > 0)
                .count() as u64
        };
        let rgba = image::RgbaImage::from_fn(13, 7, |x, y| {
            image::Rgba([x as u8, y as u8, 0, if (x * y) % 3 == 0 { 0 } else { 200 }])
        });
        let luma_alpha = image::GrayAlphaImage::from_fn(9, 5, |x, y| {
            image::LumaA([7, if x == y { 0 } else { 255 }])
        });
        let rgba16 = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(6, 6, |x, _| {
            image::Rgba([0, 0, 0, if x < 2 { 0 } else { 65535 }])
        });
        for image in [
            DynamicImage::ImageRgba8(rgba),
            DynamicImage::ImageLumaA8(luma_alpha),
            DynamicImage::ImageRgba16(rgba16),
        ] {
            let total = image.width() as u64 * image.height() as u64;
            assert_eq!(count_visible_pixels(&image, total), brute_force(&image));
        }
        // Without alpha every pixel is visible.
        let rgb = DynamicImage::ImageRgb8(image::RgbImage::new(4, 3));
        assert_eq!(count_visible_pixels(&rgb, 12), 12);
    }

    #[test]
    fn test_list_sprite_names_ignores_malformed_metadata_names() {
        // Review 3.7: a `.spriters` file panicked with "byte range starts at 1 but ends at 0".
        let dir = tmp_dir("meta_names");
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

    #[test]
    fn test_get_max_color_type_promotes_to_highest() {
        let make_img_sprite = |images: Vec<DynamicImage>| Sprite {
            config: Config::default(),
            path: PathBuf::from("test"),
            ratios: vec![1.0],
            images: images
                .into_iter()
                .enumerate()
                .map(|(i, img)| ImageAndPosition {
                    path: format!("img{}.png", i),
                    image: img,
                    computed_width: 10,
                    computed_height: 10,
                    x: 0,
                    y: 0,
                    aliases: vec![],
                    source_hash: 0,
                })
                .collect(),
            margin: util::Spacing::default(),
            padding: util::Spacing::default(),
            failed_images: Vec::new(),
            layout_hash: Default::default(),
        };

        let s = make_img_sprite(vec![
            DynamicImage::new_luma8(10, 10),
            DynamicImage::new_rgb8(10, 10),
        ]);
        assert_eq!(s.get_max_color_type(), ColorType::Rgb8);

        let s2 = make_img_sprite(vec![
            DynamicImage::new_rgba8(10, 10),
            DynamicImage::new_rgb8(10, 10),
        ]);
        assert_eq!(s2.get_max_color_type(), ColorType::Rgba8);

        let s3 = make_img_sprite(vec![DynamicImage::new_luma8(10, 10)]);
        assert_eq!(s3.get_max_color_type(), ColorType::L8);
    }
}
