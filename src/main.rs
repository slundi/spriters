use notify::Watcher;

pub mod cli;

fn main() -> Result<(), lexopt::Error> {
    let config = cli::parse_args()?;
    config.check()?;
    if config.watch {
        let mut watcher = notify::recommended_watcher(|res| match res {
            Ok(event) => todo!("event: {:?}", event),
            Err(e) => eprintln!("Watch error: {:?}", e),
        }).expect("Cannot create a watcher for the source directory");

        // Add a path to be watched. All files and directories at that path and below will be monitored for changes.
        watcher.watch(
            std::path::Path::new(&config.source.clone()),
            notify::RecursiveMode::Recursive,
        ).expect("Cannot watch the source directory.");
    }
    Ok(())
}
