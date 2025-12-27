//! Game launch runner
//!
//! Shared launch logic for CLI and GUI.

use crate::config;
use crate::core::asset::AssetManager;
use crate::core::auth::Account;
use crate::core::fabric::FabricManager;
use crate::core::forge::ForgeManager;
use crate::core::instance::{Instance, InstanceManager, ModLoader};
use crate::core::java::JavaManager;
use crate::core::launch::Launcher;
use crate::core::library::LibraryManager;
use crate::core::version::{self, ArgumentValue, Artifact, Library, LibraryDownloads};
use anyhow::{Context, Result};
use std::process::Child;

/// Result of game launch
pub enum LaunchResult {
    /// Game started successfully
    Success(Child),
    /// Game exited early with code
    EarlyExit(Option<i32>),
}

/// Prepare and launch an instance
///
/// This is the shared launch logic used by both CLI and GUI.
pub async fn launch_instance_async<F>(
    instance: &Instance,
    account: &Account,
    on_progress: F,
) -> Result<LaunchResult>
where
    F: Fn(&str) + Send + Sync,
{
    println!("=== launch_instance_async START ===");
    println!("Account: {}", account.profile.name);

    on_progress("Fetching version manifest...");

    // Fetch version manifest
    println!("Fetching manifest...");
    let manifest = version::fetch_manifest().await?;
    println!("Manifest fetched, versions: {}", manifest.versions.len());

    let version_info = version::get_version_info(&manifest, &instance.info.version)
        .context(format!("Version {} not found", instance.info.version))?;
    println!("Version info found: {}", version_info.id);

    on_progress("Downloading version details...");

    // Fetch version details
    println!("Fetching version details...");
    let mut details = version::fetch_version_details(version_info).await?;
    println!(
        "Version details fetched. Main class: {}",
        details.main_class
    );

    // Apply Fabric if needed
    if instance.info.loader == ModLoader::Fabric {
        on_progress("Loading Fabric profile...");

        // Get Fabric loader version (use latest stable if not specified)
        let loader_version = if let Some(ref v) = instance.info.loader_version {
            v.clone()
        } else {
            println!("Getting latest Fabric loader...");
            FabricManager::get_latest_loader().await?
        };

        println!("Using Fabric loader: {}", loader_version);

        // Get and merge Fabric profile
        let fabric_profile =
            FabricManager::get_profile(&instance.info.version, &loader_version).await?;
        details = FabricManager::merge_version_details(&details, &fabric_profile);

        println!(
            "Fabric profile merged. New main class: {}",
            details.main_class
        );
        println!("Total libraries after Fabric: {}", details.libraries.len());
    }

    // Apply Forge if needed
    if instance.info.loader == ModLoader::Forge {
        on_progress("Installing Forge...");

        // Get Java path for processor execution
        let data_dir = config::config_dir();
        let java_manager = JavaManager::new(&data_dir);

        // Forge requires Java - use the version required for this MC version
        let required_java = JavaManager::get_required_version(&instance.info.version);
        let java_path = java_manager
            .ensure_java(required_java, |msg| {
                println!("Java: {}", msg);
            })
            .await
            .context("Java is required for Forge. Please install Java first.")?;

        let forge_manager = ForgeManager::new(&data_dir, &java_path);

        // Get Forge version (use recommended if not specified)
        let forge_version = if let Some(ref v) = instance.info.loader_version {
            // Find the version from promotions
            let versions = forge_manager
                .get_versions_for_mc(&instance.info.version)
                .await?;
            versions
                .into_iter()
                .find(|fv| fv.forge_version == *v)
                .context(format!("Forge version {} not found", v))?
        } else {
            println!("Getting recommended Forge version...");
            forge_manager
                .get_recommended(&instance.info.version)
                .await?
                .context(format!(
                    "No recommended Forge version for MC {}",
                    instance.info.version
                ))?
        };

        println!(
            "Using Forge version: {} for MC {}",
            forge_version.forge_version, forge_version.mc_version
        );

        // Install Forge (this downloads, parses, runs processors)
        on_progress(&format!(
            "Installing Forge {}...",
            forge_version.forge_version
        ));

        let forge_json = forge_manager
            .install(&forge_version, &instance.info.version)
            .await?;

        // Merge Forge libraries into details
        for lib in &forge_json.libraries {
            // Convert ForgeLibrary to Library
            let library = Library {
                name: lib.name.clone(),
                downloads: lib.downloads.as_ref().map(|d| LibraryDownloads {
                    artifact: d.artifact.as_ref().map(|a| Artifact {
                        path: a.path.clone(),
                        sha1: a.sha1.clone(),
                        size: a.size,
                        url: a.url.clone(),
                    }),
                    classifiers: None,
                }),
                natives: None,
                rules: None,
                url: lib.url.clone(),
                extract: None,
            };
            details.libraries.push(library);
        }

        // Update main class
        details.main_class = forge_json.main_class.clone();

        // Merge arguments if present
        if let Some(ref forge_args) = forge_json.arguments {
            if let Some(ref mut args) = details.arguments {
                for arg in &forge_args.game {
                    if let Some(s) = arg.as_str() {
                        args.game.push(ArgumentValue::Simple(s.to_string()));
                    }
                }
                for arg in &forge_args.jvm {
                    if let Some(s) = arg.as_str() {
                        args.jvm.push(ArgumentValue::Simple(s.to_string()));
                    }
                }
            }
        }

        println!("Forge installed. New main class: {}", details.main_class);
        println!("Total libraries after Forge: {}", details.libraries.len());
    }

    // Setup directories
    let data_dir = config::config_dir();
    let libraries_dir = data_dir.join("libraries");
    let assets_dir = data_dir.join("assets");
    println!("Data dir: {:?}", data_dir);
    println!("Libraries: {} total", details.libraries.len());

    // Download libraries
    println!("Starting library download...");
    on_progress("Downloading libraries...");
    let lib_manager = LibraryManager::new(&libraries_dir);
    // skip_verification=true: Only check file existence (fast mode for 2nd+ launches)
    lib_manager
        .download_all(&details.libraries, true, |current, total, name| {
            if current % 10 == 0 {
                println!("Library {}/{}: {}", current + 1, total, name);
            }
        })
        .await?;
    println!("Libraries downloaded!");

    // Download assets
    println!("Starting asset download...");
    on_progress("Downloading asset index...");
    let asset_manager = AssetManager::new(&assets_dir);
    let asset_index = asset_manager.download_index(&details.asset_index).await?;
    println!(
        "Asset index downloaded: {} objects",
        asset_index.objects.len()
    );

    on_progress("Downloading assets...");
    asset_manager
        // skip_verification=true: Only check file existence (fast mode for 2nd+ launches)
        .download_all(&asset_index, true, |current, total| {
            if current % 500 == 0 {
                println!("Assets: {}/{}", current, total);
            }
        })
        .await?;
    println!("Assets downloaded!");

    // Download client JAR
    println!("Downloading client JAR...");
    on_progress("Downloading Minecraft...");
    let launcher = Launcher::new();
    let game_jar = launcher.ensure_version_jar(&details).await?;
    println!("Client JAR: {:?}", game_jar);

    // Extract natives
    println!("Extracting natives...");
    on_progress("Extracting native libraries...");
    let instance_manager = InstanceManager::new();
    let natives_dir = instance_manager.get_natives_dir(&instance.info.name);
    lib_manager.extract_natives(&details.libraries, &natives_dir)?;
    println!("Natives extracted to: {:?}", natives_dir);

    // Build classpath
    let classpath = lib_manager.build_classpath(&details.libraries, &game_jar);
    println!("Classpath length: {} chars", classpath.len());

    // Ensure Java is available (download if necessary)
    println!("Checking Java installation...");
    on_progress("Checking Java installation...");
    let java_manager = JavaManager::new(&data_dir);
    let required_java = JavaManager::get_required_version(&instance.info.version);
    println!("Required Java version: {}", required_java);

    let java_path = java_manager
        .ensure_java(required_java, |msg| {
            println!("{}", msg);
        })
        .await?;
    println!("Java path: {:?}", java_path);

    // Launch!
    println!("Starting Minecraft process...");
    on_progress("Starting Minecraft...");

    let mut child = launcher.launch(instance, &details, account, &classpath, &java_path)?;
    println!("Process spawned with PID: {:?}", child.id());

    // Wait a bit and check if process is still running
    std::thread::sleep(std::time::Duration::from_secs(2));

    match child.try_wait() {
        Ok(Some(status)) => {
            // Process exited early - this usually means an error
            tracing::error!("Minecraft exited with status: {:?}", status);
            Ok(LaunchResult::EarlyExit(status.code()))
        }
        Ok(None) => {
            // Still running
            tracing::info!("Minecraft process is running");
            Ok(LaunchResult::Success(child))
        }
        Err(e) => {
            anyhow::bail!("Failed to check process: {}", e)
        }
    }
}
