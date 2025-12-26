//! Fabric mod loader support
//!
//! Install and manage Fabric mod loader.

use crate::core::instance::{Instance, ModLoader};
use crate::core::version::{Library, VersionDetails};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";

/// Fabric loader version info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLoaderVersion {
    pub separator: String,
    pub build: u32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

/// Fabric intermediary version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricIntermediaryVersion {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

/// Fabric launch profile (from profile/json endpoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FabricProfile {
    pub id: String,
    pub inherits_from: String,
    pub release_time: String,
    pub time: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub main_class: String,
    pub arguments: Option<FabricArguments>,
    pub libraries: Vec<FabricLibrary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricArguments {
    pub game: Vec<String>,
    pub jvm: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricLibrary {
    pub name: String,
    pub url: Option<String>,
}

/// Fabric manager
pub struct FabricManager;

impl FabricManager {
    /// Get all available Fabric loader versions
    pub async fn get_loader_versions() -> Result<Vec<FabricLoaderVersion>> {
        let url = format!("{}/versions/loader", FABRIC_META_URL);
        let response = reqwest::get(&url)
            .await
            .context("Failed to fetch Fabric loader versions")?;
        let versions: Vec<FabricLoaderVersion> = response.json().await?;
        Ok(versions)
    }

    /// Get latest stable Fabric loader version
    pub async fn get_latest_loader() -> Result<String> {
        let versions = Self::get_loader_versions().await?;
        versions
            .into_iter()
            .find(|v| v.stable)
            .map(|v| v.version)
            .context("No stable Fabric loader found")
    }

    /// Get Fabric versions compatible with a Minecraft version
    pub async fn get_compatible_loaders(mc_version: &str) -> Result<Vec<FabricLoaderVersion>> {
        let url = format!("{}/versions/loader/{}", FABRIC_META_URL, mc_version);
        let response = reqwest::get(&url)
            .await
            .context("Failed to fetch compatible Fabric versions")?;

        // This endpoint returns loader+intermediary pairs, we extract loader versions
        #[derive(Deserialize)]
        struct LoaderPair {
            loader: FabricLoaderVersion,
        }

        let pairs: Vec<LoaderPair> = response.json().await?;
        Ok(pairs.into_iter().map(|p| p.loader).collect())
    }

    /// Get Fabric profile (version.json content)
    pub async fn get_profile(mc_version: &str, loader_version: &str) -> Result<FabricProfile> {
        let url = format!(
            "{}/versions/loader/{}/{}/profile/json",
            FABRIC_META_URL, mc_version, loader_version
        );

        let response = reqwest::get(&url)
            .await
            .context("Failed to fetch Fabric profile")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Fabric profile not found for {} with loader {}",
                mc_version,
                loader_version
            );
        }

        let profile: FabricProfile = response.json().await?;
        Ok(profile)
    }

    /// Convert Fabric libraries to standard Library format
    pub fn convert_libraries(fabric_libs: &[FabricLibrary]) -> Vec<Library> {
        fabric_libs
            .iter()
            .map(|fl| {
                // Parse Maven coordinates
                let parts: Vec<&str> = fl.name.split(':').collect();
                let path = if parts.len() >= 3 {
                    let group = parts[0].replace('.', "/");
                    let artifact = parts[1];
                    let version = parts[2];
                    Some(format!(
                        "{}/{}/{}/{}-{}.jar",
                        group, artifact, version, artifact, version
                    ))
                } else {
                    None
                };

                // Build URL
                let url = fl.url.as_ref().map(|base| {
                    if let Some(ref p) = path {
                        format!("{}{}", base, p)
                    } else {
                        base.clone()
                    }
                });

                Library {
                    name: fl.name.clone(),
                    downloads: path.map(|p| crate::core::version::LibraryDownloads {
                        artifact: Some(crate::core::version::Artifact {
                            path: p.clone(),
                            sha1: String::new(), // Fabric doesn't provide SHA1
                            size: 0,
                            url: url.unwrap_or_else(|| format!("https://maven.fabricmc.net/{}", p)),
                        }),
                        classifiers: None,
                    }),
                    url: fl.url.clone(),
                    rules: None,
                    natives: None,
                    extract: None,
                }
            })
            .collect()
    }

    /// Install Fabric to an instance
    pub async fn install(instance: &mut Instance, loader_version: &str) -> Result<()> {
        let mc_version = &instance.info.version;

        tracing::info!(
            "Installing Fabric {} for Minecraft {}",
            loader_version,
            mc_version
        );

        // Get Fabric profile
        let profile = Self::get_profile(mc_version, loader_version).await?;

        // Update instance
        instance.info.loader = ModLoader::Fabric;
        instance.info.loader_version = Some(loader_version.to_string());

        tracing::info!("Fabric {} installed successfully", loader_version);

        Ok(())
    }

    /// Create merged version details for Fabric
    pub fn merge_version_details(
        vanilla: &VersionDetails,
        fabric_profile: &FabricProfile,
    ) -> VersionDetails {
        let mut merged = vanilla.clone();

        // Override main class
        merged.main_class = fabric_profile.main_class.clone();

        // Convert Fabric libraries
        let fabric_libs = Self::convert_libraries(&fabric_profile.libraries);

        // Get group:artifact keys from Fabric libraries to filter out vanilla duplicates
        let fabric_keys: std::collections::HashSet<String> = fabric_libs
            .iter()
            .filter_map(|lib| {
                let parts: Vec<&str> = lib.name.split(':').collect();
                if parts.len() >= 2 {
                    Some(format!("{}:{}", parts[0], parts[1]))
                } else {
                    None
                }
            })
            .collect();

        // Filter out vanilla libraries that would conflict with Fabric's versions
        merged.libraries.retain(|lib| {
            let parts: Vec<&str> = lib.name.split(':').collect();
            if parts.len() >= 2 {
                let key = format!("{}:{}", parts[0], parts[1]);
                if fabric_keys.contains(&key) {
                    tracing::debug!("Replacing vanilla library {} with Fabric version", lib.name);
                    return false;
                }
            }
            true
        });

        // Add Fabric libraries (they take precedence)
        merged.libraries.extend(fabric_libs);

        // Mark as inherited
        merged.inherits_from = Some(fabric_profile.inherits_from.clone());

        merged
    }
}
