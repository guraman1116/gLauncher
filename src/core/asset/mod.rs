//! Asset management
//!
//! Download and manage Minecraft assets.

use crate::core::version::{AssetIndex, AssetIndexInfo, AssetObject};
use crate::util::hash::verify_sha1;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Asset manager for downloading and managing Minecraft assets
pub struct AssetManager {
    assets_dir: PathBuf,
}

impl AssetManager {
    pub fn new(assets_dir: impl Into<PathBuf>) -> Self {
        Self {
            assets_dir: assets_dir.into(),
        }
    }

    /// Get the indexes directory
    pub fn indexes_dir(&self) -> PathBuf {
        self.assets_dir.join("indexes")
    }

    /// Get the objects directory
    pub fn objects_dir(&self) -> PathBuf {
        self.assets_dir.join("objects")
    }

    /// Get path to asset index file
    pub fn get_index_path(&self, id: &str) -> PathBuf {
        self.indexes_dir().join(format!("{}.json", id))
    }

    /// Get path to asset object
    pub fn get_object_path(&self, object: &AssetObject) -> PathBuf {
        self.objects_dir().join(object.get_path())
    }

    /// Download asset index
    pub async fn download_index(&self, info: &AssetIndexInfo) -> Result<AssetIndex> {
        let index_path = self.get_index_path(&info.id);

        // Check if already exists and valid
        if index_path.exists() {
            if verify_sha1(&index_path, &info.sha1).unwrap_or(false) {
                let content = std::fs::read_to_string(&index_path)?;
                return Ok(serde_json::from_str(&content)?);
            }
        }

        // Download
        tracing::info!("Downloading asset index: {}", info.id);

        let response = reqwest::get(&info.url)
            .await
            .context("Failed to download asset index")?;
        let content = response.text().await?;

        // Save to disk
        std::fs::create_dir_all(self.indexes_dir())?;
        std::fs::write(&index_path, &content)?;

        let index: AssetIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    /// Load asset index from disk
    pub fn load_index(&self, id: &str) -> Result<AssetIndex> {
        let index_path = self.get_index_path(id);
        let content = std::fs::read_to_string(&index_path).context("Failed to read asset index")?;
        let index: AssetIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    /// Get missing assets
    pub fn get_missing_assets<'a>(
        &self,
        index: &'a AssetIndex,
    ) -> Vec<(&'a String, &'a AssetObject)> {
        index
            .objects
            .iter()
            .filter(|(_, obj)| {
                let path = self.get_object_path(obj);
                !path.exists() || !verify_sha1(&path, &obj.hash).unwrap_or(false)
            })
            .collect()
    }

    /// Download a single asset
    pub async fn download_asset(&self, object: &AssetObject) -> Result<()> {
        let dest = self.get_object_path(object);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let response = reqwest::get(object.get_url())
            .await
            .context("Failed to download asset")?;
        let bytes = response.bytes().await?;

        std::fs::write(&dest, &bytes)?;

        // Verify SHA1
        if !verify_sha1(&dest, &object.hash)? {
            std::fs::remove_file(&dest)?;
            anyhow::bail!("SHA1 mismatch for asset: {}", object.hash);
        }

        Ok(())
    }

    /// Download all missing assets with progress callback
    pub async fn download_all<F>(&self, index: &AssetIndex, mut progress: F) -> Result<()>
    where
        F: FnMut(usize, usize),
    {
        let missing = self.get_missing_assets(index);
        let total = missing.len();

        if total == 0 {
            tracing::info!("All assets already downloaded");
            return Ok(());
        }

        tracing::info!("Downloading {} assets...", total);

        for (i, (name, object)) in missing.iter().enumerate() {
            progress(i, total);

            if let Err(e) = self.download_asset(object).await {
                tracing::warn!("Failed to download asset {}: {}", name, e);
                // Continue with other assets
            }
        }

        progress(total, total);
        Ok(())
    }

    /// Get total size of missing assets
    pub fn get_missing_size(&self, index: &AssetIndex) -> u64 {
        self.get_missing_assets(index)
            .iter()
            .map(|(_, obj)| obj.size)
            .sum()
    }
}
