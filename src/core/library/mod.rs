//! Library management
//!
//! Download and manage Minecraft libraries.

use crate::core::version::{Artifact, Library};
use crate::util::hash::verify_sha1;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    /// If skip_verification is true, only check file existence (faster)
    pub fn get_missing_libraries<'a>(
        &self,
        libraries: &'a [Library],
        skip_verification: bool,
    ) -> Vec<(&'a Library, &'a Artifact)> {
        libraries
            .iter()
            .filter(|lib| lib.should_include())
            .filter_map(|lib| {
                // Check main artifact
                if let Some(artifact) = lib.get_artifact() {
                    let path = self.libraries_dir.join(&artifact.path);
                    if !path.exists() {
                        return Some((lib, artifact));
                    }
                    // Skip SHA1 verification if requested (fast mode)
                    if skip_verification {
                        return None;
                    }
                    // Only verify SHA1 if it's not empty (Fabric libs don't have SHA1)
                    if !artifact.sha1.is_empty()
                        && !verify_sha1(&path, &artifact.sha1).unwrap_or(false)
                    {
                        return Some((lib, artifact));
                    }
                }
                None
            })
            .collect()
    }

    /// Check which native libraries need to be downloaded
    /// If skip_verification is true, only check file existence (faster)
    pub fn get_missing_natives<'a>(
        &self,
        libraries: &'a [Library],
        skip_verification: bool,
    ) -> Vec<(&'a Library, &'a Artifact)> {
        libraries
            .iter()
            .filter(|lib| lib.should_include() && lib.natives.is_some())
            .filter_map(|lib| {
                if let Some(artifact) = lib.get_native_artifact() {
                    let path = self.libraries_dir.join(&artifact.path);
                    if !path.exists() {
                        return Some((lib, artifact));
                    }
                    // Skip SHA1 verification if requested (fast mode)
                    if skip_verification {
                        return None;
                    }
                    // Only verify SHA1 if it's not empty
                    if !artifact.sha1.is_empty()
                        && !verify_sha1(&path, &artifact.sha1).unwrap_or(false)
                    {
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

        // Verify SHA1 (skip if empty - Fabric libraries don't have SHA1)
        if !artifact.sha1.is_empty() && !verify_sha1(&dest, &artifact.sha1)? {
            std::fs::remove_file(&dest)?;
            anyhow::bail!("SHA1 mismatch for library: {}", artifact.path);
        }

        Ok(())
    }

    /// Download all missing libraries (parallel)
    /// If skip_verification is true, only check file existence (faster for 2nd+ launches)
    pub async fn download_all<F>(
        &self,
        libraries: &[Library],
        skip_verification: bool,
        mut progress: F,
    ) -> Result<()>
    where
        F: FnMut(usize, usize, &str) + Send,
    {
        let missing = self.get_missing_libraries(libraries, skip_verification);
        let missing_natives = self.get_missing_natives(libraries, skip_verification);

        let total = missing.len() + missing_natives.len();

        if total == 0 {
            return Ok(());
        }

        // Combine all artifacts to download
        let all_downloads: Vec<(&Library, &Artifact)> = missing
            .into_iter()
            .chain(missing_natives.into_iter())
            .collect();

        // Use atomic counter for progress
        let completed = Arc::new(AtomicUsize::new(0));
        let libraries_dir = self.libraries_dir.clone();

        // Number of concurrent downloads
        const CONCURRENT_DOWNLOADS: usize = 8;

        // Report initial progress
        progress(0, total, "Starting downloads...");

        // Create download futures
        let download_results: Vec<Result<String>> = stream::iter(all_downloads)
            .map(|(lib, artifact)| {
                let libraries_dir = libraries_dir.clone();
                let completed = Arc::clone(&completed);
                let lib_name = lib.name.clone();
                let artifact_path = artifact.path.clone();
                let artifact_url = artifact.url.clone();
                let artifact_sha1 = artifact.sha1.clone();

                async move {
                    let dest = libraries_dir.join(&artifact_path);

                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    let response = reqwest::get(&artifact_url)
                        .await
                        .context("Failed to download library")?;
                    let bytes = response.bytes().await?;
                    std::fs::write(&dest, &bytes)?;

                    // Verify SHA1 if provided
                    if !artifact_sha1.is_empty() && !verify_sha1(&dest, &artifact_sha1)? {
                        std::fs::remove_file(&dest)?;
                        anyhow::bail!("SHA1 mismatch for library: {}", artifact_path);
                    }

                    completed.fetch_add(1, Ordering::SeqCst);
                    Ok(lib_name)
                }
            })
            .buffer_unordered(CONCURRENT_DOWNLOADS)
            .collect()
            .await;

        // Report final progress
        let final_count = completed.load(Ordering::SeqCst);
        progress(final_count, total, "Downloads complete");

        // Check for errors
        for result in download_results {
            result?;
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
        // Clean and recreate natives directory
        if natives_dir.exists() {
            std::fs::remove_dir_all(natives_dir)?;
        }
        std::fs::create_dir_all(natives_dir)?;

        // Determine current platform native suffix
        let native_suffix = if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "natives-macos-arm64"
            } else {
                "natives-macos"
            }
        } else if cfg!(target_os = "windows") {
            if cfg!(target_arch = "x86_64") {
                "natives-windows"
            } else {
                "natives-windows-x86"
            }
        } else {
            "natives-linux"
        };

        println!("Looking for natives with suffix: {}", native_suffix);

        for lib in libraries.iter().filter(|l| l.should_include()) {
            // Method 1: Traditional natives field
            if lib.natives.is_some() {
                if let Some(native_path) = self.get_native_path(lib) {
                    if native_path.exists() {
                        self.extract_native_jar(&native_path, natives_dir, lib)?;
                    }
                }
            }

            // Method 2: Modern format - library name contains natives
            if lib.name.contains("natives") && lib.name.contains(native_suffix) {
                if let Some(lib_path) = self.get_library_path(lib) {
                    if lib_path.exists() {
                        println!("Extracting modern native: {:?}", lib_path);
                        self.extract_native_jar(&lib_path, natives_dir, lib)?;
                    }
                }
            }
        }

        // List extracted files
        if let Ok(entries) = std::fs::read_dir(natives_dir) {
            let count = entries.filter_map(|e| e.ok()).count();
            println!("Extracted {} native files to {:?}", count, natives_dir);
        }

        Ok(())
    }

    /// Extract a native JAR file, flattening .dylib/.dll/.so files to the root
    fn extract_native_jar(&self, jar_path: &Path, natives_dir: &Path, lib: &Library) -> Result<()> {
        let file = std::fs::File::open(jar_path)?;
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

            // Only extract native library files and flatten to root
            let is_native = name.ends_with(".dylib")
                || name.ends_with(".dll")
                || name.ends_with(".so")
                || name.ends_with(".jnilib");

            if is_native {
                // Extract just the filename, ignoring directory structure
                let file_name = Path::new(&name)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(name.clone());

                let dest = natives_dir.join(&file_name);
                println!("  -> {}", file_name);

                let mut outfile = std::fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }

        Ok(())
    }
}
