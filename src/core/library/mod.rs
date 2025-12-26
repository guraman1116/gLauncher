//! Library management
//!
//! Download and manage Minecraft libraries.

use crate::core::version::{Artifact, Library};
use crate::util::hash::verify_sha1;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Library manager for downloading and managing Minecraft libraries
pub struct LibraryManager {
    libraries_dir: PathBuf,
}

impl LibraryManager {
    pub fn new(libraries_dir: impl Into<PathBuf>) -> Self {
        Self {
            libraries_dir: libraries_dir.into(),
        }
    }

    /// Get the path to a library JAR
    pub fn get_library_path(&self, library: &Library) -> Option<PathBuf> {
        library.get_path().map(|p| self.libraries_dir.join(p))
    }

    /// Get the path to a native library JAR
    pub fn get_native_path(&self, library: &Library) -> Option<PathBuf> {
        library
            .get_native_artifact()
            .map(|a| self.libraries_dir.join(&a.path))
    }

    /// Check which libraries need to be downloaded
    pub fn get_missing_libraries<'a>(
        &self,
        libraries: &'a [Library],
    ) -> Vec<(&'a Library, &'a Artifact)> {
        libraries
            .iter()
            .filter(|lib| lib.should_include())
            .filter_map(|lib| {
                // Check main artifact
                if let Some(artifact) = lib.get_artifact() {
                    let path = self.libraries_dir.join(&artifact.path);
                    if !path.exists() || !verify_sha1(&path, &artifact.sha1).unwrap_or(false) {
                        return Some((lib, artifact));
                    }
                }
                None
            })
            .collect()
    }

    /// Check which native libraries need to be downloaded
    pub fn get_missing_natives<'a>(
        &self,
        libraries: &'a [Library],
    ) -> Vec<(&'a Library, &'a Artifact)> {
        libraries
            .iter()
            .filter(|lib| lib.should_include() && lib.natives.is_some())
            .filter_map(|lib| {
                if let Some(artifact) = lib.get_native_artifact() {
                    let path = self.libraries_dir.join(&artifact.path);
                    if !path.exists() || !verify_sha1(&path, &artifact.sha1).unwrap_or(false) {
                        return Some((lib, artifact));
                    }
                }
                None
            })
            .collect()
    }

    /// Download a single library
    pub async fn download_library(&self, artifact: &Artifact) -> Result<()> {
        let dest = self.libraries_dir.join(&artifact.path);

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        tracing::info!("Downloading library: {}", artifact.path);

        let response = reqwest::get(&artifact.url)
            .await
            .context("Failed to download library")?;
        let bytes = response.bytes().await?;

        std::fs::write(&dest, &bytes)?;

        // Verify SHA1
        if !verify_sha1(&dest, &artifact.sha1)? {
            std::fs::remove_file(&dest)?;
            anyhow::bail!("SHA1 mismatch for library: {}", artifact.path);
        }

        Ok(())
    }

    /// Download all missing libraries
    pub async fn download_all<F>(&self, libraries: &[Library], mut progress: F) -> Result<()>
    where
        F: FnMut(usize, usize, &str),
    {
        let missing = self.get_missing_libraries(libraries);
        let missing_natives = self.get_missing_natives(libraries);

        let total = missing.len() + missing_natives.len();
        let mut current = 0;

        // Download main libraries
        for (lib, artifact) in missing {
            progress(current, total, &lib.name);
            self.download_library(artifact).await?;
            current += 1;
        }

        // Download native libraries
        for (lib, artifact) in missing_natives {
            progress(current, total, &format!("{} (native)", lib.name));
            self.download_library(artifact).await?;
            current += 1;
        }

        Ok(())
    }

    /// Build classpath string from libraries
    pub fn build_classpath(&self, libraries: &[Library], game_jar: &Path) -> String {
        let separator = if cfg!(windows) { ";" } else { ":" };

        let mut paths: Vec<String> = libraries
            .iter()
            .filter(|lib| lib.should_include())
            .filter_map(|lib| {
                self.get_library_path(lib)
                    .map(|p| p.to_string_lossy().to_string())
            })
            .collect();

        // Add game JAR at the end
        paths.push(game_jar.to_string_lossy().to_string());

        paths.join(separator)
    }

    /// Extract native libraries to a directory
    pub fn extract_natives(&self, libraries: &[Library], natives_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(natives_dir)?;

        for lib in libraries
            .iter()
            .filter(|l| l.should_include() && l.natives.is_some())
        {
            if let Some(native_path) = self.get_native_path(lib) {
                if native_path.exists() {
                    tracing::info!("Extracting native: {:?}", native_path);

                    let file = std::fs::File::open(&native_path)?;
                    let mut archive = zip::ZipArchive::new(file)?;

                    // Get exclude patterns
                    let excludes = lib
                        .extract
                        .as_ref()
                        .and_then(|e| e.exclude.as_ref())
                        .map(|e| e.as_slice())
                        .unwrap_or(&[]);

                    for i in 0..archive.len() {
                        let mut entry = archive.by_index(i)?;
                        let name = entry.name().to_string();

                        // Skip excluded files
                        if excludes.iter().any(|e| name.starts_with(e)) {
                            continue;
                        }

                        // Skip directories and META-INF
                        if entry.is_dir() || name.starts_with("META-INF") {
                            continue;
                        }

                        let dest = natives_dir.join(&name);
                        if let Some(parent) = dest.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        let mut outfile = std::fs::File::create(&dest)?;
                        std::io::copy(&mut entry, &mut outfile)?;
                    }
                }
            }
        }

        Ok(())
    }
}
