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

/// Print the stats of every sprite in `output_dir`. Fails when one of them cannot be read.
fn handle_stats(output_dir: Option<&std::path::Path>) -> ExitCode {
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

    let mut status = ExitCode::SUCCESS;
    for name in &names {
        match spriters::sprite_stats(dir, name) {
            Ok(s) => {
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
            Err(err) => {
                eprintln!("Sprite '{name}': {err}");
                status = ExitCode::FAILURE;
            }
        }
    }
    status
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

/// What a watcher notification asks for.
#[derive(Debug, PartialEq, Eq)]
enum WatchAction {
    /// Nothing: an error (logged), an access, or a change of spriters' own outputs.
    Ignore,
    /// Regenerate; `source_removed` when the watched directory itself was removed (its
    /// watch is gone with it).
    Regenerate { source_removed: bool },
}

/// Decide what to do with a notification of the watcher on `source`.
fn watch_action(
    config: &spriters::Config,
    source: &std::path::Path,
    notification: Result<Event, notify::Error>,
) -> WatchAction {
    let event = match notification {
        Ok(event) => event,
        Err(e) => {
            error!("File watcher error: {e}");
            return WatchAction::Ignore;
        }
    };
    // Events were lost (e.g. queue overflow): regenerate to catch up.
    if event.need_rescan() {
        return WatchAction::Regenerate {
            source_removed: false,
        };
    }
    if !(event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
        || is_own_output(config, &event)
    {
        return WatchAction::Ignore;
    }
    WatchAction::Regenerate {
        source_removed: event.kind.is_remove() && event.paths.iter().any(|path| path == source),
    }
}

/// Run `generation`; a panic is reported as a failed run instead of ending `--watch`.
fn catch_panic(generation: impl FnOnce() -> bool) -> bool {
    // The panic message is already printed by the panic hook. Rayon forwards the panics of
    // its worker threads to the caller, so they are caught here too.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(generation)).unwrap_or_else(|_| {
        error!("Sprite generation failed with an internal error, still watching for changes");
        false
    })
}

/// Wait until `source` is a directory again, then watch it: the watch of a removed
/// directory does not survive it, even when it is created again.
fn rewatch(watcher: &mut impl Watcher, source: &std::path::Path, poll: Duration) {
    let mut reported = false;
    loop {
        if source.is_dir() {
            // Fails when the old watch already went away with the directory.
            watcher.unwatch(source).ok();
            match watcher.watch(source, RecursiveMode::Recursive) {
                Ok(()) => return,
                Err(e) => warn!("Cannot watch {:?} yet: {}", source, e),
            }
        } else if !reported {
            warn!(
                "Source directory {:?} was removed, waiting for it to be created again",
                source
            );
            reported = true;
        }
        std::thread::sleep(poll);
    }
}

/// Regenerate the sprites on every change in the source directory, until the process is
/// stopped.
fn watch(config: &spriters::Config, cache: Option<std::sync::Arc<spriters::ImageCache>>) -> ! {
    // Canonical, so that removal events of the directory itself can be recognised.
    let source = config
        .source
        .canonicalize()
        .unwrap_or_else(|_| config.source.clone());
    info!("Watching for changes in {:?}... (Ctrl+C to stop)", source);

    let (tx, rx) = channel();
    let watch_config = config.clone();
    let watched = source.clone();
    let mut watcher = notify::recommended_watcher(move |notification| {
        if let WatchAction::Regenerate { source_removed } =
            watch_action(&watch_config, &watched, notification)
        {
            let _ = tx.send(source_removed);
        }
    })
    .expect("Failed to create file watcher");
    watcher
        .watch(&source, RecursiveMode::Recursive)
        .expect("Failed to watch source directory");

    let debounce_duration = Duration::from_millis(500);
    loop {
        // The watcher (and its sender) lives as long as this loop.
        let Ok(mut source_removed) = rx.recv() else {
            unreachable!("the file watcher stopped");
        };
        while let Ok(removed) = rx.recv_timeout(debounce_duration) {
            source_removed |= removed;
        }
        if source_removed || !source.is_dir() {
            // Not regenerated meanwhile: it would replace the outputs with empty ones.
            rewatch(&mut watcher, &source, debounce_duration);
            while rx.try_recv().is_ok() {}
        }
        info!("Change detected, regenerating sprites...");
        catch_panic(|| run_sprite_generation(config, cache.clone()));
        if let Some(cache) = &cache {
            cache.retain_existing();
        }
        info!("Watching for changes... (Ctrl+C to stop)");
    }
}

/// A subcommand and its argument.
#[derive(Debug, PartialEq, Eq)]
enum Subcommand<'a> {
    Stats(Option<&'a std::path::Path>),
    Completions(Option<&'a str>),
}

/// The subcommand of the command line `args` (program name first), if any.
///
/// Only the first argument is a subcommand (as with `git`): a source directory with the
/// same name is given as a path (`./stats`) or after `--`, or after any option.
fn subcommand(args: &[std::ffi::OsString]) -> Option<Subcommand<'_>> {
    let argument = args.get(2);
    match args.get(1)?.to_str()? {
        "stats" => Some(Subcommand::Stats(argument.map(std::path::Path::new))),
        "completions" => Some(Subcommand::Completions(
            argument.and_then(|arg| arg.to_str()),
        )),
        _ => None,
    }
}

/// The note printed when the subcommand `name` runs while a directory of that name exists
/// in the working directory: it was maybe meant as the source directory.
fn same_named_directory_hint(name: &str) -> Option<String> {
    std::path::Path::new(name).is_dir().then(|| {
        format!(
            "Note: running the '{name}' subcommand. To build a sprite from the directory '{name}', \
             write it as ./{name} or after '--' (spriters -- {name} ...)."
        )
    })
}

fn hint_same_named_directory(name: &str) {
    if let Some(hint) = same_named_directory_hint(name) {
        eprintln!("{hint}");
    }
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
    match subcommand(&raw) {
        Some(Subcommand::Stats(output_dir)) => {
            hint_same_named_directory("stats");
            return handle_stats(output_dir);
        }
        Some(Subcommand::Completions(shell)) => {
            hint_same_named_directory("completions");
            handle_completions(shell);
            return ExitCode::SUCCESS;
        }
        None => {}
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
    if config.image_dir() == config.css_dir() {
        info!("Output directory: {}", config.output.to_string_lossy());
    } else {
        info!("Image directory: {}", config.image_dir().to_string_lossy());
        info!("CSS directory: {}", config.css_dir().to_string_lossy());
    }

    // In watch mode, decoded images are kept between runs: a change only re-decodes the
    // files that changed.
    let cache = config
        .watch
        .then(|| std::sync::Arc::new(spriters::ImageCache::default()));

    // In watch mode, a failed run (even a panic) is fixed by the next change: keep watching.
    if config.watch {
        catch_panic(|| run_sprite_generation(&config, cache.clone()));
        watch(&config, cache);
    }

    // Errors are already logged; the exit code reports them, so that CI does not pass on a
    // broken sprite.
    let succeeded = run_sprite_generation(&config, cache);

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
    fn test_watch_action() {
        use super::{WatchAction, watch_action};
        use notify::event::{AccessKind, CreateKind, Flag, RemoveKind};

        let dir = std::env::temp_dir().join(format!("spriters_action_{}", std::process::id()));
        let source = dir.join("icons");
        let output = dir.join("out");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        let config = spriters::Config {
            source: source.clone(),
            output: output.clone(),
            ..Default::default()
        };
        let action = |event: Event| watch_action(&config, &source, Ok(event));
        let changed = WatchAction::Regenerate {
            source_removed: false,
        };

        let created =
            Event::new(EventKind::Create(CreateKind::File)).add_path(source.join("a.png"));
        assert_eq!(action(created), changed);
        let removed_file =
            Event::new(EventKind::Remove(RemoveKind::File)).add_path(source.join("a.png"));
        assert_eq!(action(removed_file), changed);
        // Review 6.9: removing the source directory itself kills its watch.
        let removed_source =
            Event::new(EventKind::Remove(RemoveKind::Folder)).add_path(source.clone());
        assert_eq!(
            action(removed_source),
            WatchAction::Regenerate {
                source_removed: true
            }
        );
        // Lost events: regenerate to catch up.
        let rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        assert_eq!(action(rescan), changed);

        let read = Event::new(EventKind::Access(AccessKind::Read)).add_path(source.join("a.png"));
        assert_eq!(action(read), WatchAction::Ignore);
        let own = Event::new(EventKind::Modify(ModifyKind::Any)).add_path(output.join("icons.css"));
        assert_eq!(action(own), WatchAction::Ignore);
        // Review 6.9: watcher errors were dropped without a message; now logged and ignored.
        let error = watch_action(&config, &source, Err(notify::Error::generic("lost")));
        assert_eq!(error, WatchAction::Ignore);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_subcommand_only_in_first_position() {
        use super::{Subcommand, subcommand};
        use std::path::Path;

        let args = |list: &[&str]| -> Vec<std::ffi::OsString> {
            std::iter::once("spriters")
                .chain(list.iter().copied())
                .map(std::ffi::OsString::from)
                .collect()
        };
        assert_eq!(
            subcommand(&args(&["stats", "dist"])),
            Some(Subcommand::Stats(Some(Path::new("dist"))))
        );
        assert_eq!(subcommand(&args(&["stats"])), Some(Subcommand::Stats(None)));
        assert_eq!(
            subcommand(&args(&["completions", "zsh"])),
            Some(Subcommand::Completions(Some("zsh")))
        );
        // R source directory named `stats` must stay reachable.
        for list in [
            &["--", "stats", "dist"][..],
            &["./stats", "dist"],
            &["-o", "dist", "stats"],
            &["icons", "stats"],
            &["Stats"],
            &[],
        ] {
            assert_eq!(subcommand(&args(list)), None, "{list:?}");
        }
    }

    #[test]
    fn test_catch_panic_keeps_watching() {
        use super::catch_panic;
        use rayon::prelude::*;

        // A panic during a regeneration ended `--watch`.
        assert!(!catch_panic(|| panic!("broken image")));
        // Panics of rayon worker threads reach the caller too.
        assert!(!catch_panic(|| {
            (0..8)
                .into_par_iter()
                .for_each(|index| assert_ne!(index, 5));
            true
        }));
        assert!(catch_panic(|| true));
        assert!(!catch_panic(|| false));
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
