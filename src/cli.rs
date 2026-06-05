use image::imageops::FilterType;
use spriters::{Algorithm, Config, JsonFormat, Ordering, SpriteFormat};

const HELP_MSG: &str = "spriters [OPTIONS] [SOURCE_DIR] [OUTPUT_DIR]

By default, it search images on the current working directory. The output is alson on the working directory.

General options:
-h, --help:             print this message
-q, --quiet:            don't print anything on the console
-i, --input:            input directory
-o, --output:           output directory
-r, --recursive:        read directories recursively and add all the images to the same sprite
--project:              generate a sprite per working directory sub folder
--watch:                watch working directory for changes (run in background)
--follow-links:         follow symbolic links
--force:                force regeneration even if sprite is up to date

Output options:
--html:                 generate an HTML page
--json:                 generate JSON
--json-format:          JSON output format: array (default) or hash
--caat:                 generate CAAT (Canvas Advanced Animation Toolkit) JSON descriptor
--cocos2d:              generate Cocos2d XML compatible files
-l, --less:             generate LESS dynamic  stylesheet
--scss:                 generate SCSS dynamic stylesheet
--css-template:         custom Jinja template for CSS output
--less-template:        custom Jinja template for LESS output
--scss-template:        custom Jinja template for SCSS output
--css-vars:             generate a .vars.css file with CSS custom properties (--sprite-name-x, -y, -w, -h)
--css-vars-template:    custom template file for CSS vars output
--no-css:               do not generate CSS files
--no-img:               do not generate image file
--format:               output image format: png (default), webp, avif
--css:                  custom output directory for CSS files
--img:                  custom output directory for image files
--cachebuster:          when generating CSS, add image hash in the end of the filename
--cachebuster-filename: when generating CSS, add image hash before the extension in the filename
--cachebuster-filename-only-sprites: embed hash in sprite image filename but keep CSS filename unchanged
--separator:            separator for CSS class names (default: '-')
--namespace:            global namespace for CSS classes (default: 'sprite')
--sprite-namespace:     sprite-specific namespace, use %(sprite)s for sprite name
--pseudo-class-separator: separator in filenames for pseudo-classes (e.g., '__' makes button__hover.png use :hover)
-u, --url:              base URL for sprite images in CSS (e.g., https://cdn.example.com/sprites)

Image processing options:
-a, --algorithm:        algorithm to order images in the sprite: square (default), vertical, horizontal, diagonal, vertical-right, horizontal-bottom
--ordering:             sort order for images: maxside (default), width, height, area, filename. Prefix with '-' for reverse (e.g., -maxside)
--png8:                 generate grascale image(s)
--ratios:               generate sprites at multiple ratios for different DPIs (e.g., '2,1' or '2,1.5,1')
--retina:               shortcut for --ratios=2,1
-c, --crop:             crop unnecessary transparent spaces
--exclude:              skip files whose name matches a glob pattern (repeatable, e.g. --exclude '_draft_*' --exclude '*.tmp')
--max-width:            image max width in the sprite
--max-height:           image max height in the sprite
--resize-algorithm:     resize algorithm are nearesr, triangle, gaussian, lanczos3, catmullrom (default)
-p, --padding:          padding around each image (included in dimensions). Formats: 10, '5 10', '1 2 3 4'
-m, --margin:           margin around each image (not included in dimensions). Formats: 10, '5 10', '1 2 3 4'";

pub(crate) fn parse_args() -> Result<Config, lexopt::Error> {
    use lexopt::prelude::*;

    let mut args: Config = Config::default();

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('a') | Long("algorithm") => {
                let algo: String = parser.value()?.parse()?;
                match algo.to_lowercase().as_str() {
                    "square" => args.algorithm = Algorithm::Square,
                    "vertical" => args.algorithm = Algorithm::Vertical,
                    "horizontal" => args.algorithm = Algorithm::Horizontal,
                    "diagonal" => args.algorithm = Algorithm::Diagonal,
                    "verticalright" | "vertical-right" => args.algorithm = Algorithm::VerticalRight,
                    "horizontalbottom" | "horizontal-bottom" => {
                        args.algorithm = Algorithm::HorizontalBottom
                    }
                    _ => {
                        eprintln!("Unknown algorithm");
                        std::process::exit(1);
                    }
                }
            }
            Short('c') | Long("crop") => args.crop = true,
            Long("ordering") => {
                let ord: String = parser.value()?.parse()?;
                match ord.parse::<Ordering>() {
                    Ok(o) => args.ordering = o,
                    Err(_) => {
                        eprintln!("Unknown ordering: {}. Valid options: maxside, width, height, area, filename (prefix with '-' for reverse)", ord);
                        std::process::exit(1);
                    }
                }
            }
            Long("exclude") => args.exclude.push(parser.value()?.to_string_lossy().into_owned()),
            Long("max-width") => args.max_width = Some(parser.value()?.parse()?),
            Long("max-height") => args.max_height = Some(parser.value()?.parse()?),
            Short('p') | Long("padding") => {
                args.padding = Some(parser.value()?.to_string_lossy().to_string());
            }
            Short('m') | Long("margin") => {
                args.margin = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("resize-algorithm") => {
                let fa: String = parser.value()?.parse()?;
                match fa.to_lowercase().as_str() {
                    "nearest" => args.resize_algorithm = FilterType::Nearest,
                    "triangle" => args.resize_algorithm = FilterType::Triangle,
                    "gaussian" => args.resize_algorithm = FilterType::Gaussian,
                    "lanczos3" => args.resize_algorithm = FilterType::Lanczos3,
                    _ => args.resize_algorithm = FilterType::CatmullRom,
                }
            }
            Long("project") => args.project = true,
            Long("cachebuster") => args.cachebuster = true,
            Long("cachebuster-filename") => args.cachebuster_filename = true,
            Long("cachebuster-filename-only-sprites") => {
                args.cachebuster_filename_only_sprites = true
            }
            // Long("colors") => {
            //     match parser.value()?.to_ascii_lowercase() {

            //     }
            // },
            Long("html") => args.html = true,
            Long("json") => args.json = true,
            Long("json-format") => {
                let fmt: String = parser.value()?.parse()?;
                if fmt.to_lowercase() == "hash" {
                    args.json_format = JsonFormat::Hash;
                } else {
                    args.json_format = JsonFormat::Array;
                }
            }
            Long("caat") => args.caat = true,
            Long("cocos2d") => args.cocos2d = true,
            Short('l') | Long("less") => args.less = true,
            Long("scss") => args.scss = true,
            Long("css-template") => {
                args.css_template = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("less-template") => {
                args.less_template = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("scss-template") => {
                args.scss_template = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("css-vars") => args.css_vars = true,
            Long("css-vars-template") => {
                args.css_vars_template = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("no-css") => args.no_css = true,
            Long("no-img") => args.no_img = true,
            Long("css") => {
                args.css = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("img") => {
                args.img = Some(std::path::PathBuf::from(
                    parser.value()?.to_string_lossy().to_string(),
                ));
            }
            Long("format") => {
                let fmt: String = parser.value()?.parse()?;
                args.sprite_format = match fmt.to_lowercase().as_str() {
                    "webp" => SpriteFormat::WebP,
                    "avif" => SpriteFormat::Avif,
                    _ => SpriteFormat::Png,
                };
            }
            Long("png8") => args.png8 = true,
            Long("ratios") => {
                args.ratios = Some(parser.value()?.to_string_lossy().to_string());
            }
            // The option `--retina` is only a shortcut for `--ratios=2,1`.
            Long("retina") => args.ratios = Some("2,1".to_owned()),
            Long("force") => args.force = true,
            Long("follow-links") => args.follow_links = true,
            Short('q') | Long("quiet") => args.quiet = true,
            Short('r') | Long("recursive") => args.recursive = true,
            Short('i') | Long("input") => {
                args.source =
                    std::path::PathBuf::from(parser.value()?.to_string_lossy().to_string());
            }
            Short('o') | Long("output") => {
                args.output =
                    std::path::PathBuf::from(parser.value()?.to_string_lossy().to_string());
            }
            Short('u') | Long("url") => {
                args.url = Some(String::from(
                    parser.value()?.to_os_string().to_str().unwrap(),
                ));
                url::Url::parse(&args.url.clone().unwrap()).unwrap();
            }
            Short('h') | Long("help") => {
                println!("{}", HELP_MSG);
                std::process::exit(0);
            }
            Long("separator") => {
                args.separator = parser.value()?.parse()?;
            }
            Long("namespace") => {
                args.namespace = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("sprite-namespace") => {
                args.sprite_namespace = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("pseudo-class-separator") => {
                args.pseudo_class_separator = Some(parser.value()?.to_string_lossy().to_string());
            }
            Long("no-duplicate") => args.no_duplicate = true,
            Long("watch") => args.watch = true,
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(args)
}
