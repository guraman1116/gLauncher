//! Version details
//!
//! Detailed version information including libraries, assets, and arguments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Detailed version information from version JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetails {
    pub id: String,

    #[serde(rename = "type")]
    pub version_type: String,

    pub main_class: String,

    /// Legacy argument format (pre-1.13)
    pub minecraft_arguments: Option<String>,

    /// Modern argument format (1.13+)
    pub arguments: Option<Arguments>,

    pub libraries: Vec<Library>,

    pub asset_index: AssetIndexInfo,

    pub downloads: Downloads,

    pub java_version: Option<JavaVersion>,

    /// Inherited version (for modded versions)
    pub inherits_from: Option<String>,
}

/// Modern argument structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<ArgumentValue>,
    pub jvm: Vec<ArgumentValue>,
}

/// Argument can be a simple string or a conditional object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    Simple(String),
    Conditional(ConditionalArgument),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalArgument {
    pub rules: Vec<Rule>,
    pub value: StringOrVec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

/// Library dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,

    pub downloads: Option<LibraryDownloads>,

    /// URL for libraries without downloads section
    pub url: Option<String>,

    pub rules: Option<Vec<Rule>>,

    pub natives: Option<HashMap<String, String>>,

    pub extract: Option<ExtractRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractRule {
    pub exclude: Option<Vec<String>>,
}

/// Rule for conditional inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

/// Asset index information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexInfo {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: Option<u64>,
    pub url: String,
}

/// Download information for client/server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Downloads {
    pub client: Option<DownloadInfo>,
    pub server: Option<DownloadInfo>,
    pub client_mappings: Option<DownloadInfo>,
    pub server_mappings: Option<DownloadInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Java version requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

// === Rule evaluation ===

impl Rule {
    /// Check if rule allows inclusion on current OS
    pub fn is_allowed(&self) -> bool {
        let os_matches = self.os.as_ref().map_or(true, |os| os.matches_current());

        match self.action.as_str() {
            "allow" => os_matches,
            "disallow" => !os_matches,
            _ => true,
        }
    }
}

impl OsRule {
    /// Check if current OS matches the rule
    pub fn matches_current(&self) -> bool {
        if let Some(ref name) = self.name {
            let current_os = if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "macos") {
                "osx"
            } else {
                "linux"
            };

            if name != current_os {
                return false;
            }
        }

        if let Some(ref arch) = self.arch {
            let current_arch = if cfg!(target_arch = "x86_64") {
                "x64"
            } else if cfg!(target_arch = "x86") {
                "x86"
            } else if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "unknown"
            };

            if arch != current_arch {
                return false;
            }
        }

        true
    }
}

impl Library {
    /// Check if library should be included based on rules
    pub fn should_include(&self) -> bool {
        match &self.rules {
            None => true,
            Some(rules) => {
                // All rules must allow
                rules.iter().all(|r| r.is_allowed())
            }
        }
    }

    /// Get the native classifier for current OS
    pub fn get_native_classifier(&self) -> Option<String> {
        self.natives.as_ref().and_then(|natives| {
            let os_key = if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "macos") {
                "osx"
            } else {
                "linux"
            };

            natives.get(os_key).map(|s| {
                // Replace ${arch} placeholder
                let arch = if cfg!(target_arch = "x86_64") {
                    "64"
                } else {
                    "32"
                };
                s.replace("${arch}", arch)
            })
        })
    }

    /// Get artifact download info
    pub fn get_artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref().and_then(|d| d.artifact.as_ref())
    }

    /// Get native artifact download info
    pub fn get_native_artifact(&self) -> Option<&Artifact> {
        let classifier = self.get_native_classifier()?;
        self.downloads
            .as_ref()
            .and_then(|d| d.classifiers.as_ref())
            .and_then(|c| c.get(&classifier))
    }

    /// Parse Maven coordinates (group:artifact:version)
    pub fn parse_name(&self) -> Option<(String, String, String)> {
        let parts: Vec<&str> = self.name.split(':').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }

    /// Get path for library JAR
    pub fn get_path(&self) -> Option<String> {
        // Try to get from artifact first
        if let Some(artifact) = self.get_artifact() {
            return Some(artifact.path.clone());
        }

        // Otherwise construct from Maven coordinates
        self.parse_name().map(|(group, artifact, version)| {
            let group_path = group.replace('.', "/");
            format!(
                "{}/{}/{}/{}-{}.jar",
                group_path, artifact, version, artifact, version
            )
        })
    }
}

/// Asset index containing all game assets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    /// Get the path for this asset in the objects directory
    pub fn get_path(&self) -> String {
        format!("{}/{}", &self.hash[..2], &self.hash)
    }

    /// Get the download URL for this asset
    pub fn get_url(&self) -> String {
        format!(
            "https://resources.download.minecraft.net/{}/{}",
            &self.hash[..2],
            &self.hash
        )
    }
}

/// Fetch asset index
pub async fn fetch_asset_index(info: &AssetIndexInfo) -> anyhow::Result<AssetIndex> {
    let response = reqwest::get(&info.url).await?;
    let index: AssetIndex = response.json().await?;
    Ok(index)
}
