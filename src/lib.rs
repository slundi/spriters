use std::path::PathBuf;

use image::DynamicImage;
use walkdir::WalkDir;

pub mod algorithm;
pub mod util;

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

#[derive(Debug, Clone, Copy, Default)]
#[repr(u8)]
pub enum Ordering {
    #[default]
    Maxside,
    Width,
    Height,
    Area,
    Filename,
}

#[derive(Debug, Clone, Default)]
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
            eprintln!(
                "Source directory does not exist or is not accessible: {:?}",
                self.source
            );
            return Err(lexopt::Error::UnexpectedValue {
                option: "source".to_owned(),
                value: self.source.clone().into_os_string(),
            });
        }
        if !self.output.is_dir() {
            eprintln!(
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

#[derive(Debug, Default)]
pub struct Sprite {
    pub config: Config,
    pub path: PathBuf,
    pub ratios: Vec<f32>,
    pub images: Vec<DynamicImage>,
}
impl Sprite {
    pub fn new(config: Config, path: PathBuf) -> Sprite {
        let mut result = Sprite {
            config: config.clone(),
            path,
            ..Default::default()
        };
        // ratios
        if let Some(ratios) = config.ratios {
            for value in ratios.split(',') {
                result
                    .ratios
                    .push(value.trim().parse::<f32>().expect("Invalid ratio"));
            }
        } else {
            result.ratios.push(1f32);
        }
        result.discover_images();
        result.process();
        result
    }

    /// Generate sprite map
    fn process(&self) {
        match self.config.algorithm {
            Algorithm::Square => todo!(),
            Algorithm::Vertical => todo!(),
            Algorithm::Hortizontal => todo!(),
            Algorithm::Diagonal => todo!(),
            Algorithm::VerticalRight => todo!(),
            Algorithm::HorizontalBottom => todo!(),
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
        let mut images: Vec<DynamicImage> = Vec::new();
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
                    Ok(img) => images.push(img),
                    Err(err) => eprintln!("Cannot open image: {}", err),
                };
            } // else { continue; }
        }
        if images.is_empty() {
            eprintln!("No images were found in the source directory");
        }
        images.sort_by(|a, b| match self.config.algorithm {
            Algorithm::Square => todo!(),
            Algorithm::Vertical => todo!(),
            Algorithm::Hortizontal => todo!(),
            Algorithm::Diagonal => todo!(),
            Algorithm::VerticalRight => todo!(),
            Algorithm::HorizontalBottom => todo!(),
        });
    }
}

#[derive(Debug, Default)]
pub struct Manager {
    pub project: bool,
    pub sprites: Vec<Sprite>,
}

impl Manager {
    fn find_sprites(&mut self, config: &Config) {
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
                        self.sprites.push(Sprite::new(config.clone(), p));
                    }
                    Err(_) => continue,
                }
            }
        } else {
            self.sprites
                .push(Sprite::new(config.clone(), PathBuf::from(&config.source)));
        }
    }
    /// Validate all sprites inside this manager.
    fn validate(&self) {
        todo!();
    }
    /// Save all sprites inside this manager.
    fn save(&self) {
        todo!()
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
