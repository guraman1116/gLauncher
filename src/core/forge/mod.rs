//! Forge mod loader support
//!
//! Install and manage Forge mod loader.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

const FORGE_MAVEN_URL: &str = "https://maven.minecraftforge.net";
const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/maven/net/minecraftforge/forge/promotions_slim.json";

/// Forge version info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeVersion {
    pub mc_version: String,
    pub forge_version: String,
    pub full_version: String, // "1.20.1-47.2.0"
    pub is_recommended: bool,
    pub is_latest: bool,
}

impl ForgeVersion {
    /// Get the installer URL for this version
    pub fn installer_url(&self) -> String {
        format!(
            "{}/net/minecraftforge/forge/{}/forge-{}-installer.jar",
            FORGE_MAVEN_URL, self.full_version, self.full_version
        )
    }

    /// Get the universal JAR URL (for older versions)
    pub fn universal_url(&self) -> String {
        format!(
            "{}/net/minecraftforge/forge/{}/forge-{}-universal.jar",
            FORGE_MAVEN_URL, self.full_version, self.full_version
        )
    }
}

/// Promotions response from Forge
#[derive(Debug, Deserialize)]
struct ForgePromotions {
    homepage: String,
    promos: HashMap<String, String>,
}

/// Forge install profile from installer JAR
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeInstallProfile {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub minecraft: String,
    #[serde(default)]
    pub json: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub libraries: Vec<ForgeLibrary>,
    #[serde(default)]
    pub processors: Vec<ForgeProcessor>,
    #[serde(default)]
    pub data: HashMap<String, ForgeDataEntry>,
}

/// Library entry in Forge install profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeLibrary {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<ForgeLibraryDownloads>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeLibraryDownloads {
    pub artifact: Option<ForgeArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeArtifact {
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

/// Processor entry in install profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeProcessor {
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: Option<HashMap<String, String>>,
    #[serde(default)]
    pub sides: Option<Vec<String>>,
}

/// Data entry in install profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeDataEntry {
    pub client: String,
    #[serde(default)]
    pub server: Option<String>,
}

/// Forge version JSON (for launching)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeVersionJson {
    pub id: String,
    pub inherits_from: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub main_class: String,
    pub arguments: Option<ForgeArguments>,
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    pub libraries: Vec<ForgeLibrary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeArguments {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

/// Forge manager for installation and version management
pub struct ForgeManager {
    data_dir: PathBuf,
    libraries_dir: PathBuf,
    versions_dir: PathBuf,
    java_path: PathBuf,
}

impl ForgeManager {
    /// Create a new Forge manager
    pub fn new(data_dir: &Path, java_path: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            libraries_dir: data_dir.join("libraries"),
            versions_dir: data_dir.join("versions"),
            java_path: java_path.to_path_buf(),
        }
    }

    /// Get recommended/latest Forge versions for each Minecraft version
    pub async fn get_promotions(&self) -> Result<Vec<ForgeVersion>> {
        tracing::info!("Fetching Forge promotions...");

        let response = reqwest::get(FORGE_PROMOTIONS_URL)
            .await
            .context("Failed to fetch Forge promotions")?;

        let promos: ForgePromotions = response
            .json()
            .await
            .context("Failed to parse Forge promotions")?;

        let mut versions = Vec::new();
        let mut seen = HashMap::new();

        for (key, forge_version) in promos.promos {
            // Keys are like "1.20.1-recommended", "1.20.1-latest"
            let parts: Vec<&str> = key.rsplitn(2, '-').collect();
            if parts.len() != 2 {
                continue;
            }

            let version_type = parts[0]; // "recommended" or "latest"
            let mc_version = parts[1]; // "1.20.1"

            let full_version = format!("{}-{}", mc_version, forge_version);
            let is_recommended = version_type == "recommended";
            let is_latest = version_type == "latest";

            // Merge with existing entry if present
            if let Some(existing) = seen.get_mut(&full_version) {
                let v: &mut ForgeVersion = existing;
                if is_recommended {
                    v.is_recommended = true;
                }
                if is_latest {
                    v.is_latest = true;
                }
            } else {
                let version = ForgeVersion {
                    mc_version: mc_version.to_string(),
                    forge_version: forge_version.clone(),
                    full_version: full_version.clone(),
                    is_recommended,
                    is_latest,
                };
                seen.insert(full_version, version);
            }
        }

        versions.extend(seen.into_values());

        // Sort by MC version (descending), then by recommended status
        versions.sort_by(|a, b| {
            let mc_cmp = version_compare(&b.mc_version, &a.mc_version);
            if mc_cmp != std::cmp::Ordering::Equal {
                return mc_cmp;
            }
            b.is_recommended.cmp(&a.is_recommended)
        });

        tracing::info!("Found {} Forge versions", versions.len());
        Ok(versions)
    }

    /// Get Forge versions for a specific Minecraft version
    pub async fn get_versions_for_mc(&self, mc_version: &str) -> Result<Vec<ForgeVersion>> {
        let all = self.get_promotions().await?;
        Ok(all
            .into_iter()
            .filter(|v| v.mc_version == mc_version)
            .collect())
    }

    /// Get recommended Forge version for a Minecraft version
    pub async fn get_recommended(&self, mc_version: &str) -> Result<Option<ForgeVersion>> {
        let versions = self.get_versions_for_mc(mc_version).await?;
        Ok(versions.into_iter().find(|v| v.is_recommended))
    }

    /// Download the Forge installer JAR
    pub async fn download_installer(&self, version: &ForgeVersion) -> Result<PathBuf> {
        let installer_dir = self.data_dir.join("forge_installers");
        std::fs::create_dir_all(&installer_dir)?;

        let installer_path =
            installer_dir.join(format!("forge-{}-installer.jar", version.full_version));

        if installer_path.exists() {
            tracing::info!("Forge installer already downloaded: {:?}", installer_path);
            return Ok(installer_path);
        }

        let url = version.installer_url();
        tracing::info!("Downloading Forge installer: {}", url);

        let response = reqwest::get(&url)
            .await
            .context("Failed to download Forge installer")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to download Forge installer: HTTP {}",
                response.status()
            );
        }

        let bytes = response.bytes().await?;
        std::fs::write(&installer_path, &bytes)?;

        tracing::info!("Downloaded Forge installer: {:?}", installer_path);
        Ok(installer_path)
    }

    /// Extract and parse install_profile.json from installer JAR
    pub fn parse_install_profile(&self, installer_path: &Path) -> Result<ForgeInstallProfile> {
        let file = std::fs::File::open(installer_path)?;
        let mut archive = ZipArchive::new(file)?;

        // Try to find install_profile.json
        let profile_content = if let Ok(mut entry) = archive.by_name("install_profile.json") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            content
        } else {
            anyhow::bail!("install_profile.json not found in installer");
        };

        let profile: ForgeInstallProfile = serde_json::from_str(&profile_content)
            .context("Failed to parse install_profile.json")?;

        Ok(profile)
    }

    /// Extract version JSON from installer JAR
    pub fn extract_version_json(&self, installer_path: &Path) -> Result<ForgeVersionJson> {
        let file = std::fs::File::open(installer_path)?;
        let mut archive = ZipArchive::new(file)?;

        // First get the install profile to find the version json path
        let profile = self.parse_install_profile(installer_path)?;
        let version_path = if !profile.json.is_empty() {
            profile.json.trim_start_matches('/').to_string()
        } else {
            "version.json".to_string()
        };

        let version_content = if let Ok(mut entry) = archive.by_name(&version_path) {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            content
        } else {
            anyhow::bail!("Version JSON not found in installer: {}", version_path);
        };

        let version: ForgeVersionJson =
            serde_json::from_str(&version_content).context("Failed to parse version JSON")?;

        Ok(version)
    }

    /// Download all required libraries for Forge
    pub async fn download_libraries(&self, profile: &ForgeInstallProfile) -> Result<()> {
        tracing::info!("Downloading {} Forge libraries...", profile.libraries.len());

        for lib in &profile.libraries {
            self.download_library(lib).await?;
        }

        Ok(())
    }

    /// Download a single library
    async fn download_library(&self, lib: &ForgeLibrary) -> Result<()> {
        let (path, url) = if let Some(ref downloads) = lib.downloads {
            if let Some(ref artifact) = downloads.artifact {
                (artifact.path.clone(), artifact.url.clone())
            } else {
                return Ok(()); // No artifact to download
            }
        } else {
            // Parse Maven coordinates: group:artifact:version[:classifier]
            let (path, url) = maven_to_path_url(&lib.name, lib.url.as_deref())?;
            (path, url)
        };

        let dest = self.libraries_dir.join(&path);

        if dest.exists() {
            return Ok(());
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        tracing::debug!("Downloading library: {}", lib.name);

        let response = reqwest::get(&url).await?;
        if !response.status().is_success() {
            anyhow::bail!(
                "Failed to download library {}: HTTP {}",
                lib.name,
                response.status()
            );
        }

        let bytes = response.bytes().await?;
        std::fs::write(&dest, &bytes)?;

        Ok(())
    }

    /// Run all processors from install profile
    pub fn run_processors(
        &self,
        profile: &ForgeInstallProfile,
        mc_version: &str,
        installer_path: &Path,
    ) -> Result<()> {
        tracing::info!("Running {} Forge processors...", profile.processors.len());

        // Extract data files from installer first
        self.extract_installer_data(installer_path)?;

        let mc_jar = self
            .versions_dir
            .join(mc_version)
            .join(format!("{}.jar", mc_version));

        for (i, processor) in profile.processors.iter().enumerate() {
            // Check if this processor is for client only
            if let Some(ref sides) = processor.sides {
                if !sides.contains(&"client".to_string()) {
                    continue;
                }
            }

            tracing::info!(
                "Running processor {}/{}: {}",
                i + 1,
                profile.processors.len(),
                processor.jar
            );

            self.run_processor(processor, profile, &mc_jar, installer_path)?;
        }

        Ok(())
    }

    /// Extract data files from installer JAR
    fn extract_installer_data(&self, installer_path: &Path) -> Result<()> {
        let file = std::fs::File::open(installer_path)?;
        let mut archive = ZipArchive::new(file)?;

        let data_dir = self.data_dir.join("forge_data");
        std::fs::create_dir_all(&data_dir)?;

        // Extract files from data/ directory
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            if name.starts_with("data/") && !entry.is_dir() {
                let dest = data_dir.join(name.strip_prefix("data/").unwrap_or(&name));
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut content = Vec::new();
                entry.read_to_end(&mut content)?;
                std::fs::write(&dest, content)?;
            }
        }

        Ok(())
    }

    /// Run a single processor
    fn run_processor(
        &self,
        processor: &ForgeProcessor,
        profile: &ForgeInstallProfile,
        mc_jar: &Path,
        installer_path: &Path,
    ) -> Result<()> {
        // Build classpath
        let mut classpath = Vec::new();

        // Add processor JAR
        let (processor_path, _) = maven_to_path_url(&processor.jar, None)?;
        classpath.push(self.libraries_dir.join(&processor_path));

        // Add classpath entries
        for cp in &processor.classpath {
            let (cp_path, _) = maven_to_path_url(cp, None)?;
            classpath.push(self.libraries_dir.join(&cp_path));
        }

        let classpath_str = classpath
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(if cfg!(windows) { ";" } else { ":" });

        // Get main class from processor JAR manifest
        let main_class = self.get_jar_main_class(&self.libraries_dir.join(&processor_path))?;

        // Build args with placeholder replacement
        let args: Vec<String> = processor
            .args
            .iter()
            .map(|arg| self.replace_processor_placeholder(arg, profile, mc_jar, installer_path))
            .collect();

        // Run processor
        let output = Command::new(&self.java_path)
            .arg("-cp")
            .arg(&classpath_str)
            .arg(&main_class)
            .args(&args)
            .output()
            .context("Failed to run processor")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Processor {} failed: {}", processor.jar, stderr);
        }

        Ok(())
    }

    /// Get main class from JAR manifest
    fn get_jar_main_class(&self, jar_path: &Path) -> Result<String> {
        let file = std::fs::File::open(jar_path)?;
        let mut archive = ZipArchive::new(file)?;

        if let Ok(mut entry) = archive.by_name("META-INF/MANIFEST.MF") {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;

            for line in content.lines() {
                if let Some(class) = line.strip_prefix("Main-Class:") {
                    return Ok(class.trim().to_string());
                }
            }
        }

        anyhow::bail!("Main-Class not found in JAR manifest: {:?}", jar_path)
    }

    /// Replace placeholders in processor arguments
    fn replace_processor_placeholder(
        &self,
        arg: &str,
        profile: &ForgeInstallProfile,
        mc_jar: &Path,
        installer_path: &Path,
    ) -> String {
        let mut result = arg.to_string();

        // Handle {KEY} placeholders from data section
        if arg.starts_with('{') && arg.ends_with('}') {
            let key = &arg[1..arg.len() - 1];

            if key == "MINECRAFT_JAR" {
                return mc_jar.to_string_lossy().to_string();
            }

            if key == "SIDE" {
                return "client".to_string();
            }

            if key == "INSTALLER" {
                return installer_path.to_string_lossy().to_string();
            }

            if let Some(data_entry) = profile.data.get(key) {
                let value = &data_entry.client;

                // Check if it's a file reference [path]
                if value.starts_with('[') && value.ends_with(']') {
                    let maven = &value[1..value.len() - 1];
                    if let Ok((path, _)) = maven_to_path_url(maven, None) {
                        return self.libraries_dir.join(&path).to_string_lossy().to_string();
                    }
                }

                // Check if it's an installer path /data/...
                if value.starts_with('/') {
                    let data_path = value.trim_start_matches('/');
                    return self
                        .data_dir
                        .join("forge_data")
                        .join(data_path.trim_start_matches("data/"))
                        .to_string_lossy()
                        .to_string();
                }

                return value.clone();
            }
        }

        // Handle [MAVEN] placeholders
        if arg.starts_with('[') && arg.ends_with(']') {
            let maven = &arg[1..arg.len() - 1];
            if let Ok((path, _)) = maven_to_path_url(maven, None) {
                return self.libraries_dir.join(&path).to_string_lossy().to_string();
            }
        }

        result
    }

    /// Full Forge installation process
    pub async fn install(
        &self,
        version: &ForgeVersion,
        mc_version: &str,
    ) -> Result<ForgeVersionJson> {
        tracing::info!(
            "Installing Forge {} for MC {}",
            version.forge_version,
            mc_version
        );

        // Step 1: Download installer
        let installer_path = self.download_installer(version).await?;

        // Step 2: Parse install profile
        let profile = self.parse_install_profile(&installer_path)?;

        // Step 3: Extract version JSON
        let version_json = self.extract_version_json(&installer_path)?;

        // Step 4: Download libraries (from install profile)
        self.download_libraries(&profile).await?;

        // Step 5: Download libraries (from version JSON)
        for lib in &version_json.libraries {
            self.download_library(lib).await?;
        }

        // Step 6: Run processors
        self.run_processors(&profile, mc_version, &installer_path)?;

        // Step 7: Save version JSON
        let version_id = &version_json.id;
        let version_dir = self.versions_dir.join(version_id);
        std::fs::create_dir_all(&version_dir)?;

        let version_file = version_dir.join(format!("{}.json", version_id));
        let content = serde_json::to_string_pretty(&version_json)?;
        std::fs::write(&version_file, content)?;

        tracing::info!("Forge {} installed successfully!", version.forge_version);
        Ok(version_json)
    }
}

/// Convert Maven coordinates to path and URL
/// Format: group:artifact:version[:classifier][@extension]
fn maven_to_path_url(coords: &str, custom_url: Option<&str>) -> Result<(String, String)> {
    let base_url = custom_url.unwrap_or(FORGE_MAVEN_URL);

    // Split extension if present
    let (coords, extension) = if let Some(at_pos) = coords.rfind('@') {
        (&coords[..at_pos], &coords[at_pos + 1..])
    } else {
        (coords, "jar")
    };

    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        anyhow::bail!("Invalid Maven coordinates: {}", coords);
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3).copied();

    let filename = if let Some(classifier) = classifier {
        format!("{}-{}-{}.{}", artifact, version, classifier, extension)
    } else {
        format!("{}-{}.{}", artifact, version, extension)
    };

    let path = format!("{}/{}/{}/{}", group, artifact, version, filename);
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path);

    Ok((path, url))
}

/// Compare version strings (simple numeric comparison)
fn version_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
    let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();

    for (av, bv) in a_parts.iter().zip(b_parts.iter()) {
        match av.cmp(bv) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    a_parts.len().cmp(&b_parts.len())
}
