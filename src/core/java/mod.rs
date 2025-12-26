// Java Runtime Manager
// Handles downloading and managing Java runtimes from Adoptium

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Adoptium API base URL
const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

/// Java version requirements for Minecraft
#[derive(Debug, Clone, Copy)]
pub struct JavaRequirement {
    pub major_version: u32,
    pub mc_version_pattern: &'static str,
}

/// Known Java requirements for Minecraft versions
pub const JAVA_REQUIREMENTS: &[JavaRequirement] = &[
    // Minecraft 1.21+ requires Java 21
    JavaRequirement {
        major_version: 21,
        mc_version_pattern: "1.21",
    },
    // Minecraft 1.18-1.20 requires Java 17
    JavaRequirement {
        major_version: 17,
        mc_version_pattern: "1.20",
    },
    JavaRequirement {
        major_version: 17,
        mc_version_pattern: "1.19",
    },
    JavaRequirement {
        major_version: 17,
        mc_version_pattern: "1.18",
    },
    // Minecraft 1.17 requires Java 16+
    JavaRequirement {
        major_version: 17,
        mc_version_pattern: "1.17",
    },
    // Older versions work with Java 8
    JavaRequirement {
        major_version: 8,
        mc_version_pattern: "1.16",
    },
];

/// Manages Java runtime downloads and installations
pub struct JavaManager {
    java_dir: PathBuf,
}

impl JavaManager {
    /// Create a new JavaManager
    pub fn new(data_dir: &Path) -> Self {
        Self {
            java_dir: data_dir.join("java"),
        }
    }

    /// Get the required Java major version for a Minecraft version
    pub fn get_required_version(mc_version: &str) -> u32 {
        for req in JAVA_REQUIREMENTS {
            if mc_version.starts_with(req.mc_version_pattern) {
                return req.major_version;
            }
        }
        // Default to Java 8 for very old versions
        8
    }

    /// Get the Java home directory for a specific version
    fn get_java_home(&self, major_version: u32) -> PathBuf {
        self.java_dir.join(major_version.to_string())
    }

    /// Get the java executable path for a specific version
    fn get_java_executable(&self, major_version: u32) -> PathBuf {
        let java_home = self.get_java_home(major_version);
        #[cfg(target_os = "windows")]
        {
            java_home.join("bin").join("java.exe")
        }
        #[cfg(target_os = "macos")]
        {
            // macOS JRE has Contents/Home structure
            java_home
                .join("Contents")
                .join("Home")
                .join("bin")
                .join("java")
        }
        #[cfg(target_os = "linux")]
        {
            java_home.join("bin").join("java")
        }
    }

    /// Check if a Java version is installed locally
    pub fn is_installed(&self, major_version: u32) -> bool {
        let java_exe = self.get_java_executable(major_version);
        java_exe.exists()
    }

    /// Find system-installed Java of a specific version
    fn find_system_java(&self, major_version: u32) -> Option<PathBuf> {
        // Check common locations
        let paths_to_check = if cfg!(target_os = "macos") {
            vec![
                format!("/opt/homebrew/opt/openjdk@{}/bin/java", major_version),
                format!("/usr/local/opt/openjdk@{}/bin/java", major_version),
                format!(
                    "/Library/Java/JavaVirtualMachines/temurin-{}.jdk/Contents/Home/bin/java",
                    major_version
                ),
                format!(
                    "/Library/Java/JavaVirtualMachines/adoptopenjdk-{}.jdk/Contents/Home/bin/java",
                    major_version
                ),
            ]
        } else if cfg!(target_os = "linux") {
            vec![
                format!("/usr/lib/jvm/java-{}-openjdk/bin/java", major_version),
                format!("/usr/lib/jvm/temurin-{}-jdk/bin/java", major_version),
            ]
        } else {
            vec![]
        };

        for path in paths_to_check {
            let p = PathBuf::from(&path);
            if p.exists() {
                // Verify it's the right version
                if self.check_java_version(&p, major_version) {
                    return Some(p);
                }
            }
        }

        None
    }

    /// Check if a java executable is the required version
    fn check_java_version(&self, java_path: &Path, required_major: u32) -> bool {
        let output = Command::new(java_path).arg("-version").output();

        if let Ok(output) = output {
            let version_str = String::from_utf8_lossy(&output.stderr);
            // Parse version like "openjdk version \"17.0.1\" 2021-10-19"
            // or "openjdk version \"1.8.0_312\""
            for line in version_str.lines() {
                if line.contains("version") {
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line[start + 1..].find('"') {
                            let version = &line[start + 1..start + 1 + end];
                            // Handle 1.8.x format
                            if version.starts_with("1.8") && required_major == 8 {
                                return true;
                            }
                            // Handle modern format like 17.0.x
                            if let Some(major) = version.split('.').next() {
                                if let Ok(v) = major.parse::<u32>() {
                                    return v == required_major;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Get the download URL for Adoptium JRE
    fn get_download_url(&self, major_version: u32) -> String {
        let os = if cfg!(target_os = "macos") {
            "mac"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "windows"
        };

        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x64"
        };

        format!(
            "{}/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/adoptium",
            ADOPTIUM_API, major_version, os, arch
        )
    }

    /// Download and install Java from Adoptium
    pub async fn download(
        &self,
        major_version: u32,
        progress_callback: impl Fn(&str),
    ) -> Result<PathBuf> {
        let url = self.get_download_url(major_version);
        let java_home = self.get_java_home(major_version);

        progress_callback(&format!("Downloading Java {}...", major_version));

        // Create temp directory for download
        let temp_dir = self.java_dir.join("temp");
        std::fs::create_dir_all(&temp_dir)?;

        let archive_ext = if cfg!(target_os = "windows") {
            "zip"
        } else {
            "tar.gz"
        };
        let archive_path = temp_dir.join(format!("jre-{}.{}", major_version, archive_ext));

        // Download the archive
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to download Java")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download Java: HTTP {}", response.status());
        }

        let bytes = response.bytes().await?;
        std::fs::write(&archive_path, &bytes)?;

        progress_callback(&format!("Extracting Java {}...", major_version));

        // Extract the archive
        std::fs::create_dir_all(&java_home)?;

        #[cfg(not(target_os = "windows"))]
        {
            // Use tar command for extraction on Unix
            let output = Command::new("tar")
                .args([
                    "-xzf",
                    archive_path.to_str().unwrap(),
                    "-C",
                    java_home.to_str().unwrap(),
                    "--strip-components=1",
                ])
                .output()
                .context("Failed to extract Java archive")?;

            if !output.status.success() {
                anyhow::bail!(
                    "Failed to extract Java: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Use zip extraction on Windows
            let file = std::fs::File::open(&archive_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                let name = entry.name().to_string();

                // Skip the top-level directory
                let parts: Vec<&str> = name.splitn(2, '/').collect();
                if parts.len() < 2 || parts[1].is_empty() {
                    continue;
                }

                let path = java_home.join(parts[1]);
                if entry.is_dir() {
                    std::fs::create_dir_all(&path)?;
                } else {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut outfile = std::fs::File::create(&path)?;
                    std::io::copy(&mut entry, &mut outfile)?;
                }
            }
        }

        // Clean up
        let _ = std::fs::remove_file(&archive_path);
        let _ = std::fs::remove_dir(&temp_dir);

        // Make java executable on Unix
        #[cfg(not(target_os = "windows"))]
        {
            let java_exe = self.get_java_executable(major_version);
            let _ = Command::new("chmod")
                .args(["+x", java_exe.to_str().unwrap()])
                .output();
        }

        progress_callback(&format!("Java {} installed!", major_version));

        Ok(java_home)
    }

    /// Ensure Java is available, downloading if necessary
    pub async fn ensure_java(
        &self,
        major_version: u32,
        progress_callback: impl Fn(&str),
    ) -> Result<PathBuf> {
        // First check our managed Java installations
        if self.is_installed(major_version) {
            let java_exe = self.get_java_executable(major_version);
            tracing::info!("Using managed Java {} at {:?}", major_version, java_exe);
            return Ok(java_exe);
        }

        // Check system-installed Java
        if let Some(system_java) = self.find_system_java(major_version) {
            tracing::info!("Using system Java {} at {:?}", major_version, system_java);
            return Ok(system_java);
        }

        // Need to download
        tracing::info!(
            "Java {} not found, downloading from Adoptium...",
            major_version
        );
        self.download(major_version, progress_callback).await?;

        let java_exe = self.get_java_executable(major_version);
        if java_exe.exists() {
            Ok(java_exe)
        } else {
            anyhow::bail!("Java installation failed: executable not found")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_required_version() {
        assert_eq!(JavaManager::get_required_version("1.21.1"), 21);
        assert_eq!(JavaManager::get_required_version("1.20.4"), 17);
        assert_eq!(JavaManager::get_required_version("1.19.2"), 17);
        assert_eq!(JavaManager::get_required_version("1.16.5"), 8);
    }

    #[test]
    fn test_download_url() {
        let manager = JavaManager::new(Path::new("/tmp"));
        let url = manager.get_download_url(17);
        assert!(url.contains("adoptium"));
        assert!(url.contains("/17/"));
    }
}
