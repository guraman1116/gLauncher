//! Configuration schema
//!
//! Defines the structure of the configuration file.

use serde::{Deserialize, Serialize};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub java: JavaConfig,

    #[serde(default)]
    pub network: NetworkConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            java: JavaConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

/// General launcher settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// UI theme (dark/light)
    #[serde(default = "default_theme")]
    pub theme: String,

    /// UI language
    #[serde(default = "default_language")]
    pub language: String,

    /// Check for updates on startup
    #[serde(default = "default_true")]
    pub check_updates: bool,

    /// Close launcher after game starts
    #[serde(default)]
    pub close_on_launch: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            check_updates: true,
            close_on_launch: false,
        }
    }
}

/// Java runtime settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaConfig {
    /// Path to Java executable (empty = auto-detect)
    #[serde(default)]
    pub path: String,

    /// Minimum memory allocation
    #[serde(default = "default_min_memory")]
    pub min_memory: String,

    /// Maximum memory allocation
    #[serde(default = "default_max_memory")]
    pub max_memory: String,

    /// Extra JVM arguments
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl Default for JavaConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            min_memory: default_min_memory(),
            max_memory: default_max_memory(),
            extra_args: Vec::new(),
        }
    }
}

/// Network settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Proxy URL (empty = no proxy)
    #[serde(default)]
    pub proxy: String,

    /// Number of concurrent downloads
    #[serde(default = "default_concurrent_downloads")]
    pub concurrent_downloads: u32,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy: String::new(),
            concurrent_downloads: default_concurrent_downloads(),
            timeout_seconds: default_timeout(),
        }
    }
}

// Default value functions for serde
fn default_theme() -> String {
    "dark".to_string()
}
fn default_language() -> String {
    "ja".to_string()
}
fn default_true() -> bool {
    true
}
fn default_min_memory() -> String {
    "512M".to_string()
}
fn default_max_memory() -> String {
    "4G".to_string()
}
fn default_concurrent_downloads() -> u32 {
    4
}
fn default_timeout() -> u64 {
    30
}
