//! Instance management module
//!
//! Create, configure, and manage Minecraft instances.

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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    #[default]
    Vanilla,
    Fabric,
    Forge,
    Quilt,
    NeoForge,
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

/// Get the instances directory
pub fn instances_dir() -> PathBuf {
    crate::config::config_dir().join("instances")
}

/// List all instances
pub fn list() -> anyhow::Result<Vec<Instance>> {
    let dir = instances_dir();
    let mut instances = Vec::new();

    if !dir.exists() {
        return Ok(instances);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let config_path = path.join("instance.toml");
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                let instance: Instance = toml::from_str(&content)?;
                instances.push(instance);
            }
        }
    }

    Ok(instances)
}
