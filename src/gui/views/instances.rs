//! Instances view
//!
//! Instance list and management UI.

use crate::gui::types::View;
use eframe::egui;

/// Trait for instances view functionality
pub trait InstancesView {
    fn show_instances(&mut self, ui: &mut egui::Ui, ctx: &egui::Context);
    fn show_create_instance_dialog(&mut self, ctx: &egui::Context);
}
