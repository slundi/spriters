use log::*;
use notify::{Event, RecursiveMode, Watcher};
use std::io::{BufRead, IsTerminal};
use std::sync::mpsc::channel;
use std::time::Duration;

pub mod cli;

fn run_sprite_generation(config: &spriters::Config) {
    let mut manager = spriters::Manager {
        project: config.project,
        config: config.clone(),
        ..Default::default()
    };
    manager.find_sprites();

    if config.check {
        if manager.is_up_to_date() {
            info!("All sprites are up to date.");
        } else {
            eprintln!(
                "One or more sprites are out of date. Run spriters without --check to regenerate."
            );
            std::process::exit(1);
        }
        return;
    }

    manager.save();
}

fn handle_stats(output_dir: Option<&str>) {
    let dir = std::path::Path::new(output_dir.unwrap_or("."));
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

fn format_bytes(n: u64) -> String {
    if n < 1_024 {
        format!("{} B", n)
    } else if n < 1_048_576 {
        format!("{:.1} KB", n as f64 / 1_024.0)
    } else {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

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

fn main() -> Result<(), lexopt::Error> {
    // Detect subcommands before full argument parsing.
    let raw: Vec<String> = std::env::args().collect();
    match raw.get(1).map(String::as_str) {
        Some("stats") => {
            handle_stats(raw.get(2).map(String::as_str));
            return Ok(());
        }
        Some("completions") => {
            handle_completions(raw.get(2).map(String::as_str));
            return Ok(());
        }
        _ => {}
    }

    let mut config = cli::parse_args()?;

    // When stdin is a pipe, read newline-delimited image paths from it.
    // This lets spriters compose with other tools: `fd -e png . | spriters -o dist/`
    if !std::io::stdin().is_terminal() {
        for line in std::io::stdin().lock().lines() {
            if let Ok(line) = line {
                let line = line.trim().to_owned();
                if !line.is_empty() {
                    config.files.push(std::path::PathBuf::from(line));
                }
            }
        }
    }

    spriters::config::apply_env_vars(&mut config);
    if let Some(fc) = spriters::config::load_file_config(&config.source) {
        spriters::config::apply_file_config(&mut config, fc);
    }
    config.check()?;
    stderrlog::new()
        .module(module_path!())
        .verbosity(log::Level::Info)
        .quiet(config.quiet)
        .init()
        .unwrap();
    if config.files.is_empty() {
        info!("Input directory: {}", config.source.to_string_lossy());
    } else {
        info!("Reading {} files from stdin", config.files.len());
    }
    info!("Output directory: {}", config.output.to_string_lossy());

    // Initial sprite generation
    run_sprite_generation(&config);

    // If watch mode is enabled, start watching for changes
    if config.watch {
        info!(
            "Watching for changes in {:?}... (Ctrl+C to stop)",
            config.source
        );

        let (tx, rx) = channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
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
                run_sprite_generation(&config);
                info!("Watching for changes... (Ctrl+C to stop)");
            }
        }
    }

    Ok(())
}
