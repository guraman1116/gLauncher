//! Mod download and installation
//!
//! Handles downloading and installing mods to instance mods folders.

use super::search::ModVersion;
use anyhow::{Context, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Mod downloader
pub struct ModDownloader {
    client: Client,
}

impl ModDownloader {
    /// Create a new downloader
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("gLauncher/0.1.1")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Download a mod to the specified mods directory
    pub async fn download_mod(
        &self,
        version: &ModVersion,
        mods_dir: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        // Ensure mods directory exists
        tokio::fs::create_dir_all(mods_dir)
            .await
            .context("Failed to create mods directory")?;

        let target_path = mods_dir.join(&version.filename);

        // Check if already exists
        if target_path.exists() {
            tracing::info!("Mod already exists: {}", version.filename);
            return Ok(target_path);
        }

        tracing::info!(
            "Downloading mod: {} -> {}",
            version.filename,
            target_path.display()
        );

        // Start download
        let response = self
            .client
            .get(&version.download_url)
            .send()
            .await
            .context("Failed to start download")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Download failed with status: {} for URL: {}",
                response.status(),
                version.download_url
            );
        }

        let total_size = response.content_length().unwrap_or(version.file_size);

        // Create temp file
        let temp_path = mods_dir.join(format!("{}.tmp", version.filename));
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .context("Failed to create temp file")?;

        let mut downloaded: u64 = 0;

        // Download the full response body
        let bytes = response.bytes().await.context("Failed to download mod")?;
        downloaded = bytes.len() as u64;

        // Write to file
        file.write_all(&bytes)
            .await
            .context("Failed to write mod file")?;

        if let Some(ref callback) = progress {
            callback(downloaded, downloaded);
        }

        file.flush().await.context("Failed to flush file")?;
        drop(file);

        // Rename temp file to final name
        tokio::fs::rename(&temp_path, &target_path)
            .await
            .context("Failed to rename temp file")?;

        tracing::info!("Downloaded mod: {}", version.filename);

        Ok(target_path)
    }

    /// Check if a mod is already installed
    pub fn is_installed(&self, filename: &str, mods_dir: &Path) -> bool {
        let path = mods_dir.join(filename);
        path.exists()
    }

    /// Check if a mod with matching filename exists (enabled or disabled)
    pub fn find_existing(&self, filename: &str, mods_dir: &Path) -> Option<PathBuf> {
        let enabled_path = mods_dir.join(filename);
        if enabled_path.exists() {
            return Some(enabled_path);
        }

        let disabled_path = mods_dir.join(format!("{}.disabled", filename));
        if disabled_path.exists() {
            return Some(disabled_path);
        }

        None
    }
}

impl Default for ModDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Format download progress for display
pub fn format_progress(downloaded: u64, total: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    let format_size = |bytes: u64| -> String {
        if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.0} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    };

    if total > 0 {
        let percent = (downloaded as f64 / total as f64 * 100.0) as u32;
        format!(
            "{} / {} ({}%)",
            format_size(downloaded),
            format_size(total),
            percent
        )
    } else {
        format_size(downloaded)
    }
}
