//! GUI module
//!
//! egui-based graphical user interface.

mod app;

use anyhow::Result;

/// Run the GUI application
pub fn run() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("gLauncher"),
        ..Default::default()
    };

    eframe::run_native(
        "gLauncher",
        options,
        Box::new(|cc| Ok(Box::new(app::LauncherApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}
