use log::*;

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
    let mut manager = spriters::Manager {
        project: config.project,
        config,
        ..Default::default()
    };
    manager.find_sprites();
    manager.save();
    Ok(())
}
