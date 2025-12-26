//! Utility module
//!
//! Common utilities used across the application.

pub mod download;
pub mod hash;

use std::path::PathBuf;

/// Get the data directory for gLauncher
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("glauncher")
}

/// Get the cache directory
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| data_dir())
        .join("glauncher")
}
