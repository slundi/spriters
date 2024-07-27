use image::imageops::FilterType;
use spriters::Config;

pub(crate) fn parse_args() -> Result<Config, lexopt::Error> {
    use lexopt::prelude::*;

    let mut args: Config = Config::default();

    let mut thing = None;
    let mut number = 1;
    let mut shout = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('a') | Long("algorithm") => todo!(),
            Short('c') | Long("crop") => args.crop = true,
            Long("max-width") => args.max_width = Some(parser.value()?.parse()?),
            Long("max-height") => args.max_height = Some(parser.value()?.parse()?),
            Long("resize-alogorithm") => {
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
            Long("follow_links") => args.follow_links = true,
            Long("watch") => args.watch = true,
            Short('q') | Long("quiet") => args.quiet = true,
            Short('r') | Long("recursive") => args.recursive = true,
            Short('u') | Long("url") => {
                // args.url = parser.value().expect("Cannot get URL").parse().expect("Error while getting URL")
                todo!();
            }

            Short('n') | Long("number") => {
                number = parser.value()?.parse()?;
            }
            Long("shout") => {
                shout = true;
            }
            Value(val) if thing.is_none() => {
                thing = Some(val.string()?);
            }
            Long("help") => {
                println!("Usage: hello [-n|--number=NUM] [--shout] THING");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Config {
        ..Default::default()
    })
}
