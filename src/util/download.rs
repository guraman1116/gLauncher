//! Download utilities
//!
//! Async file downloading with progress.

use anyhow::Result;
use std::path::Path;

/// Download a file to the specified path
pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(dest, &bytes)?;

    Ok(())
}

/// Download multiple files concurrently
pub async fn download_files(
    downloads: Vec<(String, std::path::PathBuf)>,
    concurrent: usize,
) -> Result<()> {
    use futures::stream::{self, StreamExt};

    stream::iter(downloads)
        .map(|(url, path)| async move { download_file(&url, &path).await })
        .buffer_unordered(concurrent)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}
