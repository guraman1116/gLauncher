//! gLauncher - Lightweight Minecraft Java Edition Launcher
//!
//! Entry point for CLI and GUI modes.

mod cli;
mod config;
mod core;
mod gui;
mod util;

use clap::Parser;
use cli::{Args, Commands};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Handle subcommands first
    if let Some(command) = args.command {
        return handle_command(command).await;
    }

    // Handle CLI flags
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

async fn handle_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Create {
            name,
            version,
            loader,
        } => {
            println!(
                "Creating instance '{}' with version {} ({})",
                name, version, loader
            );
            // TODO: Implement instance creation
            anyhow::bail!("Instance creation not yet implemented")
        }
        Commands::Auth { action } => cli::handle_auth(action).await,
        Commands::Update => {
            println!("Checking for updates...");
            // TODO: Implement update check
            println!("gLauncher is up to date.");
            Ok(())
        }
    }
}
