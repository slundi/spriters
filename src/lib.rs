use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use image::{ColorType, DynamicImage, GenericImageView};
use log::*;
use util::{absolute_length, horizontal_spacing, vertical_spacing, Spacing};
use walkdir::WalkDir;

pub mod algorithm;
mod algorithm_square;
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

/// Used to order the images before adding them to the canvas can be tunned. By default the algorithm is `Square``,
/// but in some situations using another ordering like vertical or horizontal could be useful depending on the kind of images you are spriting.
#[derive(Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum Algorithm {
    /// The square algorithm was inspired by the Binary Tree Bin Packing Algorithm Article by Jake Gordon.
    #[default]
    Square,
    /// The vertical one allocates the images vertically aligning them to the left of the sprite.
    Vertical,
    /// The vertical-right one allocates the images vertically aligning them to the right of the sprite.
    Hortizontal,
    /// The horizontal one allocates the images aligning them to the top of the sprite.
    Diagonal,
    /// The horizontal-bottom one allocates the images aligning them to the bottom of the sprite.
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
    /// If the width of the image is gratear than the given value, the image will be resized before making the sprite
    pub max_width: Option<u32>,
    /// If the height of the image is gratear than the given value, the image will be resized before making the sprite
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
    /// This option has the same purpose than `--cachebuster `but insted of using the hash of the PNG as a queryarg it uses it as part of the filename.
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
    /// Unlike `--cachebuster-filename`, **sprinters** will only apply filename cachebusting to the sprite image and not to both the CSS and the sprite image.
    pub cachebuster_filename_only_sprites: bool,
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
    pub separator: String, // TODO: impl Default
    /// This flag will make **spriters** suppress all console output.
    pub quiet: bool,
    /// Read directories recursively and add all the images to the same sprite. Example structure:
    /// ```
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
    /// While you are developing a site it could be quite frustrating running **spriters** once and another every time you change a source image or a filename.
    /// `--watch` will allow you to keep **spriters** running in the background and it’ll rebuild the sprite every time it detects changes on the source directory.
    pub watch: bool,
    // output format
    /// Using the --caat option, **Spriters** will generate both a sprite image and a caat metadata file.
    pub caat: bool,
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
    /// ```
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

    // lexopt example
    pub thing: String,
    pub number: u32,
    pub shout: bool,
}
impl Config {
    /// Check mandatory arguments and some values
    pub fn check(&self) -> Result<(), lexopt::Error> {
        if !self.source.is_dir() {
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
        Ok(())
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            resize_algorithm: image::imageops::FilterType::CatmullRom,
            ..Default::default()
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
            ..Default::default()
        };
        // ratios: list & sort them
        if let Some(ratios) = config.ratios.clone() {
            for value in ratios.split(',') {
                result
                    .ratios
                    .push(value.trim().parse::<f32>().expect("Invalid ratio"));
            }
            result.ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
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
            Algorithm::Hortizontal => Algorithm::horizontal(self),
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
    fn discover_images(&self) {
        let mut images: Vec<ImageAndPosition> = Vec::new();
        for entry in WalkDir::new(std::path::Path::new(&self.config.source))
            .follow_links(self.config.follow_links)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let name = entry.file_name().to_string_lossy();
            let ext = entry.path().extension();
            if name.starts_with('.') || ext.is_none() {
                continue;
            }
            let ext = String::from(ext.unwrap().to_ascii_lowercase().to_string_lossy());
            if matches!(
                ext.as_str(),
                "jpg"
                    | "jpeg"
                    | "jpe .jif"
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
            ) {
                match image::open(entry.path()) {
                    // Perform pre-processing operations when applicable
                    // 1. crop
                    // 2. resize depending on max-width and max-height
                    // pub fn pre_process(&self, config: &Config) {
                    //     for s in self.sprites.iter() {
                    //         for i in s.images.iter() {
                    //             if config.crop {
                    //                 crop(&mut i.image, None);
                    //             }
                    //             if let Some(mw) = config.max_width {
                    //                 todo!();
                    //             }
                    //             if let Some(mh) = config.max_height {
                    //                 todo!();
                    //             }
                    //         }
                    //     }
                    // }
                    Ok(img) => {
                        let mut img2 = img.clone();
                        if self.config.crop {
                            crop(&mut img2, None);
                        }
                        let d = get_dimensions_from_max(
                            img2.width(),
                            img2.height(),
                            self.config.max_width,
                            self.config.max_height,
                        );
                        images.push(ImageAndPosition {
                            image: img,
                            computed_width: d.0,
                            computed_height: d.1,
                            ..Default::default()
                        })
                    }
                    Err(err) => error!("Cannot open image: {}", err),
                };
            } // else { continue; }
        }
        if images.is_empty() {
            error!("No images were found in the source directory");
        }
        self.sort_images(&mut images);
    }

    /// Sort images depending on the `config.ordering` value.
    fn sort_images(&self, images: &mut [ImageAndPosition]) {
        images.sort_by(|a, b| match self.config.ordering {
            Ordering::Maxside | Ordering::MaxsideReverse => absolute_length(
                b.image.height(),
                vertical_spacing(
                    self.padding.top,
                    self.padding.bottom,
                    self.margin.top,
                    self.margin.bottom,
                ),
                self.ratios[0],
            )
            .max(absolute_length(
                b.image.width(),
                horizontal_spacing(
                    self.padding.left,
                    self.padding.right,
                    self.margin.left,
                    self.margin.right,
                ),
                self.ratios[0],
            ))
            .cmp(
                &absolute_length(
                    a.image.height(),
                    vertical_spacing(
                        self.padding.top,
                        self.padding.bottom,
                        self.margin.top,
                        self.margin.bottom,
                    ),
                    self.ratios[0],
                )
                .max(absolute_length(
                    a.image.width(),
                    horizontal_spacing(
                        self.padding.left,
                        self.padding.right,
                        self.margin.left,
                        self.margin.right,
                    ),
                    self.ratios[0],
                )),
            ),
            Ordering::Height | Ordering::HeightReverse => absolute_length(
                b.image.height(),
                vertical_spacing(
                    self.padding.top,
                    self.padding.bottom,
                    self.margin.top,
                    self.margin.bottom,
                ),
                self.ratios[0],
            )
            .cmp(&absolute_length(
                a.image.height(),
                vertical_spacing(
                    self.padding.top,
                    self.padding.bottom,
                    self.margin.top,
                    self.margin.bottom,
                ),
                self.ratios[0],
            )),
            Ordering::Width | Ordering::WidthReverse => absolute_length(
                b.image.width(),
                horizontal_spacing(
                    self.padding.left,
                    self.padding.right,
                    self.margin.left,
                    self.margin.right,
                ),
                self.ratios[0],
            )
            .cmp(&absolute_length(
                a.image.width(),
                horizontal_spacing(
                    self.padding.left,
                    self.padding.right,
                    self.margin.left,
                    self.margin.right,
                ),
                self.ratios[0],
            )),
            Ordering::Area | Ordering::AreaReverse => (absolute_length(
                b.image.width(),
                horizontal_spacing(
                    self.padding.left,
                    self.padding.right,
                    self.margin.left,
                    self.margin.right,
                ),
                self.ratios[0],
            ) * absolute_length(
                b.image.height(),
                vertical_spacing(
                    self.padding.top,
                    self.padding.bottom,
                    self.margin.top,
                    self.margin.bottom,
                ),
                self.ratios[0],
            ))
            .cmp(
                &(absolute_length(
                    a.image.width(),
                    horizontal_spacing(
                        self.padding.left,
                        self.padding.right,
                        self.margin.left,
                        self.margin.right,
                    ),
                    self.ratios[0],
                ) * absolute_length(
                    a.image.height(),
                    vertical_spacing(
                        self.padding.top,
                        self.padding.bottom,
                        self.margin.top,
                        self.margin.bottom,
                    ),
                    self.ratios[0],
                )),
            ),
            Ordering::Filename | Ordering::FilenameReverse => Path::new(&b.path)
                .file_name()
                .unwrap()
                .cmp(Path::new(&a.path).file_name().unwrap()),
        });
        if self.config.ordering.is_reverse() {
            images.reverse();
        }
    }

    fn get_max_color_type(&self) -> ColorType {
        let mut current = ColorType::L8;
        for i in self.images.iter() {
            current = get_greatet_quality(&current, &i.image.color());
        }
        current
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

pub fn get_greatet_quality(a: &ColorType, b: &ColorType) -> ColorType {
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
    pub sprites: Vec<Sprite>,
}

impl Manager {
    pub fn find_sprites(&mut self, config: &Config) {
        if self.project {
            for entry in std::fs::read_dir(std::path::Path::new(&config.source.clone()))
                .expect("Source does not exists or permissions are insufficient or it is a file")
            {
                match entry {
                    Ok(de) => {
                        // Only process folders and ignore filenames starting with '.'
                        // Ignore symlinks if necessary
                        let p = de.path();
                        if !p.is_dir() || (p.is_symlink() && !config.follow_links) {
                            continue;
                        }
                        if let Some(name) = p.file_name() {
                            if name.to_string_lossy().starts_with('.') {
                                continue;
                            }
                        }
                        self.sprites.push(Sprite::new(config, p));
                    }
                    Err(_) => continue,
                }
            }
        } else {
            self.sprites
                .push(Sprite::new(config, PathBuf::from(&config.source)));
        }
        if self.sprites.is_empty() {
            log::error!("Input folder does not have images.");
        }
    }
    /// Validate all sprites inside this manager.
    fn validate(&self) {
        todo!();
    }
    /// Save all sprites inside this manager.
    fn save(&self) {
        // TODO: pre-processing (crop, resize, ...): async/thread pool
        // TODO: process: build image(s): async/thread pool in project mode?
        // TODO: generate description file (CSS, ...): async/thread pool for each format?
        todo!()
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

/// Crop an image. When no color is provided, it crops white or transparent areas (when alpha channel is available).
pub fn crop(image: &mut DynamicImage, color: Option<u32>) {
    let mut croppable_color: u32 = 0xffffff00; // default is white
    let mut crop_alpha = false;
    if let Some(c) = color {
        croppable_color = c;
    } else {
        crop_alpha = image.color().has_alpha();
    }
    let mut x_matrix: Vec<bool> = vec![true; image.width() as usize];
    let mut y_matrix: Vec<bool> = vec![true; image.height() as usize];
    // detect top/bottom
    for x in 0..image.width() {
        if image.height() == 1 {
            continue;
        }
        for y in 0..image.height() - 1 {
            let pixel = image.get_pixel(x, y);
            if pixel != image.get_pixel(x, y - 1)
                || !is_color_or_transparent(&pixel, crop_alpha, croppable_color)
            {
                y_matrix[y as usize] = false;
                break;
            }
        }
    }
    // detect left/right
    for y in 0..image.height() {
        if image.width() == 1 {
            continue;
        }
        for x in 0..image.width() - 1 {
            let pixel = image.get_pixel(x, y);
            if pixel != image.get_pixel(x - 1, y)
                || !is_color_or_transparent(&pixel, crop_alpha, croppable_color)
            {
                x_matrix[x as usize] = false;
                break;
            }
        }
    }
    // find croppable areas
    let mut top: u32 = 0;
    let mut bottom: u32 = 0;
    let mut left: u32 = 0;
    let mut right: u32 = 0;
    for i in 0..x_matrix.len() - 1 {
        if x_matrix[i] && x_matrix[i] == x_matrix[i + 1] {
            left += 1;
        } else {
            break;
        }
    }
    for i in x_matrix.len()..1 {
        if x_matrix[i] && x_matrix[i] == x_matrix[i - 1] {
            right += 1;
        } else {
            break;
        }
    }
    for i in 0..y_matrix.len() - 1 {
        if y_matrix[i] && y_matrix[i] == y_matrix[i + 1] {
            top += 1;
        } else {
            break;
        }
    }
    for i in y_matrix.len()..1 {
        if y_matrix[i] && y_matrix[i] == y_matrix[i - 1] {
            bottom += 1;
        } else {
            break;
        }
    }
    image.crop_imm(
        left,
        top,
        image.width() - left - right,
        image.height() - top - bottom,
    );
}

fn is_color_or_transparent(pixel: &image::Rgba<u8>, crop_alpha: bool, color: u32) -> bool {
    if crop_alpha {
        if pixel.0[3] < 255 {
            return false;
        }
        return true;
    }
    u32::from_be_bytes(pixel.0) == color
}

/// Get the `(width, height)` tuple after the computation of the new image size depending on max width an height.
fn get_dimensions_from_max(
    width: u32,
    height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> (u32, u32) {
    let mut dimensions = (width as f64, height as f64);
    let mut ratio = 1f64;
    if let Some(mw) = max_width {
        if mw < width {
            ratio = (mw as f64) / (width as f64);
            dimensions.0 = mw as f64;
            dimensions.1 = (height as f64) * ratio;
        }
    }
    if let Some(mh) = max_height {
        let mhf64 = mh as f64;
        if mhf64 < dimensions.1 {
            ratio = mhf64 / dimensions.1;
            dimensions.1 = mhf64;
            dimensions.0 *= ratio;
        }
    }
    (dimensions.0 as u32, dimensions.1 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_croppable_color() {
        // not croppable color
        assert!(!is_color_or_transparent(
            &image::Rgba::from([128u8, 128, 128, 0]),
            false,
            0xffffff00
        ));
        // transparent but not set
        assert!(!is_color_or_transparent(
            &image::Rgba::from([128u8, 128, 128, 255]),
            false,
            0xffffff00
        ));
        // transparent cropable
        assert!(is_color_or_transparent(
            &image::Rgba::from([128u8, 128, 128, 255]),
            true,
            0xffffff00
        ));
        // croppable color no alpha
        assert!(is_color_or_transparent(
            &image::Rgba::from([255u8, 255, 255, 0]),
            false,
            0xffffff00
        ));
    }

    #[test]
    fn test_greatest_quality() {
        assert_eq!(
            get_greatet_quality(&ColorType::L8, &ColorType::Rgb16),
            ColorType::Rgb16
        );
        assert_eq!(
            get_greatet_quality(&ColorType::La8, &ColorType::La8),
            ColorType::La8
        );
        assert_eq!(
            get_greatet_quality(&ColorType::Rgba32F, &ColorType::Rgb16),
            ColorType::Rgba32F
        );
    }

    #[test]
    fn test_max_dimensions() {
        assert_eq!(get_dimensions_from_max(10, 20, None, None), (10, 20));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), None), (5, 10));
        assert_eq!(get_dimensions_from_max(10, 20, Some(15), None), (10, 20));
        assert_eq!(get_dimensions_from_max(10, 20, None, Some(10)), (5, 10));
        assert_eq!(get_dimensions_from_max(10, 20, None, Some(40)), (10, 20));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), Some(10)), (5, 10));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), Some(15)), (5, 10));
        assert_eq!(get_dimensions_from_max(10, 20, Some(10), Some(10)), (5, 10));
        assert_eq!(get_dimensions_from_max(10, 20, Some(2), Some(10)), (2, 4));
        assert_eq!(get_dimensions_from_max(10, 20, Some(5), Some(5)), (2, 5));
    }
}
