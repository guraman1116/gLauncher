//! Auto-update module
//!
//! Check for and apply updates from GitHub Releases.

use anyhow::{Context, Result};
use serde::Deserialize;

/// Current version from Cargo.toml
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub repository owner and name
const REPO_OWNER: &str = "guraman1116";
const REPO_NAME: &str = "gLauncher";

/// GitHub Release info
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub html_url: String,
    pub body: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<ReleaseAsset>,
}

/// Release asset (binary download)
#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub content_type: String,
}

/// Update check result
#[derive(Debug)]
pub enum UpdateStatus {
    /// Current version is up to date
    UpToDate,
    /// New version available
    UpdateAvailable {
        current: String,
        latest: String,
        release_url: String,
        download_url: Option<String>,
        release_notes: Option<String>,
    },
    /// Failed to check for updates
    CheckFailed(String),
}

/// Update manager
pub struct UpdateManager;

impl UpdateManager {
    /// Check for updates from GitHub Releases
    pub async fn check_for_updates() -> UpdateStatus {
        match Self::fetch_latest_release().await {
            Ok(release) => {
                let latest = release.tag_name.trim_start_matches('v').to_string();
                let current = CURRENT_VERSION.to_string();

                if Self::is_newer(&latest, &current) {
                    let download_url = Self::get_platform_asset(&release.assets);

                    UpdateStatus::UpdateAvailable {
                        current,
                        latest,
                        release_url: release.html_url,
                        download_url,
                        release_notes: release.body,
                    }
                } else {
                    UpdateStatus::UpToDate
                }
            }
            Err(e) => UpdateStatus::CheckFailed(e.to_string()),
        }
    }

    /// Fetch the latest release from GitHub
    async fn fetch_latest_release() -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            REPO_OWNER, REPO_NAME
        );

        let client = reqwest::Client::builder().user_agent("gLauncher").build()?;

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch release info")?;

        if !response.status().is_success() {
            if response.status().as_u16() == 404 {
                anyhow::bail!("No releases found");
            }
            anyhow::bail!("GitHub API error: {}", response.status());
        }

        let release: GitHubRelease = response
            .json()
            .await
            .context("Failed to parse release info")?;

        Ok(release)
    }

    /// Get the download URL for current platform
    fn get_platform_asset(assets: &[ReleaseAsset]) -> Option<String> {
        let platform_patterns = if cfg!(target_os = "macos") {
            vec!["macos", "darwin", "osx", "apple"]
        } else if cfg!(target_os = "windows") {
            vec!["windows", "win64", "win32", ".exe"]
        } else if cfg!(target_os = "linux") {
            vec!["linux", "ubuntu", "debian"]
        } else {
            vec![]
        };

        let arch_patterns = if cfg!(target_arch = "aarch64") {
            vec!["aarch64", "arm64", "apple-silicon"]
        } else if cfg!(target_arch = "x86_64") {
            vec!["x86_64", "x64", "amd64"]
        } else {
            vec![]
        };

        // Try to find a matching asset
        for asset in assets {
            let name_lower = asset.name.to_lowercase();

            // Skip .sha256 and other non-binary files
            if name_lower.ends_with(".sha256")
                || name_lower.ends_with(".sig")
                || name_lower.ends_with(".asc")
            {
                continue;
            }

            // Check for platform match
            let platform_match = platform_patterns.iter().any(|p| name_lower.contains(p));
            let arch_match =
                arch_patterns.is_empty() || arch_patterns.iter().any(|p| name_lower.contains(p));

            if platform_match && arch_match {
                return Some(asset.browser_download_url.clone());
            }
        }

        // Fallback: return first non-checksum asset if no platform match
        assets
            .iter()
            .find(|a| {
                let name = a.name.to_lowercase();
                !name.ends_with(".sha256") && !name.ends_with(".sig") && !name.ends_with(".asc")
            })
            .map(|a| a.browser_download_url.clone())
    }

    /// Compare version strings (semantic versioning)
    fn is_newer(latest: &str, current: &str) -> bool {
        let parse = |v: &str| -> Vec<u32> {
            v.split('.')
                .filter_map(|p| p.split('-').next()) // Handle pre-release tags
                .filter_map(|p| p.parse().ok())
                .collect()
        };

        let latest_parts = parse(latest);
        let current_parts = parse(current);

        for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
            match l.cmp(c) {
                std::cmp::Ordering::Greater => return true,
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Equal => continue,
            }
        }

        latest_parts.len() > current_parts.len()
    }

    /// Perform self-update using self_update crate
    pub fn update() -> Result<()> {
        use self_update::backends::github::Update;

        println!("🔄 Checking for updates...");

        let status = Update::configure()
            .repo_owner(REPO_OWNER)
            .repo_name(REPO_NAME)
            .bin_name("glauncher")
            .show_download_progress(true)
            .current_version(CURRENT_VERSION)
            .build()?
            .update()?;

        if status.updated() {
            println!("✅ Updated to version {}!", status.version());
        } else {
            println!("✨ Already up to date (v{}).", CURRENT_VERSION);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(UpdateManager::is_newer("1.0.1", "1.0.0"));
        assert!(UpdateManager::is_newer("1.1.0", "1.0.0"));
        assert!(UpdateManager::is_newer("2.0.0", "1.9.9"));
        assert!(!UpdateManager::is_newer("1.0.0", "1.0.0"));
        assert!(!UpdateManager::is_newer("1.0.0", "1.0.1"));
        assert!(UpdateManager::is_newer("1.0.0", "0.1.0"));
    }
}
