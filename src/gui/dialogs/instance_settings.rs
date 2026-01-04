//! Instance settings dialog
//!
//! Per-instance configuration dialog.

use eframe::egui;

/// Trait for instance settings dialog
pub trait InstanceSettingsDialog {
    fn show_instance_settings_dialog(&mut self, ctx: &egui::Context);
}
