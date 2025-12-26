//! CLI argument definitions
//!
//! Uses clap derive macros for argument parsing.

use clap::{Parser, Subcommand};

/// gLauncher - Lightweight Minecraft Java Edition Launcher
#[derive(Parser, Debug)]
#[command(name = "glauncher")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Launch a specific instance directly (skips GUI)
    #[arg(short, long)]
    pub instance: Option<String>,

    /// Run in offline mode (requires previous login)
    #[arg(long)]
    pub offline: bool,

    /// List all instances
    #[arg(short, long)]
    pub list: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new instance
    Create {
        /// Instance name
        name: String,
        /// Minecraft version
        #[arg(short, long)]
        version: String,
        /// Mod loader (vanilla, fabric, forge)
        #[arg(short, long, default_value = "vanilla")]
        loader: String,
    },

    /// Manage authentication
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Check for updates
    Update,
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Login with Microsoft account
    Login,
    /// Logout from current account
    Logout,
    /// Show authentication status
    Status,
}
