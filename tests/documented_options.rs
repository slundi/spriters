//! Options that were documented but ignored (review §7): `--css`, `--img`, the custom
//! templates, `--pseudo-class-separator`, `--separator=camelcase` and `--version`.

use spriters::{Algorithm, Config, ConfigError, Manager, Ordering};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn with_tmp_dir<F: FnOnce(&Path)>(test: F) {
    let tmp = std::env::temp_dir().join(format!(
        "spriters_documented_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    test(&tmp);
    std::fs::remove_dir_all(&tmp).ok();
}

fn write_icon(path: &Path, width: u32, color: [u8; 4]) {
    image::RgbaImage::from_pixel(width, 8, image::Rgba(color))
        .save(path)
        .unwrap();
}

/// `<tmp>/icons` holding the given `(file name, width)` images.
fn icons(tmp: &Path, files: &[(&str, u32)]) -> PathBuf {
    let source = tmp.join("icons");
    std::fs::create_dir_all(&source).unwrap();
    for (index, (file, width)) in files.iter().enumerate() {
        write_icon(&source.join(file), *width, [index as u8 * 40, 0, 200, 255]);
    }
    source
}

fn dir(tmp: &Path, name: &str) -> PathBuf {
    let path = tmp.join(name);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Horizontal layout in alphabetical order (`FilenameReverse`: `Filename` sorts in reverse),
/// so that positions are predictable.
fn config(source: &Path, output: &Path) -> Config {
    Config {
        source: source.to_path_buf(),
        output: output.to_path_buf(),
        algorithm: Algorithm::Horizontal,
        ordering: Ordering::FilenameReverse,
        ..Default::default()
    }
}

fn generate(config: &Config) -> (Manager, bool) {
    assert_eq!(config.check(), Ok(()));
    let mut manager = Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    let succeeded = manager.generate();
    (manager, succeeded)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Sorted names of the files in `dir`, hidden ones included.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.ends_with(".tmp"))
        .collect();
    names.sort();
    names
}

fn spriters(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_spriters"));
    command.args(args).stdin(Stdio::null());
    for (key, _) in std::env::vars() {
        if key.starts_with("SPRITERS_") {
            command.env_remove(key);
        }
    }
    command.output().expect("failed to run spriters")
}

// ─── --css / --img ───────────────────────────────────────────────────────────

#[test]
fn css_and_img_options_choose_the_output_directories() {
    // Review §7: `--css` and `--img` were ignored, everything went to `--output`.
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8), ("b.png", 8)]);
        let output = dir(tmp, "out");
        let config = Config {
            css: Some(dir(tmp, "static/css")),
            img: Some(dir(tmp, "static/img")),
            json: true,
            html: true,
            ..config(&source, &output)
        };
        let (_, succeeded) = generate(&config);
        assert!(succeeded);

        assert!(names(&output).is_empty(), "{:?}", names(&output));
        assert_eq!(
            names(&tmp.join("static/img")),
            [".icons.spriters", "icons.png"]
        );
        assert_eq!(
            names(&tmp.join("static/css")),
            ["icons.css", "icons.html", "icons.json"]
        );
        // The CSS reaches the image from its own directory.
        let css = read(&tmp.join("static/css/icons.css"));
        assert!(css.contains("url('../img/icons.png')"), "{css}");
        let json = read(&tmp.join("static/css/icons.json"));
        assert!(
            json.contains("\"sprite_url\": \"../img/icons.png\""),
            "{json}"
        );

        // Up to date: the metadata is found again in the image directory.
        let mut manager = Manager {
            config: config.clone(),
            ..Default::default()
        };
        assert!(manager.check());
        // `stats` reads the image directory.
        let stats = spriters::sprite_stats(&tmp.join("static/img"), "icons").unwrap();
        assert_eq!((stats.width, stats.height), (16, 8));
    });
}

#[test]
fn css_option_alone_keeps_images_in_the_output_directory() {
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8)]);
        let output = dir(tmp, "public/sprites");
        let config = Config {
            css: Some(dir(tmp, "public/css")),
            ratios: Some(String::from("2,1")),
            cachebuster_filename: true,
            ..config(&source, &output)
        };
        assert!(generate(&config).1);
        assert!(names(&output).iter().any(|name| name.ends_with("@2x.png")));
        let css = read(&tmp.join("public/css/icons.css"));
        // Every image reference, media queries included, is relative to the CSS.
        let references: Vec<&str> = css
            .match_indices("url('")
            .map(|(at, _)| &css[at..])
            .collect();
        assert_eq!(references.len(), 2, "{css}");
        for reference in references {
            assert!(reference.starts_with("url('../sprites/icons_"), "{css}");
        }
    });
}

#[test]
fn image_directory_inside_the_css_directory() {
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8)]);
        let css_dir = dir(tmp, "css");
        let config = Config {
            css: Some(css_dir.clone()),
            img: Some(dir(tmp, "css/images")),
            ..config(&source, tmp)
        };
        assert!(generate(&config).1);
        assert!(read(&css_dir.join("icons.css")).contains("url('images/icons.png')"));

        // `--url` replaces the relative path.
        let config = Config {
            url: Some(String::from("/static/sprites")),
            force: true,
            ..config
        };
        assert!(generate(&config).1);
        assert!(read(&css_dir.join("icons.css")).contains("url('/static/sprites/icons.png')"));
    });
}

#[test]
fn outputs_in_the_source_are_not_read_back() {
    // The image and the CSS written into the source directory must not become images.
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8), ("b.png", 8)]);
        let config = Config {
            css: Some(dir(tmp, "icons/css")),
            img: Some(source.clone()),
            force: true,
            ..config(&source, tmp)
        };
        for _ in 0..2 {
            let (manager, succeeded) = generate(&config);
            assert!(succeeded);
            assert_eq!(manager.sprites[0].images.len(), 2);
        }
        let filter = spriters::OutputFilter::new(&config);
        assert!(filter.is_generated_file(&source.join("icons.png")));
        assert!(filter.is_generated_file(&source.join("css/icons.css")));
        assert!(!filter.is_generated_file(&source.join("a.png")));
        assert!(filter.is_output_dir(&source.join("css")));
    });
}

#[test]
fn missing_css_or_img_directory_is_an_error() {
    with_tmp_dir(|tmp| {
        let missing = tmp.join("missing");
        for (option, config) in [
            (
                "css",
                Config {
                    css: Some(missing.clone()),
                    ..config(tmp, tmp)
                },
            ),
            (
                "img",
                Config {
                    img: Some(missing.clone()),
                    ..config(tmp, tmp)
                },
            ),
        ] {
            assert_eq!(
                config.check(),
                Err(ConfigError::DirNotFound {
                    option,
                    path: missing.clone(),
                })
            );
        }

        let output = spriters(&["-i", &tmp.to_string_lossy(), "--img", "/nonexistent/img"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--img directory does not exist"),
            "{stderr}"
        );
    });
}

// ─── Templates ───────────────────────────────────────────────────────────────

const CSS_TEMPLATE: &str = "\
/* {{ name }} {{ width }}x{{ height }} {{ sprite_filename }} */
{% for image in images %}{{ image.selector }}{% if not image.last %}, {% endif %}{% endfor %} { background: url('{{ sprite_path }}'); }
{% for image in images %}
.{{ image.label }}{{ image.pseudo }} { background-position: -{{ image.x }}px -{{ image.y }}px; width: {{ image.width }}px; height: {{ image.height }}px; } /* {{ image.name }} {{ image.filename }} */
{%- endfor %}
{% for ratio in ratios %}
@media (min-resolution: {{ ratio.ratio }}dppx) { /* {{ ratio.fraction }} */ .x { background-image: url('{{ ratio.sprite_path }}'); } }
{%- endfor %}
";

#[test]
fn css_template_replaces_the_built_in_css() {
    // Review §7: `--css-template` and the other templates were parsed but never used.
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a_b.png", 8), ("c.png", 4)]);
        let output = dir(tmp, "out");
        let template = tmp.join("sprite.css.jinja");
        std::fs::write(&template, CSS_TEMPLATE).unwrap();
        let config = Config {
            css_template: Some(template.clone()),
            ratios: Some(String::from("2,1")),
            ..config(&source, &output)
        };
        assert!(generate(&config).1);

        let css = read(&output.join("icons.css"));
        let expected = "\
/* icons 6x4 icons.png */
.sprite-icons-a-b, .sprite-icons-c { background: url('icons.png'); }

.sprite-icons-a-b { background-position: -0px -0px; width: 4px; height: 4px; } /* a-b a_b.png */
.sprite-icons-c { background-position: -4px -0px; width: 2px; height: 4px; } /* c c.png */

@media (min-resolution: 2dppx) { /* 2 */ .x { background-image: url('icons@2x.png'); } }
";
        assert_eq!(css, expected);

        // A change of the template alone regenerates the CSS (no --force).
        std::fs::write(&template, "/* {{ images | length }} images */\n").unwrap();
        assert!(generate(&config).1);
        assert_eq!(read(&output.join("icons.css")), "/* 2 images */\n");
    });
}

#[test]
fn each_template_is_used_for_its_output() {
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8)]);
        let output = dir(tmp, "out");
        let template = |name: &str| {
            let path = tmp.join(format!("{name}.jinja"));
            std::fs::write(&path, format!("{name} {{{{ images[0].label }}}}\n")).unwrap();
            Some(path)
        };
        let config = Config {
            less: true,
            scss: true,
            css_vars: true,
            less_template: template("less"),
            scss_template: template("scss"),
            css_vars_template: template("vars"),
            ..config(&source, &output)
        };
        assert!(generate(&config).1);
        assert_eq!(read(&output.join("icons.less")), "less sprite-icons-a\n");
        assert_eq!(read(&output.join("icons.scss")), "scss sprite-icons-a\n");
        assert_eq!(
            read(&output.join("icons.vars.css")),
            "vars sprite-icons-a\n"
        );
        // No CSS template: the built-in CSS.
        assert!(read(&output.join("icons.css")).contains(".sprite-icons-a {"));
    });
}

#[test]
fn invalid_templates_are_reported() {
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("a.png", 8)]);
        let output = dir(tmp, "out");

        // Missing file and syntax error: rejected before anything is generated.
        let missing = tmp.join("missing.jinja");
        let broken = tmp.join("broken.jinja");
        std::fs::write(&broken, "{% for image in images %}").unwrap();
        for path in [&missing, &broken] {
            let config = Config {
                scss_template: Some(path.clone()),
                ..config(&source, &output)
            };
            match config.check() {
                Err(ConfigError::InvalidTemplate { option, .. }) => {
                    assert_eq!(option, "scss-template")
                }
                other => panic!("{}: {other:?}", path.display()),
            }
        }

        // Valid syntax that fails while rendering: the run fails, no CSS is written.
        let failing = tmp.join("failing.jinja");
        std::fs::write(&failing, "{% for x in width %}{% endfor %}").unwrap();
        let config = Config {
            css_template: Some(failing),
            ..config(&source, &output)
        };
        let (_, succeeded) = generate(&config);
        assert!(!succeeded);
        assert!(!output.join("icons.css").exists());

        let template_arg = broken.to_string_lossy().into_owned();
        let source_arg = source.to_string_lossy().into_owned();
        let output_arg = output.to_string_lossy().into_owned();
        let run = spriters(&[&source_arg, &output_arg, "--css-template", &template_arg]);
        assert!(!run.status.success());
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(stderr.contains("--css-template"), "{stderr}");
        assert!(stderr.contains("syntax error"), "{stderr}");
    });
}

// ─── --pseudo-class-separator ────────────────────────────────────────────────

#[test]
fn pseudo_class_separator_adds_pseudo_classes() {
    // Review §7: `--pseudo-class-separator` was ignored (`button__hover` gave
    // `.sprite-icons-button-hover`).
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("button.png", 8), ("button__hover.png", 4)]);
        let output = dir(tmp, "out");
        let config = Config {
            pseudo_class_separator: Some(String::from("__")),
            json: true,
            html: true,
            tailwind: true,
            css_vars: true,
            ..config(&source, &output)
        };
        assert!(generate(&config).1);

        let css = read(&output.join("icons.css"));
        assert!(
            css.starts_with(".sprite-icons-button, .sprite-icons-button:hover {"),
            "{css}"
        );
        assert!(
            css.contains(
                ".sprite-icons-button:hover {\n  background-position: -8px 0;\n  width: 4px;"
            ),
            "{css}"
        );
        assert!(!css.contains("button-hover"), "{css}");

        // Other outputs keep a name per file.
        let json = read(&output.join("icons.json"));
        assert!(json.contains("\"filename\": \"button-hover\""), "{json}");
        let vars = read(&output.join("icons.vars.css"));
        assert!(
            vars.contains("--sprite-icons-button-hover-x: -8px;"),
            "{vars}"
        );
        // The HTML page shows the selector, on an element with the class.
        let html = read(&output.join("icons.html"));
        assert!(
            html.contains(
                "<td>.sprite-icons-button:hover</td><td><div class=\"sprite-icons-button\">"
            ),
            "{html}"
        );
        // A class appears once in the safelist.
        let tailwind = read(&output.join("icons.tailwind.json"));
        assert_eq!(
            tailwind.matches("\"sprite-icons-button\"").count(),
            1,
            "{tailwind}"
        );

        // Without the option (the default), the file name is the label.
        let config = Config {
            pseudo_class_separator: None,
            force: true,
            ..config
        };
        assert!(generate(&config).1);
        assert!(read(&output.join("icons.css")).contains(".sprite-icons-button-hover {"));
    });
}

#[test]
fn pseudo_class_selector_collisions_are_reported() {
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("button__hover.png", 8)]);
        std::fs::create_dir_all(source.join("sub")).unwrap();
        write_icon(&source.join("sub/button__HOVER.png"), 4, [9, 9, 9, 255]);
        let config = Config {
            pseudo_class_separator: Some(String::from("__")),
            recursive: true,
            ..config(&source, &dir(tmp, "out"))
        };
        let (manager, succeeded) = generate(&config);
        assert!(!succeeded, "a left-out image fails the run");
        assert_eq!(manager.sprites[0].images.len(), 1);
        assert_eq!(manager.sprites[0].failed_images.len(), 1);
    });
}

// ─── --separator=camelcase ───────────────────────────────────────────────────

#[test]
fn camelcase_separator_joins_class_names_in_camel_case() {
    // Review §7: `--separator=camelcase` gave `spritecamelcaseicons…`.
    with_tmp_dir(|tmp| {
        let source = icons(tmp, &[("my_image.png", 8), ("arrow-left.png", 8)]);
        let output = dir(tmp, "out");
        let config = Config {
            separator: String::from("camelCase"),
            html: true,
            ..config(&source, &output)
        };
        assert!(generate(&config).1);
        let css = read(&output.join("icons.css"));
        assert!(
            css.starts_with(".spriteIconsArrowLeft, .spriteIconsMyImage {"),
            "{css}"
        );
        assert!(!css.contains("camel"), "{css}");
        assert!(read(&output.join("icons.html")).contains("class=\"spriteIconsMyImage\""));
    });
}

// ─── --version ───────────────────────────────────────────────────────────────

#[test]
fn version_option_prints_the_version() {
    // Review §7: `--version` was missing, although `install.sh` calls it.
    let expected = format!("spriters {}\n", env!("CARGO_PKG_VERSION"));
    for option in ["--version", "-V"] {
        let output = spriters(&[option]);
        assert!(output.status.success(), "{option}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{option}"
        );
    }
}
