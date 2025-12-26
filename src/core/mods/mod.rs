//! Mod management module
//!
//! Scans and manages mods in the instance mods folder.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// Information about a mod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    /// Filename of the mod JAR
    pub filename: String,
    /// Full path to the mod
    pub path: PathBuf,
    /// Mod name (from fabric.mod.json or filename)
    pub name: String,
    /// Mod version
    pub version: String,
    /// Mod description
    pub description: String,
    /// Whether the mod is enabled (.jar) or disabled (.jar.disabled)
    pub enabled: bool,
    /// Last modified time
    pub modified_at: DateTime<Utc>,
    /// File size in bytes
    pub size: u64,
}

/// Fabric mod metadata from fabric.mod.json
#[derive(Debug, Deserialize)]
struct FabricModJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
}

/// Mod manager for scanning and managing mods
pub struct ModManager {
    mods_dir: PathBuf,
}

impl ModManager {
    /// Create a new mod manager for the given mods directory
    pub fn new(mods_dir: &Path) -> Self {
        Self {
            mods_dir: mods_dir.to_path_buf(),
        }
    }

    /// Get the mods directory
    pub fn mods_dir(&self) -> &Path {
        &self.mods_dir
    }

    /// Ensure the mods directory exists
    pub fn ensure_dir(&self) -> Result<()> {
        if !self.mods_dir.exists() {
            fs::create_dir_all(&self.mods_dir)?;
        }
        Ok(())
    }

    /// List all mods in the directory
    pub fn list_mods(&self) -> Result<Vec<ModInfo>> {
        let mut mods = Vec::new();

        if !self.mods_dir.exists() {
            return Ok(mods);
        }

        for entry in fs::read_dir(&self.mods_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check if it's a JAR file (enabled or disabled)
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();

            let is_jar = filename.ends_with(".jar");
            let is_disabled_jar = filename.ends_with(".jar.disabled");

            if !is_jar && !is_disabled_jar {
                continue;
            }

            let metadata = fs::metadata(&path)?;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
                .flatten()
                .unwrap_or_else(Utc::now);

            // Try to read mod metadata from the JAR
            let (name, version, description) = self.read_mod_metadata(&path).unwrap_or_else(|| {
                // Fallback to filename
                let clean_name = filename
                    .trim_end_matches(".disabled")
                    .trim_end_matches(".jar")
                    .to_string();
                (clean_name, "Unknown".to_string(), String::new())
            });

            mods.push(ModInfo {
                filename: filename.clone(),
                path: path.clone(),
                name,
                version,
                description,
                enabled: is_jar,
                modified_at: modified,
                size: metadata.len(),
            });
        }

        // Sort by name
        mods.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(mods)
    }

    /// Read mod metadata from a JAR file
    fn read_mod_metadata(&self, path: &Path) -> Option<(String, String, String)> {
        let file = fs::File::open(path).ok()?;
        let mut archive = ZipArchive::new(file).ok()?;

        // Try fabric.mod.json first
        if let Ok(mut entry) = archive.by_name("fabric.mod.json") {
            let mut contents = String::new();
            entry.read_to_string(&mut contents).ok()?;

            if let Ok(meta) = serde_json::from_str::<FabricModJson>(&contents) {
                if !meta.name.is_empty() {
                    return Some((meta.name, meta.version, meta.description));
                }
            }
        }

        None
    }

    /// Toggle a mod's enabled state
    pub fn toggle_mod(&self, mod_info: &ModInfo) -> Result<PathBuf> {
        let new_path = if mod_info.enabled {
            // Disable: rename .jar to .jar.disabled
            self.mods_dir
                .join(format!("{}.disabled", mod_info.filename))
        } else {
            // Enable: remove .disabled suffix
            let new_name = mod_info.filename.trim_end_matches(".disabled");
            self.mods_dir.join(new_name)
        };

        fs::rename(&mod_info.path, &new_path).context("Failed to toggle mod")?;

        Ok(new_path)
    }

    /// Delete a mod
    pub fn delete_mod(&self, mod_info: &ModInfo) -> Result<()> {
        fs::remove_file(&mod_info.path).context("Failed to delete mod")?;
        Ok(())
    }

    /// Open mods folder in file explorer
    #[cfg(target_os = "macos")]
    pub fn open_folder(&self) -> Result<()> {
        self.ensure_dir()?;
        std::process::Command::new("open")
            .arg(&self.mods_dir)
            .spawn()
            .context("Failed to open folder")?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub fn open_folder(&self) -> Result<()> {
        self.ensure_dir()?;
        std::process::Command::new("explorer")
            .arg(&self.mods_dir)
            .spawn()
            .context("Failed to open folder")?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn open_folder(&self) -> Result<()> {
        self.ensure_dir()?;
        std::process::Command::new("xdg-open")
            .arg(&self.mods_dir)
            .spawn()
            .context("Failed to open folder")?;
        Ok(())
    }
}

/// Format file size for display
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
