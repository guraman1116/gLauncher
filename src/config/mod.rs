//! Configuration module
//!
//! Handles loading and saving launcher configuration.

mod schema;

pub use schema::{Config, GeneralConfig, JavaConfig, NetworkConfig};

use anyhow::Result;
use std::path::PathBuf;

/// Get the configuration directory path
pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".glauncher")
}

/// Get the config file path
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Load configuration from disk
pub fn load() -> Result<Config> {
    let path = config_path();

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        // Create default config
        let config = Config::default();
        save(&config)?;
        Ok(config)
    }
}

/// Save configuration to disk
pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    let dir = config_dir();

    // Ensure config directory exists
    std::fs::create_dir_all(&dir)?;

    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;

    tracing::info!("Configuration saved to {:?}", path);
    Ok(())
}
