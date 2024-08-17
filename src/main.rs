use log::*;
use notify::Watcher;

pub mod cli;

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
    if config.watch {
        let mut watcher = notify::recommended_watcher(|res| match res {
            Ok(event) => todo!("event: {:?}", event),
            Err(e) => error!("Watch error: {:?}", e),
        })
        .expect("Cannot create a watcher for the source directory");

        // Add a path to be watched. All files and directories at that path and below will be monitored for changes.
        watcher
            .watch(
                std::path::Path::new(&config.source.clone()),
                notify::RecursiveMode::Recursive,
            )
            .expect("Cannot watch the source directory.");
        return Ok(());
    }
    let mut manager = spriters::Manager {
        project: config.project,
        ..Default::default()
    };
    manager.find_sprites(&config);
    // manager.save(&config);
    Ok(())
}
