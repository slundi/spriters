use log::*;
use notify::{Event, RecursiveMode, Watcher};
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
    manager.save();
}

fn main() -> Result<(), lexopt::Error> {
    let config = cli::parse_args()?;
    config.check()?;
    stderrlog::new()
        .module(module_path!())
        .verbosity(log::Level::Info)
        .quiet(config.quiet)
        .init()
        .unwrap();
    info!("Input directory: {}", config.source.to_string_lossy());
    info!("Output directory: {}", config.output.to_string_lossy());

    // Initial sprite generation
    run_sprite_generation(&config);

    // If watch mode is enabled, start watching for changes
    if config.watch {
        info!("Watching for changes in {:?}... (Ctrl+C to stop)", config.source);

        let (tx, rx) = channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                // Only trigger on file modifications, creations, or deletions
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    let _ = tx.send(());
                }
            }
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&config.source, RecursiveMode::Recursive)
            .expect("Failed to watch source directory");

        // Debounce: wait for changes to settle before regenerating
        let debounce_duration = Duration::from_millis(500);

        loop {
            // Wait for a change event
            if rx.recv().is_ok() {
                // Drain any additional events that arrived (debouncing)
                while rx.recv_timeout(debounce_duration).is_ok() {}

                info!("Change detected, regenerating sprites...");
                run_sprite_generation(&config);
                info!("Watching for changes... (Ctrl+C to stop)");
            }
        }
    }

    Ok(())
}
