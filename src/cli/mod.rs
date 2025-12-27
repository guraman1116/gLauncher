//! CLI module
//!
//! Command-line interface for gLauncher.

mod args;

pub use args::{Args, AuthAction, Commands};

use crate::core::auth::{AccountManager, AccountType};
use crate::core::instance::{InstanceManager, ModLoader};
use crate::core::launch::{LaunchResult, launch_instance_async};
use anyhow::{Context, Result};

/// Launch a specific instance directly
pub async fn run_instance(name: &str, _offline: bool) -> Result<()> {
    tracing::info!("Running instance '{}'", name);

    let instance_manager = InstanceManager::new();
    let account_manager = AccountManager::new()?;

    // Load instance
    let instance = instance_manager
        .load(name)
        .context(format!("Instance '{}' not found", name))?;

    // Get active account
    let account = account_manager
        .active_account()
        .context("No active account. Use 'glauncher auth login' first.")?
        .clone();

    println!("🚀 Launching instance: {}", name);
    println!("   Version: {}", instance.info.version);
    println!("   Loader: {}", instance.info.loader);
    println!(
        "   Account: {} ({})",
        account.profile.name,
        if account.account_type == AccountType::Offline {
            "Offline"
        } else {
            "Microsoft"
        }
    );

    // Use shared launch logic
    match launch_instance_async(&instance, &account, |msg| {
        println!("   {}", msg);
    })
    .await?
    {
        LaunchResult::Success(mut child) => {
            println!("✅ Minecraft started (PID: {:?})", child.id());
            // Wait for process
            let _ = child.wait()?;
        }
        LaunchResult::EarlyExit(code) => {
            println!("❌ Minecraft exited early with code: {:?}", code);
            println!("   Check the terminal output for error details.");
        }
    }

    Ok(())
}

/// List all available instances
pub fn list_instances() -> Result<()> {
    let instance_manager = InstanceManager::new();
    let instances = instance_manager.list()?;

    if instances.is_empty() {
        println!("📦 No instances found.");
        println!("   Use 'glauncher create <name> --version <ver>' to create one.");
        return Ok(());
    }

    println!("📦 Instances ({}):", instances.len());
    println!();

    for instance in &instances {
        let loader = format!("{}", instance.info.loader);
        println!(
            "   {} - {} ({})",
            instance.info.name, instance.info.version, loader
        );
    }

    Ok(())
}

/// Create a new instance from CLI
pub async fn create_instance(name: &str, version: &str, loader: &str) -> Result<()> {
    let instance_manager = InstanceManager::new();

    let mod_loader = match loader.to_lowercase().as_str() {
        "vanilla" => ModLoader::Vanilla,
        "fabric" => ModLoader::Fabric,
        "forge" => ModLoader::Forge,
        _ => anyhow::bail!("Unknown loader: {}. Use vanilla, fabric, or forge.", loader),
    };

    println!("📦 Creating instance '{}'...", name);
    println!("   Version: {}", version);
    println!("   Loader: {}", loader);

    instance_manager.create(name, version, mod_loader, None)?;

    println!("✅ Instance '{}' created successfully!", name);
    println!("   Use 'glauncher -i {}' to launch.", name);

    Ok(())
}

/// Add an offline account
pub fn add_offline_account(username: &str) -> Result<()> {
    let mut manager = AccountManager::new()?;

    let account = manager.add_offline_account(username)?;

    println!("✅ Added offline account: {}", account.profile.name);
    println!("   UUID: {}", account.profile.id);
    println!("   Note: Offline accounts cannot join multiplayer servers.");

    Ok(())
}

/// Handle auth subcommands
pub async fn handle_auth(action: AuthAction) -> Result<()> {
    match action {
        AuthAction::Login => auth_login().await,
        AuthAction::Offline { username } => add_offline_account(&username),
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
        println!("   Use 'glauncher auth login' to add a Microsoft account.");
        println!("   Use 'glauncher auth offline <username>' to add an offline account.");
        return Ok(());
    }

    println!("👤 Accounts ({}):", accounts.len());
    for account in accounts {
        let active = if account.is_active { " ✓ active" } else { "" };
        let acc_type = match account.account_type {
            AccountType::Microsoft => "🔐 Microsoft",
            AccountType::Offline => "👤 Offline",
        };
        println!(
            "   {} {} ({}){}",
            acc_type, account.profile.name, account.profile.id, active
        );
    }

    Ok(())
}
