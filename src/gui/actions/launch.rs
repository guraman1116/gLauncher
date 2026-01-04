//! Launch actions
//!
//! Game launch related operations.

use crate::core::auth::Account;
use crate::core::instance::Instance;
use crate::core::launch::{LaunchResult, launch_instance_async};
use crate::core::process::SharedProcessManager;
use crate::gui::dialogs::log_viewer::cleanup_old_logs;
use crate::gui::types::AsyncResult;
use anyhow::Context;
use eframe::egui;
use std::sync::mpsc;

/// Trait for launch actions
pub trait LaunchActions {
    fn start_launch(&mut self, instance: Instance, ctx: &egui::Context);
}

/// Launch an instance (runs in background thread)
pub async fn launch_instance(
    instance: Instance,
    account: Option<Account>,
    tx: mpsc::Sender<AsyncResult>,
    process_manager: SharedProcessManager,
) -> anyhow::Result<()> {
    println!("=== launch_instance START ===");
    let account = account.context("No account. Please login first.")?;
    println!("Account OK: {}", account.profile.name);

    let instance_name = instance.info.name.clone();

    // Use shared launch logic with progress callback (GUI captures output for logs)
    let tx_clone = tx.clone();
    match launch_instance_async(
        &instance,
        &account,
        move |msg| {
            let _ = tx_clone.send(AsyncResult::LaunchProgress(msg.to_string()));
        },
        true,
    )
    .await
    {
        Ok(LaunchResult::Success(child)) => {
            tracing::info!("Minecraft process is running, setting up log capture");

            // Create log file path
            let log_dir = crate::config::config_dir().join("logs");
            let _ = std::fs::create_dir_all(&log_dir);

            // Cleanup old log files (keep only 3 most recent per instance)
            cleanup_old_logs(&log_dir, &instance_name, 3);

            let log_file = log_dir.join(format!(
                "{}_{}.log",
                instance_name,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            ));

            // Register process and start log capture
            {
                let running_instance = crate::core::process::RunningInstance::new(
                    instance_name.clone(),
                    child,
                    Some(log_file),
                );

                if let Ok(mut manager) = process_manager.lock() {
                    manager.register(running_instance);
                }
            }

            // Start log capture threads
            if let Ok(mut manager) = process_manager.lock() {
                if let Some(inst) = manager.get_instance_mut(&instance_name) {
                    crate::core::process::start_log_capture(
                        instance_name.clone(),
                        &mut inst.child,
                        std::sync::Arc::clone(&process_manager),
                    );
                }
            }

            let _ = tx.send(AsyncResult::LaunchSuccess);
        }
        Ok(LaunchResult::EarlyExit(code)) => {
            let error_msg = format!(
                "Minecraft exited unexpectedly with code: {:?}\nCheck terminal for details.",
                code
            );
            let _ = tx.send(AsyncResult::Error(format!(
                "Minecraft failed: {}",
                error_msg
            )));
        }
        Err(e) => {
            let _ = tx.send(AsyncResult::Error(e.to_string()));
        }
    }

    Ok(())
}
