//! Accounts view
//!
//! Account management UI.

use eframe::egui;

/// Trait for accounts view functionality
pub trait AccountsView {
    fn show_accounts(&mut self, ui: &mut egui::Ui, ctx: &egui::Context);
}
