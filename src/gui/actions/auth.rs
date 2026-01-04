//! Authentication actions
//!
//! Login and authentication related operations.

use crate::gui::types::DeviceCodeData;
use eframe::egui;

/// Trait for authentication actions
pub trait AuthActions {
    fn start_login(&mut self, ctx: &egui::Context);
    fn continue_login(&mut self, data: &DeviceCodeData, ctx: &egui::Context);
}
