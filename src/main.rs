use log::*;
use notify::{Event, RecursiveMode, Watcher};
use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::Duration;

pub mod cli;

/// Build and save (or check) every sprite. `cache` keeps decoded images between `--watch`
/// runs. Returns whether every image could be loaded and every file written.
fn run_sprite_generation(
    config: &spriters::Config,
    cache: Option<std::sync::Arc<spriters::ImageCache>>,
) -> bool {
    let mut manager = spriters::Manager {
        project: config.project,
        config: config.clone(),
        cache,
        ..Default::default()
    };
    // Each sprite is built and released in turn, so a project never holds every decoded
    // sprite in memory at once.
    if config.check {
        if manager.check() {
            info!("All sprites are up to date.");
        } else {
            eprintln!(
                "One or more sprites are out of date. Run spriters without --check to regenerate."
            );
            std::process::exit(1);
        }
        return true;
    }

    manager.generate()
}

fn handle_stats(output_dir: Option<&std::path::Path>) {
    let dir = output_dir.unwrap_or(std::path::Path::new("."));
    if !dir.is_dir() {
        eprintln!("Error: '{}' is not a directory", dir.display());
        std::process::exit(1);
    }

    let names = spriters::list_sprite_names(dir);
    if names.is_empty() {
        eprintln!(
            "No sprite metadata found in '{}'. Generate sprites first.",
            dir.display()
        );
        std::process::exit(1);
    }

    for name in &names {
        match spriters::sprite_stats(dir, name) {
            Some(s) => {
                println!("Sprite:     {}", s.name);
                println!("  Dimensions: {}×{} px", s.width, s.height);
                println!("  File size:  {}", format_bytes(s.file_size));
                println!(
                    "  Efficiency: {}%  ({} of {} px used)",
                    s.packing_efficiency_pct(),
                    s.used_pixels,
                    s.total_pixels
                );
                println!();
            }
            None => eprintln!("Could not load image for sprite '{}'", name),
        }
    }
}

fn handle_completions(shell: Option<&str>) {
    match shell {
        Some("bash") => print!("{}", spriters::completions::BASH),
        Some("zsh") => print!("{}", spriters::completions::ZSH),
        Some("fish") => print!("{}", spriters::completions::FISH),
        _ => {
            eprintln!("Usage: spriters completions <bash|zsh|fish>");
            std::process::exit(1);
        }
    }
}

/// Image paths from a newline-separated list; surrounding spaces and blank lines are ignored.
fn read_file_list(reader: impl BufRead) -> Vec<std::path::PathBuf> {
    reader
        .lines()
        .map_while(Result::ok)
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

/// Whether a watcher event only concerns files written by spriters itself: reacting to it
/// would regenerate in a loop when the output directory is inside the watched source.
fn is_own_output(config: &spriters::Config, event: &Event) -> bool {
    // Rebuilt for each event: sprites recorded since the last one must be recognised too.
    let outputs = spriters::OutputFilter::new(config);
    !event.paths.is_empty()
        && event
            .paths
            .iter()
            .all(|path| outputs.is_generated_file(path))
}

fn format_bytes(n: u64) -> String {
    if n < 1_024 {
        format!("{} B", n)
    } else if n < 1_048_576 {
        format!("{:.1} KB", n as f64 / 1_024.0)
    } else {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    }
}

fn main() -> ExitCode {
    // Detect subcommands before full argument parsing. `args_os`: `args` panics on
    // arguments that are not valid UTF-8 (lexopt reports those properly).
    let raw: Vec<std::ffi::OsString> = std::env::args_os().collect();
    match raw.get(1).and_then(|arg| arg.to_str()) {
        Some("stats") => {
            handle_stats(raw.get(2).map(std::path::Path::new));
            return ExitCode::SUCCESS;
        }
        Some("completions") => {
            handle_completions(raw.get(2).and_then(|arg| arg.to_str()));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let cli = match cli::parse_args() {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("Error: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Start logging before reading spriters.toml and env vars, so their warnings are shown.
    // `quiet` can only be set on the command line.
    stderrlog::new()
        .module(module_path!())
        .verbosity(log::Level::Info)
        .quiet(cli.quiet())
        .init()
        .unwrap();

    let mut config =
        spriters::config::resolve(|config| cli.apply(config), |key| std::env::var(key).ok());

    // Read newline-delimited image paths from stdin only when asked for, so spriters
    // composes with other tools (`fd -e png . | spriters --stdin -o dist/`) without
    // hanging when a caller leaves stdin open.
    if cli.reads_stdin() {
        if std::io::stdin().is_terminal() {
            info!("Reading image paths from the terminal, end with Ctrl+D");
        }
        config.files.extend(read_file_list(std::io::stdin().lock()));
    }

    if let Err(err) = config.check() {
        // Printed directly: the error must be visible even with --quiet.
        eprintln!("Error: {err}");
        return ExitCode::FAILURE;
    }
    if config.files.is_empty() {
        info!("Input directory: {}", config.source.to_string_lossy());
    } else {
        info!("Reading {} files from stdin", config.files.len());
    }
    info!("Output directory: {}", config.output.to_string_lossy());

    // In watch mode, decoded images are kept between runs: a change only re-decodes the
    // files that changed.
    let cache = config
        .watch
        .then(|| std::sync::Arc::new(spriters::ImageCache::default()));

    // Initial sprite generation. Errors are already logged; the exit code reports them, so
    // that CI does not pass on a broken sprite.
    let succeeded = run_sprite_generation(&config, cache.clone());

    // If watch mode is enabled, start watching for changes (a failed run is fixed by
    // the next change)
    if config.watch {
        info!(
            "Watching for changes in {:?}... (Ctrl+C to stop)",
            config.source
        );

        let (tx, rx) = channel();

        let watch_config = config.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res
                && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
                && !is_own_output(&watch_config, &event)
            {
                let _ = tx.send(());
            }
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&config.source, RecursiveMode::Recursive)
            .expect("Failed to watch source directory");

        let debounce_duration = Duration::from_millis(500);

        loop {
            if rx.recv().is_ok() {
                while rx.recv_timeout(debounce_duration).is_ok() {}
                info!("Change detected, regenerating sprites...");
                run_sprite_generation(&config, cache.clone());
                if let Some(cache) = &cache {
                    cache.retain_existing();
                }
                info!("Watching for changes... (Ctrl+C to stop)");
            }
        }
    }

    if succeeded {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, is_own_output, read_file_list};

    #[test]
    fn test_read_file_list() {
        let input = "icons/a.png\n\n  icons/b c.png  \r\n\t\nicons/d.svg";
        assert_eq!(
            read_file_list(input.as_bytes()),
            vec![
                std::path::PathBuf::from("icons/a.png"),
                std::path::PathBuf::from("icons/b c.png"),
                std::path::PathBuf::from("icons/d.svg"),
            ]
        );
        assert!(read_file_list("".as_bytes()).is_empty());
    }
    use notify::{Event, EventKind, event::ModifyKind};

    #[test]
    fn test_is_own_output_ignores_generated_files_only() {
        let dir = std::env::temp_dir().join(format!("spriters_watch_{}", std::process::id()));
        let source = dir.join("same");
        std::fs::create_dir_all(&source).unwrap();
        let config = spriters::Config {
            source: source.clone(),
            output: source.clone(),
            ..Default::default()
        };
        let event = |files: &[&str]| {
            files.iter().fold(
                Event::new(EventKind::Modify(ModifyKind::Any)),
                |event, file| event.add_path(source.join(file)),
            )
        };

        // Review 3.5: writing the outputs into the watched source re-triggered generation.
        assert!(is_own_output(&config, &event(&["same.png", "same.css"])));
        assert!(is_own_output(&config, &event(&[".same.spriters"])));
        assert!(!is_own_output(&config, &event(&["icon.png"])));
        assert!(!is_own_output(&config, &event(&["same.css", "icon.png"])));
        assert!(!is_own_output(&config, &event(&[])));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_bytes_bytes_range() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_kb_range() {
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(1_047_552), "1023.0 KB");
    }

    #[test]
    fn test_format_bytes_mb_range() {
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(2_097_152), "2.0 MB");
    }
}
