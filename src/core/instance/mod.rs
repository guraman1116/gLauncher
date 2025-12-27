//! Instance management module
//!
//! Create, configure, and manage Minecraft instances.

use crate::config;
use crate::core::version::VersionDetails;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Instance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub info: InstanceInfo,
    pub java: InstanceJavaConfig,
    pub game: GameConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub name: String,
    pub version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    #[default]
    Vanilla,
    Fabric,
    Forge,
    Quilt,
    NeoForge,
}

impl std::fmt::Display for ModLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModLoader::Vanilla => write!(f, "Vanilla"),
            ModLoader::Fabric => write!(f, "Fabric"),
            ModLoader::Forge => write!(f, "Forge"),
            ModLoader::Quilt => write!(f, "Quilt"),
            ModLoader::NeoForge => write!(f, "NeoForge"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceJavaConfig {
    #[serde(default)]
    pub override_global: bool,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub min_memory: String,
    #[serde(default)]
    pub max_memory: String,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl Default for InstanceJavaConfig {
    fn default() -> Self {
        Self {
            override_global: false,
            path: String::new(),
            min_memory: String::new(),
            max_memory: String::new(),
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(default = "default_width")]
    pub resolution_width: u32,
    #[serde(default = "default_height")]
    pub resolution_height: u32,
    #[serde(default)]
    pub fullscreen: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            resolution_width: default_width(),
            resolution_height: default_height(),
            fullscreen: false,
        }
    }
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}

/// Instance manager
pub struct InstanceManager {
    instances_dir: PathBuf,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            instances_dir: config::config_dir().join("instances"),
        }
    }

    /// Get instances directory
    pub fn instances_dir(&self) -> &Path {
        &self.instances_dir
    }

    /// Get instance directory
    pub fn get_instance_dir(&self, name: &str) -> PathBuf {
        self.instances_dir.join(name)
    }

    /// Get instance game directory (.minecraft)
    pub fn get_game_dir(&self, name: &str) -> PathBuf {
        self.get_instance_dir(name).join(".minecraft")
    }

    /// Get instance natives directory
    pub fn get_natives_dir(&self, name: &str) -> PathBuf {
        self.get_instance_dir(name).join("natives")
    }

    /// Check if instance exists
    pub fn exists(&self, name: &str) -> bool {
        self.get_instance_dir(name).join("instance.toml").exists()
    }

    /// Create a new instance
    pub fn create(
        &self,
        name: &str,
        version: &str,
        loader: ModLoader,
        loader_version: Option<String>,
    ) -> Result<Instance> {
        let instance_dir = self.get_instance_dir(name);

        if instance_dir.exists() {
            anyhow::bail!("Instance '{}' already exists", name);
        }

        // Create directories
        std::fs::create_dir_all(&instance_dir)?;
        std::fs::create_dir_all(self.get_game_dir(name))?;

        let instance = Instance {
            info: InstanceInfo {
                name: name.to_string(),
                version: version.to_string(),
                loader,
                loader_version,
                created_at: chrono::Utc::now(),
            },
            java: InstanceJavaConfig::default(),
            game: GameConfig::default(),
        };

        self.save(&instance)?;

        tracing::info!("Created instance: {}", name);
        Ok(instance)
    }

    /// Save instance configuration
    pub fn save(&self, instance: &Instance) -> Result<()> {
        let config_path = self
            .get_instance_dir(&instance.info.name)
            .join("instance.toml");
        let content = toml::to_string_pretty(instance)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// Load instance configuration
    pub fn load(&self, name: &str) -> Result<Instance> {
        let config_path = self.get_instance_dir(name).join("instance.toml");
        let content = std::fs::read_to_string(&config_path)
            .context(format!("Instance '{}' not found", name))?;
        let instance: Instance = toml::from_str(&content)?;
        Ok(instance)
    }

    /// Delete an instance
    pub fn delete(&self, name: &str) -> Result<()> {
        let instance_dir = self.get_instance_dir(name);
        if !instance_dir.exists() {
            anyhow::bail!("Instance '{}' not found", name);
        }
        std::fs::remove_dir_all(&instance_dir)?;
        tracing::info!("Deleted instance: {}", name);
        Ok(())
    }

    /// Rename an instance
    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        if old_name == new_name {
            return Ok(());
        }

        let old_dir = self.get_instance_dir(old_name);
        let new_dir = self.get_instance_dir(new_name);

        if !old_dir.exists() {
            anyhow::bail!("Instance '{}' not found", old_name);
        }

        if new_dir.exists() {
            anyhow::bail!("Instance '{}' already exists", new_name);
        }

        std::fs::rename(&old_dir, &new_dir)?;
        tracing::info!("Renamed instance: {} -> {}", old_name, new_name);
        Ok(())
    }

    /// List all instances
    pub fn list(&self) -> Result<Vec<Instance>> {
        let mut instances = Vec::new();

        if !self.instances_dir.exists() {
            return Ok(instances);
        }

        for entry in std::fs::read_dir(&self.instances_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    match self.load(name) {
                        Ok(instance) => instances.push(instance),
                        Err(e) => tracing::warn!("Failed to load instance {}: {}", name, e),
                    }
                }
            }
        }

        // Sort by creation date (newest first)
        instances.sort_by(|a, b| b.info.created_at.cmp(&a.info.created_at));

        Ok(instances)
    }
}

impl Default for InstanceManager {
    fn default() -> Self {
        Self::new()
    }
}

use std::path::Path;
