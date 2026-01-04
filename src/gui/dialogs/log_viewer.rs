//! Log viewer dialog
//!
//! Real-time log viewing window.

use eframe::egui;

/// Trait for log viewer functionality
pub trait LogViewerDialog {
    fn show_log_viewer_window(&mut self, ctx: &egui::Context);
}

/// Cleanup old log files, keeping only the most recent `max_files` per instance
pub fn cleanup_old_logs(log_dir: &std::path::Path, instance_name: &str, max_files: usize) {
    use std::fs;

    let prefix = format!("{}_", instance_name);

    // Collect log files for this instance
    let mut log_files: Vec<_> = match fs::read_dir(log_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().starts_with(&prefix)
                    && e.path()
                        .extension()
                        .map(|ext| ext == "log")
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => return,
    };

    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| {
        let time_a = a.metadata().and_then(|m| m.modified()).ok();
        let time_b = b.metadata().and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    // Delete files beyond max_files (keep max_files - 1 since we're about to create a new one)
    let keep_count = max_files.saturating_sub(1);
    for file in log_files.into_iter().skip(keep_count) {
        let path = file.path();
        if let Err(e) = fs::remove_file(&path) {
            tracing::warn!("Failed to delete old log file {:?}: {}", path, e);
        } else {
            tracing::debug!("Deleted old log file: {:?}", path);
        }
    }
}
