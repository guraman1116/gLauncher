//! Launch module
//!
//! Start Minecraft with proper arguments.

use crate::config;
use crate::core::asset::AssetManager;
use crate::core::auth::Account;
use crate::core::instance::{Instance, InstanceManager};
use crate::core::library::LibraryManager;
use crate::core::version::VersionDetails;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Minecraft launcher
pub struct Launcher {
    java_path: PathBuf,
    libraries_dir: PathBuf,
    assets_dir: PathBuf,
    versions_dir: PathBuf,
}

impl Launcher {
    pub fn new() -> Self {
        let data_dir = config::config_dir();

        // Auto-detect Java path
        let java_path = Self::find_java().unwrap_or_else(|| PathBuf::from("java"));

        Self {
            java_path,
            libraries_dir: data_dir.join("libraries"),
            assets_dir: data_dir.join("assets"),
            versions_dir: data_dir.join("versions"),
        }
    }

    /// Find Java executable
    fn find_java() -> Option<PathBuf> {
        // Check JAVA_HOME
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            let java = PathBuf::from(&java_home).join("bin").join("java");
            if java.exists() {
                return Some(java);
            }
        }

        // Check common locations on macOS
        #[cfg(target_os = "macos")]
        {
            let paths = [
                "/usr/bin/java",
                "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java",
                "/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home/bin/java",
            ];
            for path in paths {
                let java = PathBuf::from(path);
                if java.exists() {
                    return Some(java);
                }
            }
        }

        // Use 'java' from PATH
        Some(PathBuf::from("java"))
    }

    /// Get version JAR path
    pub fn get_version_jar(&self, version_id: &str) -> PathBuf {
        self.versions_dir
            .join(version_id)
            .join(format!("{}.jar", version_id))
    }

    /// Download version JAR if missing
    pub async fn ensure_version_jar(&self, details: &VersionDetails) -> Result<PathBuf> {
        let jar_path = self.get_version_jar(&details.id);

        if jar_path.exists() {
            return Ok(jar_path);
        }

        let client = details
            .downloads
            .client
            .as_ref()
            .context("No client download info")?;

        tracing::info!("Downloading Minecraft {}", details.id);

        if let Some(parent) = jar_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let response = reqwest::get(&client.url).await?;
        let bytes = response.bytes().await?;
        std::fs::write(&jar_path, &bytes)?;

        Ok(jar_path)
    }

    /// Build JVM arguments
    fn build_jvm_args(&self, instance: &Instance, natives_dir: &Path) -> Vec<String> {
        let min_mem = if instance.java.min_memory.is_empty() {
            "512M".to_string()
        } else {
            instance.java.min_memory.clone()
        };

        let max_mem = if instance.java.max_memory.is_empty() {
            "2G".to_string()
        } else {
            instance.java.max_memory.clone()
        };

        let mut args = vec![
            format!("-Xms{}", min_mem),
            format!("-Xmx{}", max_mem),
            format!("-Djava.library.path={}", natives_dir.display()),
            "-Dminecraft.launcher.brand=gLauncher".to_string(),
            "-Dminecraft.launcher.version=0.1.0".to_string(),
        ];

        // Add extra JVM args
        args.extend(instance.java.extra_args.clone());

        args
    }

    /// Build game arguments
    fn build_game_args(
        &self,
        instance: &Instance,
        details: &VersionDetails,
        account: &Account,
        game_dir: &Path,
    ) -> Vec<String> {
        // Handle legacy argument format
        if let Some(ref mc_args) = details.minecraft_arguments {
            return self.parse_legacy_args(mc_args, instance, account, game_dir);
        }

        // Modern argument format
        let mut args = Vec::new();

        if let Some(ref arguments) = details.arguments {
            for arg in &arguments.game {
                match arg {
                    crate::core::version::ArgumentValue::Simple(s) => {
                        args.push(
                            self.replace_placeholders(s, instance, details, account, game_dir),
                        );
                    }
                    crate::core::version::ArgumentValue::Conditional(c) => {
                        // Check rules
                        if c.rules.iter().all(|r| r.is_allowed()) {
                            match &c.value {
                                crate::core::version::StringOrVec::Single(s) => {
                                    args.push(self.replace_placeholders(
                                        s, instance, details, account, game_dir,
                                    ));
                                }
                                crate::core::version::StringOrVec::Multiple(v) => {
                                    for s in v {
                                        args.push(self.replace_placeholders(
                                            s, instance, details, account, game_dir,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        args
    }

    /// Parse legacy Minecraft arguments
    fn parse_legacy_args(
        &self,
        args_str: &str,
        instance: &Instance,
        account: &Account,
        game_dir: &Path,
    ) -> Vec<String> {
        args_str
            .split_whitespace()
            .map(|s| {
                s.replace("${auth_player_name}", &account.profile.name)
                    .replace("${version_name}", &instance.info.version)
                    .replace("${game_directory}", &game_dir.display().to_string())
                    .replace("${assets_root}", &self.assets_dir.display().to_string())
                    .replace("${assets_index_name}", &instance.info.version)
                    .replace("${auth_uuid}", &account.profile.id)
                    .replace("${auth_access_token}", &account.mc_access_token)
                    .replace("${user_type}", "msa")
                    .replace("${version_type}", "release")
            })
            .collect()
    }

    /// Replace placeholders in argument
    fn replace_placeholders(
        &self,
        arg: &str,
        instance: &Instance,
        details: &VersionDetails,
        account: &Account,
        game_dir: &Path,
    ) -> String {
        arg.replace("${auth_player_name}", &account.profile.name)
            .replace("${version_name}", &instance.info.version)
            .replace("${game_directory}", &game_dir.display().to_string())
            .replace("${assets_root}", &self.assets_dir.display().to_string())
            .replace("${assets_index_name}", &details.asset_index.id)
            .replace("${auth_uuid}", &account.profile.id)
            .replace("${auth_access_token}", &account.mc_access_token)
            .replace("${user_type}", "msa")
            .replace("${version_type}", &details.version_type)
            .replace("${clientid}", "")
            .replace("${auth_xuid}", "")
    }

    /// Launch Minecraft
    pub fn launch(
        &self,
        instance: &Instance,
        details: &VersionDetails,
        account: &Account,
        classpath: &str,
    ) -> Result<Child> {
        let instance_mgr = InstanceManager::new();
        let game_dir = instance_mgr.get_game_dir(&instance.info.name);
        let natives_dir = instance_mgr.get_natives_dir(&instance.info.name);

        // Ensure game directory exists
        std::fs::create_dir_all(&game_dir)?;

        let mut cmd = Command::new(&self.java_path);

        // JVM arguments
        for arg in self.build_jvm_args(instance, &natives_dir) {
            cmd.arg(arg);
        }

        // Classpath
        cmd.arg("-cp").arg(classpath);

        // Main class
        cmd.arg(&details.main_class);

        // Game arguments
        for arg in self.build_game_args(instance, details, account, &game_dir) {
            cmd.arg(arg);
        }

        // Set working directory
        cmd.current_dir(&game_dir);

        // Redirect output
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        tracing::info!(
            "Launching Minecraft: {} {}",
            instance.info.name,
            instance.info.version
        );
        tracing::debug!("Command: {:?}", cmd);

        let child = cmd.spawn().context("Failed to start Minecraft")?;

        Ok(child)
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}
