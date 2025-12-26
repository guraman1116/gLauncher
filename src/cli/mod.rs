//! CLI module
//!
//! Command-line interface for gLauncher.

mod args;

pub use args::Args;

use anyhow::Result;

/// Launch a specific instance directly
pub fn run_instance(name: &str, offline: bool) -> Result<()> {
    tracing::info!("Running instance '{}' (offline: {})", name, offline);

    // TODO: Implement instance launching
    // 1. Load instance config
    // 2. Verify game files
    // 3. Start Minecraft process

    println!("🚀 Launching instance: {}", name);
    if offline {
        println!("   Mode: Offline");
    }

    anyhow::bail!("Instance launching not yet implemented")
}

/// List all available instances
pub fn list_instances() -> Result<()> {
    tracing::info!("Listing instances");

    // TODO: Implement instance listing
    // 1. Read instances directory
    // 2. Parse instance configs
    // 3. Display list

    println!("📦 Instances:");
    println!("   (No instances found)");

    Ok(())
}
