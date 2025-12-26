//! Launch module
//!
//! Start Minecraft with proper arguments.

use std::process::Command;

/// Launch configuration
pub struct LaunchConfig {
    pub java_path: String,
    pub min_memory: String,
    pub max_memory: String,
    pub game_dir: std::path::PathBuf,
    pub version: String,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
}

/// Launch Minecraft
pub fn launch(config: &LaunchConfig) -> anyhow::Result<std::process::Child> {
    let mut cmd = Command::new(&config.java_path);

    // JVM arguments
    cmd.arg(format!("-Xms{}", config.min_memory));
    cmd.arg(format!("-Xmx{}", config.max_memory));

    for arg in &config.extra_jvm_args {
        cmd.arg(arg);
    }

    // TODO: Add classpath, main class, and game arguments
    // This is a placeholder - actual implementation requires:
    // 1. Building classpath from libraries
    // 2. Setting native library path
    // 3. Adding authentication arguments
    // 4. Adding game arguments

    cmd.current_dir(&config.game_dir);

    tracing::info!("Launching Minecraft: {:?}", cmd);

    let child = cmd.spawn()?;
    Ok(child)
}
