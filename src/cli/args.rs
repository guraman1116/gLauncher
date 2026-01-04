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

    /// List running instances
    Ps,

    /// Kill a running instance
    Kill {
        /// Instance name to kill
        name: String,
    },

    /// Show logs for a running instance
    Logs {
        /// Instance name
        name: String,
        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show (default: 50)
        #[arg(short, long, default_value = "50")]
        lines: usize,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Login with Microsoft account
    Login,
    /// Add an offline account
    Offline {
        /// Username for offline mode
        username: String,
    },
    /// Logout from current account
    Logout,
    /// Show authentication status
    Status,
}
