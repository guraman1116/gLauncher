//! Settings view
//!
//! Global settings UI.

use eframe::egui;

/// Trait for settings view functionality
pub trait SettingsView {
    fn show_settings(&mut self, ui: &mut egui::Ui);
}
