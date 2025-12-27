//! GUI module
//!
//! egui-based graphical user interface.

mod app;

use anyhow::Result;

/// Run the GUI application
/// Run the GUI application
pub fn run() -> Result<()> {
    // Load embedded icon
    let icon_data = include_bytes!("../../assets/icons/app_icon.png");
    let icon = load_icon(icon_data);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1024.0, 768.0])
        .with_min_inner_size([800.0, 600.0])
        .with_title("gLauncher");

    if let Ok(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "gLauncher",
        options,
        Box::new(|cc| Ok(Box::new(app::LauncherApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}

fn load_icon(data: &[u8]) -> Result<egui::IconData> {
    let image = image::load_from_memory(data)?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}
