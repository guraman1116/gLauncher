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

        // Default to Java 21 (latest LTS)
        let java_path = Self::find_java_for_version(21).unwrap_or_else(|| PathBuf::from("java"));

        Self {
            java_path,
            libraries_dir: data_dir.join("libraries"),
            assets_dir: data_dir.join("assets"),
            versions_dir: data_dir.join("versions"),
        }
    }

    /// Find Java executable for a specific major version
    pub fn find_java_for_version(major_version: u32) -> Option<PathBuf> {
        println!("Looking for Java {} ...", major_version);

        // Check JAVA_HOME first
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            let java = PathBuf::from(&java_home).join("bin").join("java");
            if java.exists() {
                // Check version
                if Self::check_java_version(&java, major_version) {
                    println!("Found Java {} via JAVA_HOME: {:?}", major_version, java);
                    return Some(java);
                }
            }
        }

        // Check common locations on macOS
        #[cfg(target_os = "macos")]
        {
            // Homebrew paths - check specific version first
            let homebrew_paths = [
                format!("/opt/homebrew/opt/openjdk@{}/bin/java", major_version),
                format!("/usr/local/opt/openjdk@{}/bin/java", major_version),
            ];

            for path in homebrew_paths {
                let java = PathBuf::from(&path);
                if java.exists() {
                    println!("Found Java {} at: {:?}", major_version, java);
                    return Some(java);
                }
            }

            // Temurin/Adoptium paths
            let temurin_paths = [
                format!(
                    "/Library/Java/JavaVirtualMachines/temurin-{}.jdk/Contents/Home/bin/java",
                    major_version
                ),
                format!(
                    "/Library/Java/JavaVirtualMachines/adoptopenjdk-{}.jdk/Contents/Home/bin/java",
                    major_version
                ),
            ];

            for path in temurin_paths {
                let java = PathBuf::from(&path);
                if java.exists() {
                    println!("Found Java {} at: {:?}", major_version, java);
                    return Some(java);
                }
            }

            // Fallback: Check all JDKs and find one with matching version
            let jvm_dir = Path::new("/Library/Java/JavaVirtualMachines");
            if jvm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(jvm_dir) {
                    for entry in entries.flatten() {
                        let java = entry.path().join("Contents/Home/bin/java");
                        if java.exists() && Self::check_java_version(&java, major_version) {
                            println!("Found Java {} at: {:?}", major_version, java);
                            return Some(java);
                        }
                    }
                }
            }
        }

        // Fall back to PATH and check version
        let java = PathBuf::from("java");
        if Self::check_java_version(&java, major_version) {
            println!("Using Java {} from PATH", major_version);
            return Some(java);
        }

        println!("Java {} not found!", major_version);
        None
    }

    /// Check if a Java executable is the required version
    fn check_java_version(java_path: &Path, required_major: u32) -> bool {
        let output = std::process::Command::new(java_path)
            .arg("-version")
            .output();

        if let Ok(output) = output {
            let version_str = String::from_utf8_lossy(&output.stderr);
            // Parse version like "openjdk version \"21.0.1\"" or "17.0.17"
            // The major version is usually the first number
            for line in version_str.lines() {
                if line.contains("version") {
                    // Extract numbers
                    let parts: Vec<&str> = line.split(|c: char| !c.is_ascii_digit()).collect();
                    for part in parts {
                        if let Ok(ver) = part.parse::<u32>() {
                            if ver == required_major {
                                return true;
                            }
                            // For Java 9+, first number IS the major version
                            if ver >= 9 {
                                return ver == required_major;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Get Java path for a specific version details
    pub fn get_java_for_version(&self, details: &VersionDetails) -> Result<PathBuf> {
        let required_version = details
            .java_version
            .as_ref()
            .map(|jv| jv.major_version)
            .unwrap_or(8); // Default to Java 8 for old versions

        Self::find_java_for_version(required_version).context(format!(
            "Java {} not found. Please install it:\n  brew install openjdk@{}",
            required_version, required_version
        ))
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
    fn build_jvm_args(
        &self,
        instance: &Instance,
        details: &VersionDetails,
        account: &Account,
        game_dir: &Path,
        _natives_dir: &Path,
    ) -> Vec<String> {
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
            // Don't set java.library.path - let LWJGL extract natives from JAR
            // This avoids path length issues with macOS OpenGL dispatch
            "-Dminecraft.launcher.brand=gLauncher".to_string(),
            "-Dminecraft.launcher.version=0.1.0".to_string(),
        ];

        // macOS requires starting on the first thread for LWJGL/OpenGL
        #[cfg(target_os = "macos")]
        args.push("-XstartOnFirstThread".to_string());

        // Add JVM args from version JSON (important for Forge)
        if let Some(ref arguments) = details.arguments {
            for arg in &arguments.jvm {
                match arg {
                    crate::core::version::ArgumentValue::Simple(s) => {
                        let processed =
                            self.replace_jvm_placeholders(s, instance, details, account, game_dir);
                        // Skip empty args
                        if !processed.is_empty() {
                            args.push(processed);
                        }
                    }
                    crate::core::version::ArgumentValue::Conditional(cond) => {
                        // Check if rules allow this argument
                        if self.check_rules(&cond.rules) {
                            match &cond.value {
                                crate::core::version::StringOrVec::Single(s) => {
                                    let processed = self.replace_jvm_placeholders(
                                        s, instance, details, account, game_dir,
                                    );
                                    if !processed.is_empty() {
                                        args.push(processed);
                                    }
                                }
                                crate::core::version::StringOrVec::Multiple(values) => {
                                    for s in values {
                                        let processed = self.replace_jvm_placeholders(
                                            s, instance, details, account, game_dir,
                                        );
                                        if !processed.is_empty() {
                                            args.push(processed);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add extra JVM args from instance config
        args.extend(instance.java.extra_args.clone());

        args
    }

    /// Replace placeholders in JVM arguments
    fn replace_jvm_placeholders(
        &self,
        arg: &str,
        instance: &Instance,
        details: &VersionDetails,
        _account: &Account,
        _game_dir: &Path,
    ) -> String {
        let classpath_separator = if cfg!(windows) { ";" } else { ":" };

        arg.replace("${launcher_name}", "gLauncher")
            .replace("${launcher_version}", "0.1.0")
            .replace("${version_name}", &instance.info.version)
            .replace(
                "${library_directory}",
                &self.libraries_dir.display().to_string(),
            )
            .replace("${classpath_separator}", classpath_separator)
            .replace(
                "${natives_directory}",
                &InstanceManager::new()
                    .get_natives_dir(&instance.info.name)
                    .display()
                    .to_string(),
            )
            .replace("${version_type}", &details.version_type)
    }

    /// Check if rules allow an argument
    fn check_rules(&self, rules: &[crate::core::version::Rule]) -> bool {
        for rule in rules {
            let action_allow = rule.action == "allow";

            if let Some(ref os) = rule.os {
                let os_match = if let Some(ref os_name) = os.name {
                    #[cfg(target_os = "macos")]
                    let current_os = "osx";
                    #[cfg(target_os = "windows")]
                    let current_os = "windows";
                    #[cfg(target_os = "linux")]
                    let current_os = "linux";

                    os_name == current_os
                } else {
                    true
                };

                if action_allow && !os_match {
                    return false;
                }
                if !action_allow && os_match {
                    return false;
                }
            } else if !action_allow {
                return false;
            }
        }
        true
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
        let mut raw_args = Vec::new();

        if let Some(ref arguments) = details.arguments {
            for arg in &arguments.game {
                match arg {
                    crate::core::version::ArgumentValue::Simple(s) => {
                        raw_args.push(
                            self.replace_placeholders(s, instance, details, account, game_dir),
                        );
                    }
                    crate::core::version::ArgumentValue::Conditional(c) => {
                        // Check rules - skip demo mode and other conditional features
                        let dominated_by_features = c.rules.iter().any(|r| r.features.is_some());
                        if !dominated_by_features && c.rules.iter().all(|r| r.is_allowed()) {
                            match &c.value {
                                crate::core::version::StringOrVec::Single(s) => {
                                    raw_args.push(self.replace_placeholders(
                                        s, instance, details, account, game_dir,
                                    ));
                                }
                                crate::core::version::StringOrVec::Multiple(v) => {
                                    for s in v {
                                        raw_args.push(self.replace_placeholders(
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

        // Filter out empty arguments and their corresponding flags
        let mut args = Vec::new();
        let mut skip_next = false;
        for (i, arg) in raw_args.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }

            // Skip demo mode
            if arg == "--demo" {
                continue;
            }

            // Skip arguments that start with $ (unresolved)
            if arg.contains("${") {
                // Also skip the preceding flag if any
                continue;
            }

            // Check if this is a flag with empty value following
            if arg.starts_with("--") && !arg.contains('=') {
                if let Some(next) = raw_args.get(i + 1) {
                    if next.is_empty() || next.contains("${") {
                        // Skip this flag and its value
                        skip_next = true;
                        continue;
                    }
                }
            }

            // Skip empty values
            if arg.is_empty() {
                continue;
            }

            args.push(arg.clone());
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
            // Resolution - use reasonable defaults
            .replace("${resolution_width}", "854")
            .replace("${resolution_height}", "480")
            // Quick play options - not used
            .replace("${quickPlayPath}", "")
            .replace("${quickPlaySingleplayer}", "")
            .replace("${quickPlayMultiplayer}", "")
            .replace("${quickPlayRealms}", "")
    }

    /// Check if argument should be included (filter out unresolved placeholders and demo)
    fn should_include_arg(&self, arg: &str, prev_arg: Option<&str>) -> bool {
        // Skip if still contains unresolved placeholder
        if arg.contains("${") {
            return false;
        }
        // Skip empty arguments
        if arg.is_empty() {
            return false;
        }
        // Skip demo mode
        if arg == "--demo" {
            return false;
        }
        // Skip argument if previous was a flag that requires a value but value is empty/placeholder
        if let Some(prev) = prev_arg {
            if prev.starts_with("--") && !prev.contains('=') && arg.is_empty() {
                return false;
            }
        }
        true
    }

    /// Launch Minecraft
    pub fn launch(
        &self,
        instance: &Instance,
        details: &VersionDetails,
        account: &Account,
        classpath: &str,
        java_path: &Path,
    ) -> Result<Child> {
        let instance_mgr = InstanceManager::new();
        let game_dir = instance_mgr.get_game_dir(&instance.info.name);
        let natives_dir = instance_mgr.get_natives_dir(&instance.info.name);

        // Ensure game directory exists
        std::fs::create_dir_all(&game_dir)?;

        println!("Using Java: {:?}", java_path);

        // Build the full command as a shell string for proper environment handling
        let mut cmd_parts = Vec::new();
        cmd_parts.push(format!("\"{}\"", java_path.display()));

        // JVM arguments
        for arg in self.build_jvm_args(instance, details, account, &game_dir, &natives_dir) {
            cmd_parts.push(format!("\"{}\"", arg));
        }

        // Classpath
        cmd_parts.push("-cp".to_string());
        cmd_parts.push(format!("\"{}\"", classpath));

        // Main class
        cmd_parts.push(details.main_class.clone());

        // Game arguments
        for arg in self.build_game_args(instance, details, account, &game_dir) {
            cmd_parts.push(format!("\"{}\"", arg));
        }

        let full_cmd = cmd_parts.join(" ");

        println!(
            "=== MINECRAFT LAUNCH COMMAND ===\nJava: {:?}\nMain class: {}\nGame dir: {:?}\nNatives dir: {:?}",
            java_path, details.main_class, game_dir, natives_dir
        );
        println!("Full command: cd {:?} && {}", game_dir, full_cmd);

        // Use shell to execute - this ensures proper environment inheritance
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg(format!("cd \"{}\" && {}", game_dir.display(), full_cmd));

        // Inherit environment and I/O
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        cmd.stdin(Stdio::inherit());

        let child = cmd.spawn().context("Failed to start Minecraft")?;
        tracing::info!("Spawned Minecraft process with PID: {:?}", child.id());

        Ok(child)
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}
