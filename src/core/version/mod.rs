//! Version management module
//!
//! Download and manage Minecraft versions.

mod details;

pub use details::*;

use serde::{Deserialize, Serialize};

/// Version manifest from Mojang
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Fetch the version manifest from Mojang
pub async fn fetch_manifest() -> anyhow::Result<VersionManifest> {
    let response = reqwest::get(VERSION_MANIFEST_URL).await?;
    let manifest: VersionManifest = response.json().await?;
    Ok(manifest)
}

/// Get version info by ID
pub fn get_version_info<'a>(
    manifest: &'a VersionManifest,
    version_id: &str,
) -> Option<&'a VersionInfo> {
    manifest.versions.iter().find(|v| v.id == version_id)
}

/// Fetch detailed version info
pub async fn fetch_version_details(version_info: &VersionInfo) -> anyhow::Result<VersionDetails> {
    let response = reqwest::get(&version_info.url).await?;
    let details: VersionDetails = response.json().await?;
    Ok(details)
}

/// Filter versions by type
pub fn filter_versions(manifest: &VersionManifest, include_snapshots: bool) -> Vec<&VersionInfo> {
    manifest
        .versions
        .iter()
        .filter(|v| {
            v.version_type == VersionType::Release
                || (include_snapshots && v.version_type == VersionType::Snapshot)
        })
        .collect()
}
