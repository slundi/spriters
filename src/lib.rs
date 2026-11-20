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

use image::{ColorType, DynamicImage, GenericImageView};
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
    /// Using the filename of the source images you can customize the pseudo class related to the images, so if you simply append `__hover` to the filename
    /// **spriters** will add `:hover` to the CSS class name.
    pub pseudo_class_separator: Option<String>,
    /// Usually both CSS and PNG files reside on different folders, e.g. css and img. If you want to choose an individual folder for each type of file you can
    /// use the -`-img=<dir> --css=<dir>` options together to customize where the output files will be created.
    ///
    /// `$ spriters source --img=images/compiled --css=css/compiled``
    pub css: Option<std::path::PathBuf>,
    /// While using `--css` you can use your own css template using `--css-template=<FILE>`.
    ///
    /// *Note: by default **spriters** will use it’s own internal css template, so this command is not required unless you want to super-customize **spriters**’s css output using
    /// your own template. You can find further documentation about how templates work in the templates documentation page.*
    ///
    /// `$ spriters source output --css-template=my_template.jinja`
    pub css_template: Option<std::path::PathBuf>,
    pub img: Option<std::path::PathBuf>,
    /// By using the flag `png8` the output image format will be `png8` instead of `png32`.
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
    /// using your own template. You can find further documentation about how templates work in the templates documentation page.
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
    /// Disallow duplicate sprite
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
    /// Explicit file list, populated from stdin when stdin is a pipe.
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
impl Config {
    /// Check mandatory arguments and some values
    pub fn check(&self) -> Result<(), lexopt::Error> {
        if self.files.is_empty() && !self.source.is_dir() {
            error!(
                "Source directory does not exist or is not accessible: {:?}",
                self.source
            );
            return Err(lexopt::Error::UnexpectedValue {
                option: "source".to_owned(),
                value: self.source.clone().into_os_string(),
            });
        }
        if !self.output.is_dir() {
            error!(
                "Output directory does not exist or is not accessible: {:?}",
                self.output
            );
            return Err(lexopt::Error::UnexpectedValue {
                option: "output".to_owned(),
                value: self.output.clone().into_os_string(),
            });
        }
        if let Some(mw) = self.max_width
            && mw == 0
        {
            error!("max-width must be greater than 0");
            return Err(lexopt::Error::UnexpectedValue {
                option: "max-width".to_owned(),
                value: "0".into(),
            });
        }
        if let Some(mh) = self.max_height
            && mh == 0
        {
            error!("max-height must be greater than 0");
            return Err(lexopt::Error::UnexpectedValue {
                option: "max-height".to_owned(),
                value: "0".into(),
            });
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
            source: std::env::current_dir().unwrap(),
            output: std::env::current_dir().unwrap(),
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
    pub image: DynamicImage,
    pub x: u32,
    pub y: u32,
    pub computed_width: u32,
    pub computed_height: u32,
    /// Paths of images with identical content that consolidate into this region.
    pub aliases: Vec<String>,
    /// XxHash64 of the source file bytes, used to detect pixel changes between runs.
    pub source_hash: u64,
}
impl ImageAndPosition {
    pub fn label(&self) -> String {
        slug::slugify(
            std::path::Path::new(&self.path)
                .file_stem()
                .unwrap()
                .to_string_lossy(),
        )
    }
}

#[derive(Debug, Default)]
pub struct Sprite {
    pub config: Config,
    pub path: PathBuf,
    pub ratios: Vec<f32>,
    pub images: Vec<ImageAndPosition>,
    pub margin: Spacing,
    pub padding: Spacing,
}
impl Sprite {
    pub fn new(config: &Config, path: PathBuf) -> Sprite {
        let mut result = Sprite {
            config: config.clone(),
            path,
            margin: Spacing::from(config.margin.clone()),
            padding: Spacing::from(config.padding.clone()),
            ..Default::default()
        };
        // ratios: list & sort them
        if let Some(ratios) = config.ratios.clone() {
            for value in ratios.split(',') {
                let ratio: f32 = value.trim().parse().expect("Invalid ratio: not a number");
                if !ratio.is_finite() || ratio <= 0.0 {
                    panic!(
                        "Invalid ratio '{}': must be a positive finite number",
                        value.trim()
                    );
                }
                result.ratios.push(ratio);
            }
            result.ratios.sort_by(|a, b| a.total_cmp(b));
        } else {
            result.ratios.push(1f32);
        }
        result.discover_images();
        result.process();
        result
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
    }
    /// Discover images: it return all valid images within a folder.
    ///
    /// All files not supported by [Image](https://crates.io/crates/image) or beginning with '.' will be ignored.
    ///
    /// If the folder doesn't contain any valid image it will exit with an error message.
    ///
    /// The list of images will be ordered using the desired ordering algorithm. The default is 'Maxside'.
    fn discover_images(&mut self) {
        use rayon::prelude::*;

        // Phase 1 (sequential): walk directory, deduplicate, collect (path, bytes) pairs.
        // Reading bytes once avoids a second file-read for hashing.
        // Duplicate images (same size + hash) are tracked as aliases of their primary;
        // only the primary is decoded and placed in the sprite.
        let mut seen: HashMap<(u64, u64), String> = HashMap::new();
        let mut alias_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut candidates: Vec<(String, Vec<u8>, u64)> = Vec::new();

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

        if self.config.files.is_empty() {
            // Directory-walk mode (default).
            let mut walker = WalkDir::new(std::path::Path::new(&self.config.source))
                .follow_links(self.config.follow_links);
            if !self.config.recursive {
                walker = walker.max_depth(1);
            }
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy();
                let ext = entry.path().extension();
                if name.starts_with('.') || ext.is_none() {
                    continue;
                }
                if exclude_patterns
                    .iter()
                    .any(|p| p.matches_with(&name, match_opts))
                {
                    debug!("Excluding: {}", name);
                    continue;
                }
                let ext = ext
                    .unwrap()
                    .to_ascii_lowercase()
                    .to_string_lossy()
                    .to_string();
                if !is_image_extension(&ext) {
                    continue;
                }
                let path = entry.path().to_string_lossy().to_string();
                match std::fs::read(entry.path()) {
                    Ok(bytes) => {
                        let size = bytes.len() as u64;
                        let hash = twox_hash::XxHash64::oneshot(713705, &bytes);
                        if let Some(primary) = seen.get(&(size, hash)) {
                            if self.config.no_duplicate {
                                error!("{} and {} are identical", path, primary);
                                std::process::exit(1);
                            }
                            warn!("{} and {} are identical — consolidating", path, primary);
                            alias_map.entry(primary.clone()).or_default().push(path);
                            continue;
                        }
                        seen.insert((size, hash), path.clone());
                        candidates.push((path, bytes, hash));
                    }
                    Err(e) => error!("Cannot read {:?}: {}", entry.path(), e),
                }
            }
        } else {
            // File-list mode: paths supplied explicitly (e.g. from stdin).
            for path_buf in &self.config.files {
                let name = match path_buf.file_name() {
                    Some(n) => n.to_string_lossy().into_owned(),
                    None => continue,
                };
                let ext = match path_buf.extension() {
                    Some(e) => e.to_ascii_lowercase().to_string_lossy().into_owned(),
                    None => continue,
                };
                if name.starts_with('.') || !is_image_extension(&ext) {
                    continue;
                }
                if exclude_patterns
                    .iter()
                    .any(|p| p.matches_with(&name, match_opts))
                {
                    debug!("Excluding: {}", name);
                    continue;
                }
                let path = path_buf.to_string_lossy().into_owned();
                match std::fs::read(path_buf) {
                    Ok(bytes) => {
                        let size = bytes.len() as u64;
                        let hash = twox_hash::XxHash64::oneshot(713705, &bytes);
                        if let Some(primary) = seen.get(&(size, hash)) {
                            if self.config.no_duplicate {
                                error!("{} and {} are identical", path, primary);
                                std::process::exit(1);
                            }
                            warn!("{} and {} are identical — consolidating", path, primary);
                            alias_map.entry(primary.clone()).or_default().push(path);
                            continue;
                        }
                        seen.insert((size, hash), path.clone());
                        candidates.push((path, bytes, hash));
                    }
                    Err(e) => error!("Cannot read {:?}: {}", path_buf, e),
                }
            }
        }

        // Phase 2 (parallel): decode, resize, then crop each image independently.
        // Resize happens before crop so that max-width/height apply to the full image
        // (including any transparent border), and crop removes the leftover padding after.
        let config = &self.config;
        // The highest ratio represents the source resolution (e.g., 2.0 for --ratios=2,1).
        // max-width/height are specified in 1× units; scale up accordingly.
        let display_ratio = self.ratios.last().copied().unwrap_or(1.0);
        self.images = candidates
            .into_par_iter()
            .filter_map(|(path, bytes, source_hash)| {
                let is_svg = path.to_lowercase().ends_with(".svg");

                // SVG: rasterized directly to the target size (no post-load resize needed).
                // Raster: loaded then resized if max-width/height requires it.
                let load_result: Result<DynamicImage, String> = if is_svg {
                    rasterize_svg(&bytes, display_ratio, config.max_width, config.max_height)
                } else {
                    image::load_from_memory(&bytes).map_err(|e| e.to_string())
                };

                match load_result {
                    Ok(img) => {
                        let original = (img.width(), img.height());
                        let mut img2 = if is_svg {
                            img // SVG already rasterized at target size
                        } else {
                            // 1. Resize (ratio-aware)
                            let d = get_dimensions_from_max(
                                img.width(),
                                img.height(),
                                config.max_width,
                                config.max_height,
                                display_ratio,
                            );
                            if d.0 != img.width() || d.1 != img.height() {
                                img.resize_exact(d.0, d.1, config.resize_algorithm)
                            } else {
                                img
                            }
                        };
                        // 2. Crop after resize (applies to both SVG and raster)
                        if config.crop {
                            img2 = crop(img2, None);
                        }
                        let (final_w, final_h) = (img2.width(), img2.height());
                        let aliases = alias_map.get(&path).cloned().unwrap_or_default();
                        info!(
                            "\t Adding: {} ({}x{} -> {}x{}){}",
                            path,
                            original.0,
                            original.1,
                            final_w,
                            final_h,
                            if aliases.is_empty() {
                                String::new()
                            } else {
                                format!(" [aliases: {}]", aliases.len())
                            }
                        );
                        Some(ImageAndPosition {
                            path,
                            image: img2,
                            computed_width: final_w,
                            computed_height: final_h,
                            aliases,
                            source_hash,
                            ..Default::default()
                        })
                    }
                    Err(err) => {
                        error!("Cannot open image {}: {}", path, err);
                        None
                    }
                }
            })
            .collect();

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
        let ratio = self.ratios[0];

        self.images.sort_by(|a, b| {
            let primary = match self.config.ordering {
                Ordering::Maxside | Ordering::MaxsideReverse => {
                    let b_max = absolute_length(b.image.height(), vspace, ratio)
                        .max(absolute_length(b.image.width(), hspace, ratio));
                    let a_max = absolute_length(a.image.height(), vspace, ratio)
                        .max(absolute_length(a.image.width(), hspace, ratio));
                    b_max.cmp(&a_max)
                }
                Ordering::Height | Ordering::HeightReverse => {
                    let b_h = absolute_length(b.image.height(), vspace, ratio);
                    let a_h = absolute_length(a.image.height(), vspace, ratio);
                    b_h.cmp(&a_h)
                }
                Ordering::Width | Ordering::WidthReverse => {
                    let b_w = absolute_length(b.image.width(), hspace, ratio);
                    let a_w = absolute_length(a.image.width(), hspace, ratio);
                    b_w.cmp(&a_w)
                }
                Ordering::Area | Ordering::AreaReverse => {
                    let b_area = absolute_length(b.image.width(), hspace, ratio)
                        * absolute_length(b.image.height(), vspace, ratio);
                    let a_area = absolute_length(a.image.width(), hspace, ratio)
                        * absolute_length(a.image.height(), vspace, ratio);
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

    /// Calculate the sprite dimensions based on positioned images.
    pub fn dimensions(&self) -> (u32, u32) {
        let mut width: u32 = 0;
        let mut height: u32 = 0;
        for img in self.images.iter() {
            width = width.max(img.x + img.computed_width);
            height = height.max(img.y + img.computed_height);
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
}

impl Manager {
    pub fn find_sprites(&mut self) {
        use rayon::prelude::*;

        if self.project {
            // Collect subdirectory paths first (sequential — read_dir is not parallel).
            let paths: Vec<PathBuf> = std::fs::read_dir(&self.config.source)
                .expect("Source does not exist or permissions are insufficient or it is a file")
                .filter_map(|e| e.ok())
                .filter_map(|de| {
                    let p = de.path();
                    if !p.is_dir() || (p.is_symlink() && !self.config.follow_links) {
                        return None;
                    }
                    if p.file_name()?.to_string_lossy().starts_with('.') {
                        return None;
                    }
                    Some(p)
                })
                .collect();

            // Build each sprite concurrently (discover_images is the heavy work).
            // Each sprite must have its own config.source pointing to its subdirectory;
            // discover_images walks config.source, not sprite.path.
            let config = &self.config;
            self.sprites = paths
                .into_par_iter()
                .map(|p| {
                    info!(
                        "Creating sprite: {}",
                        p.file_stem().unwrap().to_string_lossy()
                    );
                    let mut sprite_config = config.clone();
                    sprite_config.source = p.clone();
                    Sprite::new(&sprite_config, p)
                })
                .collect();
        } else {
            let p = PathBuf::from(&self.config.source);
            info!(
                "Creating sprite: {}",
                p.file_stem().unwrap().to_string_lossy()
            );
            self.sprites.push(Sprite::new(&self.config, p));
        }
        if self.sprites.is_empty() {
            error!("Input folder does not have images.");
        }
    }

    /// Save all sprites inside this manager.
    pub fn save(&self) {
        use rayon::prelude::*;
        self.sprites
            .par_iter()
            .for_each(|sprite| crate::output::generate_files(&self.config, sprite));
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

/// Load stats for a named sprite in `output_dir`.
/// Uses the image file recorded in the sprite metadata (which accounts for cache-busted
/// file names); falls back to `.png`, `.webp`, `.avif` in order for older metadata.
/// Returns `None` if no image is found.
pub fn sprite_stats(output_dir: &Path, name: &str) -> Option<SpriteStats> {
    let recorded = output::read_metadata_file(&output_dir.join(output::meta_file_name(name)))
        .image_file
        // A bare file name only: never follow a path out of the output directory.
        .filter(|file| Path::new(file).file_name() == Some(std::ffi::OsStr::new(file)))
        .map(|file| output_dir.join(file))
        .filter(|path| path.exists());
    let img_path = recorded.or_else(|| {
        ["png", "webp", "avif"]
            .iter()
            .map(|ext| output_dir.join(format!("{}.{}", name, ext)))
            .find(|p| p.exists())
    })?;

    let file_size = std::fs::metadata(&img_path).ok()?.len();
    let img = image::open(&img_path).ok()?;
    let (w, h) = (img.width(), img.height());
    let total = w as u64 * h as u64;
    let used = img.to_rgba8().pixels().filter(|p| p.0[3] > 0).count() as u64;

    Some(SpriteStats {
        name: name.to_owned(),
        width: w,
        height: h,
        file_size,
        used_pixels: used,
        total_pixels: total,
    })
}

/// Scan `output_dir` for `.*.spriters` metadata files and return the sprite names found.
pub fn list_sprite_names(output_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(output_dir) else {
        return vec![];
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            (n.starts_with('.') && n.ends_with(".spriters"))
                .then(|| n[1..n.len() - ".spriters".len()].to_owned())
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
    let (target_w, target_h) =
        get_dimensions_from_max(render_w, render_h, max_width, max_height, display_ratio);

    if target_w == 0 || target_h == 0 {
        return Err("SVG target dimensions are zero after constraints".to_string());
    }

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or("Failed to create pixmap")?;

    let scale_x = target_w as f32 / natural_w as f32;
    let scale_y = target_h as f32 / natural_h as f32;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    // Encode to PNG so the image crate can decode it with straight alpha.
    let png = pixmap
        .encode_png()
        .map_err(|e| format!("PNG encode error: {e}"))?;
    image::load_from_memory(&png).map_err(|e| format!("Image load error: {e}"))
}

/// Crop an image. When no color is provided, it crops white or transparent areas (when alpha channel is available).
pub fn crop(image: DynamicImage, color: Option<u32>) -> DynamicImage {
    let mut cropable_color: u32 = 0xffffffff; // default is white
    let mut crop_alpha = false;
    if let Some(c) = color {
        cropable_color = c;
    } else {
        crop_alpha = image.color().has_alpha();
    }

    let mut left: u32 = 0;
    let mut right: u32 = 0;
    let mut top: u32 = 0;
    let mut bottom: u32 = 0;

    // detect left/right
    let mut stop = false;
    if image.width() > 1 {
        // left
        for x in 0..image.width() {
            for y in 0..image.height() {
                let pixel = image.get_pixel(x, y);
                if !is_color_or_transparent(pixel, crop_alpha, cropable_color) {
                    stop = true;
                    break;
                }
            }
            if stop {
                stop = false;
                break;
            }
            left += 1;
        }
        // right
        for x in (0..image.width()).rev() {
            for y in 0..image.height() {
                let pixel = image.get_pixel(x, y);
                if !is_color_or_transparent(pixel, crop_alpha, cropable_color) {
                    stop = true;
                    break;
                }
            }
            if stop {
                stop = false;
                break;
            }
            right += 1;
        }
    }
    // detect top/bottom
    if image.height() > 1 {
        // top
        for y in 0..image.height() {
            for x in 0..image.width() {
                let pixel = image.get_pixel(x, y);
                if !is_color_or_transparent(pixel, crop_alpha, cropable_color) {
                    stop = true;
                    break;
                }
            }
            if stop {
                stop = false;
                break;
            }
            top += 1;
        }
        // bottom
        for y in 0..image.height() {
            for x in (0..image.width()).rev() {
                let pixel = image.get_pixel(x, y);
                if !is_color_or_transparent(pixel, crop_alpha, cropable_color) {
                    stop = true;
                    break;
                }
            }
            if stop {
                break;
            }
            bottom += 1;
        }
    }
    debug!(
        "\t\t{}x{}, left={}, top={}, right={}, bottom={} => {}x{}",
        image.width(),
        image.height(),
        left,
        top,
        right,
        bottom,
        image.width() - left - right,
        image.height() - top - bottom,
    );
    image.crop_imm(
        left,
        top,
        image.width() - left - right,
        image.height() - top - bottom,
    )
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
fn get_dimensions_from_max(
    width: u32,
    height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
    display_ratio: f32,
) -> (u32, u32) {
    let r = display_ratio as f64;
    let mut dims = (width as f64, height as f64);
    if let Some(mw) = max_width {
        let effective = (mw as f64 * r) as u32;
        if effective < width {
            let scale = effective as f64 / width as f64;
            dims.0 = effective as f64;
            dims.1 = height as f64 * scale;
        }
    }
    if let Some(mh) = max_height {
        let effective = mh as f64 * r;
        if effective < dims.1 {
            let scale = effective / dims.1;
            dims.1 = effective;
            dims.0 *= scale;
        }
    }
    (dims.0 as u32, dims.1 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            get_dimensions_from_max(10, 20, Some(5), Some(5), 1.0),
            (2, 5)
        );
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

        // crop transparency
        let mut img2 = img.clone();
        img2 = crop(img2, None);
        assert_eq!(img2.width(), 100);
        assert_eq!(img2.height(), 74);

        // crop white... will be same size
        let mut img2 = img.clone();
        img2 = crop(img2, Some(0xffffffff));
        assert_eq!(img2.width(), 120);
        assert_eq!(img2.height(), 100);
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
        let d = tmp_dir("chk_ok");
        let mut c = Config::default();
        c.source = d.clone();
        c.output = d.clone();
        assert!(c.check().is_ok());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn test_config_check_output_not_dir() {
        let d = tmp_dir("chk_out");
        let mut c = Config::default();
        c.source = d.clone();
        c.output = PathBuf::from("/nonexistent/__spriters_test__");
        let err = c.check();
        std::fs::remove_dir_all(&d).ok();
        assert!(err.is_err());
    }

    #[test]
    fn test_config_check_source_not_dir() {
        let d = tmp_dir("chk_src");
        let mut c = Config::default();
        c.source = PathBuf::from("/nonexistent/__spriters_test__");
        c.output = d.clone();
        let err = c.check();
        std::fs::remove_dir_all(&d).ok();
        assert!(err.is_err());
    }

    #[test]
    fn test_config_check_max_width_zero() {
        let d = tmp_dir("chk_mw0");
        let mut c = Config::default();
        c.source = d.clone();
        c.output = d.clone();
        c.max_width = Some(0);
        let err = c.check();
        std::fs::remove_dir_all(&d).ok();
        assert!(err.is_err());
    }

    #[test]
    fn test_config_check_max_height_zero() {
        let d = tmp_dir("chk_mh0");
        let mut c = Config::default();
        c.source = d.clone();
        c.output = d.clone();
        c.max_height = Some(0);
        let err = c.check();
        std::fs::remove_dir_all(&d).ok();
        assert!(err.is_err());
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
