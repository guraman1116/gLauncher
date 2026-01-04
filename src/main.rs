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
        cli::run_instance(instance_name, args.offline).await?;
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
        } => cli::create_instance(&name, &version, &loader).await,
        Commands::Auth { action } => cli::handle_auth(action).await,
        Commands::Update => {
            use crate::core::update::UpdateManager;
            UpdateManager::update()?;
            Ok(())
        }
        Commands::Ps => {
            println!("Running instances:");
            println!("  (No running instances - CLI mode doesn't track other processes)");
            println!("\nNote: Use the GUI to view running instances and their logs.");
            Ok(())
        }
        Commands::Kill { name } => {
            println!("⚠️  Kill command currently only works in GUI mode.");
            println!("   Instance '{}' cannot be killed from CLI.", name);
            println!("\nNote: Use the GUI to manage running instances.");
            Ok(())
        }
        Commands::Logs {
            name,
            follow: _,
            lines: _,
        } => {
            println!("📋 Logs for '{}':", name);
            println!("   (Log viewing currently only available in GUI mode)");
            println!("\nNote: Use the GUI to view real-time logs.");
            Ok(())
        }
    }
}
