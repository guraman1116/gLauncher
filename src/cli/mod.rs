//! CLI module
//!
//! Command-line interface for gLauncher.

mod args;

pub use args::{Args, AuthAction, Commands};

use crate::core::auth::AccountManager;
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

/// Handle auth subcommands
pub async fn handle_auth(action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login => auth_login().await,
        AuthAction::Logout => auth_logout(),
        AuthAction::Status => auth_status(),
    }
}

/// Login with Microsoft account
async fn auth_login() -> Result<()> {
    println!("🔐 Starting Microsoft login...\n");

    let mut manager = AccountManager::new()?;

    // Start device code flow
    let device_code = manager.start_login().await?;

    // Display instructions to user
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  To sign in, use a web browser to open the page:");
    println!("  \x1b[1;36m{}\x1b[0m", device_code.verification_uri);
    println!();
    println!("  And enter the code:");
    println!("  \x1b[1;33m{}\x1b[0m", device_code.user_code);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("Waiting for authentication...");

    // Complete authentication
    let account = manager.complete_login(&device_code).await?;

    println!();
    println!(
        "✅ Successfully logged in as: \x1b[1;32m{}\x1b[0m",
        account.profile.name
    );
    println!("   UUID: {}", account.profile.id);

    Ok(())
}

/// Logout from account
fn auth_logout() -> Result<()> {
    let mut manager = AccountManager::new()?;

    if manager.accounts().is_empty() {
        println!("No accounts to logout.");
        return Ok(());
    }

    // For now, logout all accounts
    manager.logout_all()?;
    println!("✅ Logged out from all accounts.");
    Ok(())
}

/// Show authentication status
fn auth_status() -> Result<()> {
    let manager = AccountManager::new()?;
    let accounts = manager.accounts();

    if accounts.is_empty() {
        println!("👤 No accounts linked.");
        println!("   Use 'glauncher auth login' to add an account.");
        return Ok(());
    }

    println!("👤 Accounts:");
    for account in accounts {
        let active = if account.is_active { " (active)" } else { "" };
        println!(
            "   {} - {}{}",
            account.profile.name, account.profile.id, active
        );
    }

    Ok(())
}
