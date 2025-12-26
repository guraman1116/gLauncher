//! gLauncher - Lightweight Minecraft Java Edition Launcher
//!
//! Entry point for CLI and GUI modes.

mod cli;
mod config;
mod core;
mod gui;
mod util;

use clap::Parser;
use cli::Args;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    if let Some(instance_name) = &args.instance {
        // CLI mode: Launch instance directly
        tracing::info!("Launching instance: {}", instance_name);
        cli::run_instance(instance_name, args.offline)?;
    } else if args.list {
        // List instances
        cli::list_instances()?;
    } else {
        // GUI mode: Start the launcher UI
        tracing::info!("Starting gLauncher GUI");
        gui::run()?;
    }

    Ok(())
}
