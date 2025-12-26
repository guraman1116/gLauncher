//! Main GUI application
//!
//! egui application state and rendering.

use eframe::egui;

/// Main launcher application state
pub struct LauncherApp {
    /// Currently selected instance
    selected_instance: Option<String>,
    /// Current view
    current_view: View,
}

#[derive(Default, PartialEq)]
enum View {
    #[default]
    Instances,
    Settings,
    Accounts,
}

impl LauncherApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            selected_instance: None,
            current_view: View::Instances,
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel - Header
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🎮 gLauncher");
                ui.separator();

                if ui
                    .selectable_label(self.current_view == View::Instances, "📦 Instances")
                    .clicked()
                {
                    self.current_view = View::Instances;
                }
                if ui
                    .selectable_label(self.current_view == View::Accounts, "👤 Accounts")
                    .clicked()
                {
                    self.current_view = View::Accounts;
                }
                if ui
                    .selectable_label(self.current_view == View::Settings, "⚙️ Settings")
                    .clicked()
                {
                    self.current_view = View::Settings;
                }
            });
        });

        // Bottom panel - Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Ready");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("v0.1.0");
                });
            });
        });

        // Central panel - Main content
        egui::CentralPanel::default().show(ctx, |ui| match self.current_view {
            View::Instances => self.show_instances(ui),
            View::Accounts => self.show_accounts(ui),
            View::Settings => self.show_settings(ui),
        });
    }
}

impl LauncherApp {
    fn show_instances(&mut self, ui: &mut egui::Ui) {
        ui.heading("Instances");
        ui.separator();

        // Add instance button
        if ui.button("➕ New Instance").clicked() {
            // TODO: Open instance creation dialog
        }

        ui.separator();

        // Instance list placeholder
        ui.vertical(|ui| {
            ui.label("No instances yet.");
            ui.label("Click 'New Instance' to create one.");
        });

        // Launch button (disabled until instance selected)
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_enabled_ui(self.selected_instance.is_some(), |ui| {
                if ui.button("▶ Launch").clicked() {
                    // TODO: Launch selected instance
                }
            });
        });
    }

    fn show_accounts(&mut self, ui: &mut egui::Ui) {
        ui.heading("Accounts");
        ui.separator();

        ui.label("No accounts linked.");

        if ui.button("🔐 Login with Microsoft").clicked() {
            // TODO: Start Microsoft OAuth flow
        }
    }

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        ui.collapsing("Java Settings", |ui| {
            ui.label("Java Path: (auto-detect)");
            ui.label("Min Memory: 512M");
            ui.label("Max Memory: 4G");
        });

        ui.collapsing("Launcher Settings", |ui| {
            ui.label("Theme: Dark");
            ui.label("Language: Japanese");
        });
    }
}
