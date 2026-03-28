use image::imageops::FilterType;
use spriters::{Algorithm, Config};

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

Output options:
--html:                 generate an HTML page
--json:                 generate JSON
--cocos2d:              generate Cocos2d XML compatible files
-l, --less:             generate LESS dynamic  stylesheet
--scss:                 generate SCSS dynamic stylesheet
--no-css:               do not generate CSS files
--no-img:               do not generate image file
--cachebuster:          when generating CSS, add image hash in the end of the filename
--cachebuster-filename: when generating CSS, add image hash before the extension in the filename

Image processing options:
-a, --algorithm:        algorithm to order images in the sprite: square (default), vertical, hortizontal, diagonal, vertical-right, horizontal-bottom
--png8:                 generate grascale image(s)
-c, --crop:             crop unnecessary transparent spaces
--max-width:            image max width in the sprite
--max-height:           image max height in the sprite
--resize-algorithm:     resize algorithm are nearesr, triangle, gaussian, lanczos3, catmullrom (default)";

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
                    "hortizontal" => args.algorithm = Algorithm::Hortizontal,
                    "diagonal" => args.algorithm = Algorithm::Diagonal,
                    "verticalright" | "vertical-right" => args.algorithm = Algorithm::VerticalRight,
                    "horizontalbottom" | "horizontal-bottom" => {
                        args.algorithm = Algorithm::HorizontalBottom
                    }
                    _ => {
                        eprintln!("Unknown algorithm");
                        std::process::exit(0);
                    }
                }
            }
            Short('c') | Long("crop") => args.crop = true,
            Long("max-width") => args.max_width = Some(parser.value()?.parse()?),
            Long("max-height") => args.max_height = Some(parser.value()?.parse()?),
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
            // Long("colors") => {
            //     match parser.value()?.to_ascii_lowercase() {

            //     }
            // },
            Long("html") => args.html = true,
            Long("json") => args.json = true,
            Long("cocos2d") => args.cocos2d = true,
            Short('l') | Long("less") => args.less = true,
            Long("scss") => args.scss = true,
            Long("no-css") => args.no_css = true,
            Long("no-img") => args.no_img = true,
            Long("png8") => args.png8 = true,
            // The option `--retina` is only a shortcut for `--ratios=2,1`.
            Long("retina") => args.ratios = Some("2,1".to_owned()),
            Long("force") => args.force = true,
            Long("follow-links") => args.follow_links = true,
            Short('q') | Long("quiet") => args.quiet = true,
            Short('r') | Long("recursive") => args.recursive = true,
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
            Long("no-duplicate") => args.no_duplicate = true,
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(args)
}
