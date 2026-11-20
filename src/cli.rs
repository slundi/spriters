use spriters::{Config, config};

const HELP_MSG: &str = "spriters [OPTIONS] [SOURCE_DIR] [OUTPUT_DIR]
spriters stats [OUTPUT_DIR]
spriters completions <bash|zsh|fish>

By default, it search images on the current working directory. The output is alson on the working directory.
SOURCE_DIR and OUTPUT_DIR can also be given with -i and -o (but not both ways).

Subcommands are only recognised as the first argument:
stats:                  report the dimensions, file size and packing efficiency of generated sprites
completions:            print the shell completion script
A SOURCE_DIR named like a subcommand is written as a path or after '--': spriters ./stats dist/

General options:
-h, --help:             print this message
-q, --quiet:            don't print anything on the console
-i, --input:            input directory ('-' is the same as --stdin)
-o, --output:           output directory
--stdin:                read newline-separated image paths from stdin instead of walking the input directory
                        (the input directory still names the sprite and holds spriters.toml)
-r, --recursive:        read directories recursively and add all the images to the same sprite
--project:              generate a sprite per working directory sub folder
--watch:                watch working directory for changes (run in background)
--follow-links:         follow symbolic links
--force:                force regeneration even if sprite is up to date
--dry-run:              print what would be generated without writing any files
--check:                exit non-zero if sprite output is missing or stale (for CI)

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
--tailwind:             generate a .tailwind.json file with a Tailwind CSS safelist of all sprite class names
--dts:                  generate a .d.ts TypeScript definition file with a SpriteName union type
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
-u, --url:              base URL for sprite images in CSS (e.g., /static/ or https://cdn.example.com/sprites)

Image processing options:
-a, --algorithm:        algorithm to order images in the sprite: square (default), vertical, horizontal, diagonal, vertical-right, horizontal-bottom
--ordering:             sort order for images: maxside (default), width, height, area, filename. Prefix with '-' for reverse (e.g., -maxside)
--png8:                 generate 8-bit palette PNG (up to 256 colours, transparency kept)
--ratios:               generate sprites at multiple ratios for different DPIs (e.g., '2,1' or '2,1.5,1')
--retina:               shortcut for --ratios=2,1
-c, --crop:             crop unnecessary transparent spaces
--exclude:              skip files whose name matches a glob pattern (repeatable, e.g. --exclude '_draft_*' --exclude '*.tmp')
--max-width:            image max width in the sprite
--max-height:           image max height in the sprite
--resize-algorithm:     resize algorithm: nearest, triangle, gaussian, lanczos3, catmullrom (default)
-p, --padding:          padding around each image (included in dimensions). Formats: 10, '5 10', '1 2 3 4'
-m, --margin:           margin around each image (not included in dimensions). Formats: 10, '5 10', '1 2 3 4'";

/// Applies one command line option to a configuration.
type Setter = Box<dyn Fn(&mut Config)>;

/// Settings given on the command line.
///
/// Only the options actually passed are recorded, so they can be applied on top of
/// `spriters.toml` and `SPRITERS_*` env vars (see [`spriters::config::resolve`]).
#[derive(Default)]
pub(crate) struct CliArgs {
    setters: Vec<Setter>,
    /// Read the image paths from stdin (`--stdin` or `-i -`).
    read_stdin: bool,
}
impl CliArgs {
    /// Whether image paths must be read from stdin. Never implied by stdin being a pipe:
    /// many callers (Node `spawn`, `docker run -i`, CI runners) leave a pipe open forever.
    pub(crate) fn reads_stdin(&self) -> bool {
        self.read_stdin
    }

    fn set(&mut self, setter: impl Fn(&mut Config) + 'static) {
        self.setters.push(Box::new(setter));
    }

    /// Apply the command line settings to `config`, in the order they were given.
    pub(crate) fn apply(&self, config: &mut Config) {
        for setter in &self.setters {
            setter(config);
        }
    }

    /// Whether `--quiet` was given (needed to set up logging before resolving the config).
    pub(crate) fn quiet(&self) -> bool {
        let mut config = Config::default();
        self.apply(&mut config);
        config.quiet
    }
}

pub(crate) fn parse_args() -> Result<CliArgs, lexopt::Error> {
    parse_args_from(lexopt::Parser::from_env())
}

fn parse_args_from(mut parser: lexopt::Parser) -> Result<CliArgs, lexopt::Error> {
    use lexopt::prelude::*;

    let mut cli = CliArgs::default();
    // Directories given with -i/-o and as positional arguments, to report conflicts.
    let mut source_option = false;
    let mut output_option = false;
    let mut positionals: Vec<std::path::PathBuf> = Vec::new();

    while let Some(arg) = parser.next()? {
        match arg {
            Short('a') | Long("algorithm") => {
                let algorithm = parse_value(&mut parser, config::parse_algorithm)?;
                cli.set(move |config| config.algorithm = algorithm);
            }
            Short('c') | Long("crop") => cli.set(|config| config.crop = true),
            Long("ordering") => {
                let ordering = parse_value(&mut parser, config::parse_ordering)?;
                cli.set(move |config| config.ordering = ordering);
            }
            Long("exclude") => {
                let pattern = parser.value()?.to_string_lossy().into_owned();
                cli.set(move |config| config.exclude.push(pattern.clone()));
            }
            Long("max-width") => {
                let max_width: u32 = parser.value()?.parse()?;
                cli.set(move |config| config.max_width = Some(max_width));
            }
            Long("max-height") => {
                let max_height: u32 = parser.value()?.parse()?;
                cli.set(move |config| config.max_height = Some(max_height));
            }
            Short('p') | Long("padding") => {
                let padding = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.padding = Some(padding.clone()));
            }
            Short('m') | Long("margin") => {
                let margin = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.margin = Some(margin.clone()));
            }
            Long("resize-algorithm") => {
                let filter = parse_value(&mut parser, config::parse_resize_algorithm)?;
                cli.set(move |config| config.resize_algorithm = filter);
            }
            Long("project") => cli.set(|config| config.project = true),
            Long("cachebuster") => cli.set(|config| config.cachebuster = true),
            Long("cachebuster-filename") => cli.set(|config| config.cachebuster_filename = true),
            Long("cachebuster-filename-only-sprites") => {
                cli.set(|config| config.cachebuster_filename_only_sprites = true)
            }
            Long("html") => cli.set(|config| config.html = true),
            Long("json") => cli.set(|config| config.json = true),
            Long("json-format") => {
                let json_format = parse_value(&mut parser, config::parse_json_format)?;
                cli.set(move |config| config.json_format = json_format);
            }
            Long("caat") => cli.set(|config| config.caat = true),
            Long("cocos2d") => cli.set(|config| config.cocos2d = true),
            Short('l') | Long("less") => cli.set(|config| config.less = true),
            Long("scss") => cli.set(|config| config.scss = true),
            Long("css-template") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.css_template = Some(path.clone()));
            }
            Long("less-template") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.less_template = Some(path.clone()));
            }
            Long("scss-template") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.scss_template = Some(path.clone()));
            }
            Long("css-vars") => cli.set(|config| config.css_vars = true),
            Long("tailwind") => cli.set(|config| config.tailwind = true),
            Long("dts") => cli.set(|config| config.dts = true),
            Long("css-vars-template") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.css_vars_template = Some(path.clone()));
            }
            Long("no-css") => cli.set(|config| config.no_css = true),
            Long("no-img") => cli.set(|config| config.no_img = true),
            Long("css") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.css = Some(path.clone()));
            }
            Long("img") => {
                let path = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.img = Some(path.clone()));
            }
            Long("format") => {
                let sprite_format = parse_value(&mut parser, config::parse_sprite_format)?;
                cli.set(move |config| config.sprite_format = sprite_format);
            }
            Long("png8") => cli.set(|config| config.png8 = true),
            Long("ratios") => {
                let ratios = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.ratios = Some(ratios.clone()));
            }
            // The option `--retina` is only a shortcut for `--ratios=2,1`.
            Long("retina") => cli.set(|config| config.ratios = Some("2,1".to_owned())),
            Long("force") => cli.set(|config| config.force = true),
            Long("dry-run") => cli.set(|config| config.dry_run = true),
            Long("check") => cli.set(|config| config.check = true),
            Long("follow-links") => cli.set(|config| config.follow_links = true),
            Short('q') | Long("quiet") => cli.set(|config| config.quiet = true),
            Short('r') | Long("recursive") => cli.set(|config| config.recursive = true),
            Short('i') | Long("input") => {
                let value = parser.value()?;
                if value == "-" {
                    cli.read_stdin = true;
                } else {
                    source_option = true;
                    let source = std::path::PathBuf::from(value);
                    cli.set(move |config| config.source = source.clone());
                }
            }
            Long("stdin") => cli.read_stdin = true,
            Short('o') | Long("output") => {
                output_option = true;
                let output = std::path::PathBuf::from(parser.value()?);
                cli.set(move |config| config.output = output.clone());
            }
            Value(value) => {
                if positionals.len() == 2 {
                    return Err(Value(value).unexpected());
                }
                positionals.push(std::path::PathBuf::from(value));
            }
            Short('u') | Long("url") => {
                // Prepended as is: relative URLs (`/static/`) are the most common case.
                let url = parser.value()?.into_string().map_err(|value| {
                    lexopt::Error::Custom(format!("--url must be valid UTF-8: {value:?}").into())
                })?;
                cli.set(move |config| config.url = Some(url.clone()));
            }
            Short('h') | Long("help") => {
                println!("{}", HELP_MSG);
                std::process::exit(0);
            }
            Long("separator") => {
                let separator: String = parser.value()?.parse()?;
                cli.set(move |config| config.separator = separator.clone());
            }
            Long("namespace") => {
                let namespace = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.namespace = Some(namespace.clone()));
            }
            Long("sprite-namespace") => {
                let namespace = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.sprite_namespace = Some(namespace.clone()));
            }
            Long("pseudo-class-separator") => {
                let separator = parser.value()?.to_string_lossy().to_string();
                cli.set(move |config| config.pseudo_class_separator = Some(separator.clone()));
            }
            Long("no-duplicate") => cli.set(|config| config.no_duplicate = true),
            Long("watch") => cli.set(|config| config.watch = true),
            _ => return Err(arg.unexpected()),
        }
    }

    // `spriters [OPTIONS] [SOURCE_DIR] [OUTPUT_DIR]`
    let mut positionals = positionals.into_iter();
    if let Some(source) = positionals.next() {
        if source_option {
            return Err(conflict("source", "-i/--input", &source));
        }
        cli.set(move |config| config.source = source.clone());
    }
    if let Some(output) = positionals.next() {
        if output_option {
            return Err(conflict("output", "-o/--output", &output));
        }
        cli.set(move |config| config.output = output.clone());
    }

    Ok(cli)
}

/// Value of the current option, parsed with `parse`. Unlike `spriters.toml` and env vars,
/// an invalid value on the command line is an error: it used to fall back to a default
/// in silence (`--format foo` wrote a PNG).
fn parse_value<T>(
    parser: &mut lexopt::Parser,
    parse: fn(&str) -> Result<T, config::InvalidValue>,
) -> Result<T, lexopt::Error> {
    let value: String = lexopt::ValueExt::parse(&parser.value()?)?;
    parse(&value).map_err(|e| lexopt::Error::Custom(e.into()))
}

/// A directory given both with an option and as a positional argument.
fn conflict(what: &str, option: &str, positional: &std::path::Path) -> lexopt::Error {
    lexopt::Error::Custom(
        format!(
            "{what} directory given twice: with {option} and as argument {:?}",
            positional
        )
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::imageops::FilterType;
    use spriters::{Algorithm, JsonFormat, Ordering, SpriteFormat};

    fn parse(args: &[&str]) -> CliArgs {
        parse_args_from(lexopt::Parser::from_args(args.iter().copied())).unwrap()
    }

    fn parsed_config(args: &[&str]) -> Config {
        let mut config = Config::default();
        parse(args).apply(&mut config);
        config
    }

    #[test]
    fn test_no_argument_leaves_config_untouched() {
        let mut config = Config {
            force: true,
            namespace: Some(String::from("from-file")),
            ..Default::default()
        };
        parse(&[]).apply(&mut config);
        assert!(config.force);
        assert_eq!(config.namespace.as_deref(), Some("from-file"));
    }

    #[test]
    fn test_options_are_applied() {
        let config = parsed_config(&[
            "--force",
            "-a",
            "vertical",
            "--ordering",
            "-width",
            "--namespace",
            "cli-ns",
            "-i",
            "icons",
            "-o",
            "dist",
            "--max-width",
            "32",
            "--json-format",
            "hash",
            "--format",
            "webp",
            "--retina",
        ]);
        assert!(config.force);
        assert_eq!(config.algorithm, Algorithm::Vertical);
        assert_eq!(config.ordering, Ordering::WidthReverse);
        assert_eq!(config.namespace.as_deref(), Some("cli-ns"));
        assert_eq!(config.source, std::path::PathBuf::from("icons"));
        assert_eq!(config.output, std::path::PathBuf::from("dist"));
        assert_eq!(config.max_width, Some(32));
        assert_eq!(config.json_format, JsonFormat::Hash);
        assert_eq!(config.sprite_format, SpriteFormat::WebP);
        assert_eq!(config.ratios.as_deref(), Some("2,1"));
    }

    fn parse_error(args: &[&str]) -> String {
        match parse_args_from(lexopt::Parser::from_args(args.iter().copied())) {
            Ok(_) => panic!("{args:?} must be rejected"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn test_invalid_values_are_errors() {
        // `--format foo` gave PNG, `--resize-algorithm foo` CatmullRom and
        // `--json-format foo` array, without any message.
        for (args, expected) in [
            (["--format", "gif"], "invalid image format 'gif'"),
            (
                ["--resize-algorithm", "bicubic"],
                "invalid resize algorithm 'bicubic'",
            ),
            (["--json-format", "object"], "invalid JSON format 'object'"),
            // These two printed a message and called `process::exit`.
            (["-a", "spiral"], "invalid algorithm 'spiral'"),
            (["--ordering", "size"], "invalid ordering 'size'"),
        ] {
            let message = parse_error(&args);
            assert!(message.contains(expected), "{args:?}: {message}");
            assert!(message.contains("expected"), "{args:?}: {message}");
        }
    }

    #[test]
    fn test_valid_values_are_case_insensitive() {
        let config = parsed_config(&[
            "--format",
            "AVIF",
            "--resize-algorithm",
            "Lanczos3",
            "--json-format",
            "Array",
            "-a",
            "Horizontal-Bottom",
        ]);
        assert_eq!(config.sprite_format, SpriteFormat::Avif);
        assert_eq!(config.resize_algorithm, FilterType::Lanczos3);
        assert_eq!(config.json_format, JsonFormat::Array);
        assert_eq!(config.algorithm, Algorithm::HorizontalBottom);
        // The default filter can also be given explicitly.
        let config = parsed_config(&["--resize-algorithm", "catmullrom"]);
        assert_eq!(config.resize_algorithm, FilterType::CatmullRom);
    }

    #[test]
    fn test_positional_source_and_output() {
        // Review 3.8: `spriters icons/ dist/ --retina` failed with `unexpected argument`.
        let config = parsed_config(&["icons/", "dist/", "--retina"]);
        assert_eq!(config.source, std::path::PathBuf::from("icons/"));
        assert_eq!(config.output, std::path::PathBuf::from("dist/"));
        assert_eq!(config.ratios.as_deref(), Some("2,1"));
    }

    #[test]
    fn test_positional_source_only_keeps_default_output() {
        let config = parsed_config(&["--json", "icons"]);
        assert_eq!(config.source, std::path::PathBuf::from("icons"));
        assert_eq!(config.output, Config::default().output);
        assert!(config.json);
    }

    #[test]
    fn test_positional_mixed_with_options() {
        let config = parsed_config(&["-o", "dist", "icons"]);
        assert_eq!(config.source, std::path::PathBuf::from("icons"));
        assert_eq!(config.output, std::path::PathBuf::from("dist"));

        // `--` ends the options: a directory may start with a dash.
        let config = parsed_config(&["--", "-icons", "dist"]);
        assert_eq!(config.source, std::path::PathBuf::from("-icons"));
    }

    #[test]
    fn test_positional_source_with_stdin() {
        // File-list mode: the positional directory still names the sprite.
        let cli = parse(&["--stdin", "icons"]);
        assert!(cli.reads_stdin());
        let mut config = Config::default();
        cli.apply(&mut config);
        assert_eq!(config.source, std::path::PathBuf::from("icons"));
        assert!(parse(&["-i", "-", "icons"]).reads_stdin());
    }

    #[test]
    fn test_too_many_positionals_is_an_error() {
        assert!(parse_error(&["icons", "dist", "extra"]).contains("extra"));
    }

    #[test]
    fn test_directory_given_twice_is_an_error() {
        assert!(parse_error(&["-i", "assets", "icons"]).contains("source directory given twice"),);
        assert!(
            parse_error(&["-o", "dist", "icons", "out"]).contains("output directory given twice"),
        );
    }

    #[test]
    fn test_stdin_is_only_read_on_request() {
        // Review 3.6: stdin was read whenever it was not a terminal.
        assert!(!parse(&[]).reads_stdin());
        assert!(!parse(&["-i", "icons", "-o", "dist"]).reads_stdin());
        assert!(parse(&["--stdin"]).reads_stdin());
        assert!(parse(&["--stdin", "-i", "icons"]).reads_stdin());
    }

    #[test]
    fn test_input_dash_reads_stdin_and_keeps_default_source() {
        let cli = parse(&["-i", "-", "-o", "dist"]);
        assert!(cli.reads_stdin());
        let mut config = Config::default();
        cli.apply(&mut config);
        assert_eq!(config.source, Config::default().source);
        assert_eq!(config.output, std::path::PathBuf::from("dist"));
    }

    #[test]
    fn test_repeated_option_last_wins_and_exclude_accumulates() {
        let config = parsed_config(&[
            "--namespace",
            "first",
            "--namespace",
            "second",
            "--exclude",
            "*.tmp",
            "--exclude",
            "draft-*",
        ]);
        assert_eq!(config.namespace.as_deref(), Some("second"));
        assert_eq!(config.exclude, vec!["*.tmp", "draft-*"]);
    }

    #[test]
    fn test_cli_overrides_env_and_file() {
        // Review 3.3: `SPRITERS_FORCE=0 spriters --force` ignored --force.
        let source = std::env::temp_dir().join(format!(
            "spriters_cli_prio_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(
            source.join("spriters.toml"),
            "force = false\nnamespace = \"toml-ns\"\nalgorithm = \"diagonal\"\n",
        )
        .unwrap();

        let source_arg = source.to_string_lossy().into_owned();
        let cli = parse(&["--force", "-i", &source_arg, "--namespace", "cli-ns"]);
        let config = spriters::config::resolve(
            |config| cli.apply(config),
            |key| match key {
                "SPRITERS_FORCE" => Some(String::from("0")),
                "SPRITERS_NAMESPACE" => Some(String::from("env-ns")),
                _ => None,
            },
        );
        std::fs::remove_dir_all(&source).ok();

        assert!(config.force, "--force must win over env and file");
        assert_eq!(config.namespace.as_deref(), Some("cli-ns"));
        // Not given on the command line: the file value survives.
        assert_eq!(config.algorithm, Algorithm::Diagonal);
    }
}
