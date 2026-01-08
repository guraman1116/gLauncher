//! Mod browser view
//!
//! UI for searching and downloading mods from Modrinth and CurseForge.

use crate::core::instance::Instance;
use crate::core::mods::curseforge::CurseForgeClient;
use crate::core::mods::download::ModDownloader;
use crate::core::mods::modrinth::ModrinthClient;
use crate::core::mods::search::{
    ModSearchResult, ModSource, ModVersion, SearchFilter, SearchResults,
};
use std::path::PathBuf;
use std::sync::Arc;

/// State for the mod browser
#[derive(Default)]
pub struct ModBrowserState {
    /// Search query
    pub query: String,
    /// Selected source platform
    pub source: ModSourceFilter,
    /// Selected game version filter
    pub game_version: String,
    /// Selected loader filter
    pub loader: String,
    /// Current search results
    pub results: Option<SearchResults>,
    /// Currently selected mod for version browsing
    pub selected_mod: Option<ModSearchResult>,
    /// Versions for the selected mod
    pub mod_versions: Option<Vec<ModVersion>>,
    /// Target instance for installation
    pub target_instance: Option<Instance>,
    /// Loading state
    pub loading: bool,
    /// Error message
    pub error: Option<String>,
    /// Download progress (filename, downloaded, total)
    pub download_progress: Option<(String, u64, u64)>,
    /// Success message
    pub success_message: Option<String>,
    /// Flag to signal that search should be triggered
    pub search_requested: bool,
}

/// Source filter for mod search
#[derive(Default, PartialEq, Clone)]
pub enum ModSourceFilter {
    #[default]
    Modrinth,
    CurseForge,
    Both,
}

impl ModBrowserState {
    /// Create a new mod browser state
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset search results
    pub fn clear_results(&mut self) {
        self.results = None;
        self.selected_mod = None;
        self.mod_versions = None;
        self.error = None;
        self.success_message = None;
    }

    /// Set target instance
    pub fn set_target_instance(&mut self, instance: Instance) {
        self.game_version = instance.info.version.clone();
        self.loader = match instance.info.loader {
            crate::core::instance::ModLoader::Fabric => "fabric".to_string(),
            crate::core::instance::ModLoader::Forge => "forge".to_string(),
            crate::core::instance::ModLoader::Quilt => "quilt".to_string(),
            crate::core::instance::ModLoader::NeoForge => "neoforge".to_string(),
            crate::core::instance::ModLoader::Vanilla => String::new(),
        };
        self.target_instance = Some(instance);
    }
}

/// Mod browser UI trait
pub trait ModBrowserView {
    /// Get mutable reference to mod browser state
    fn mod_browser_state(&mut self) -> &mut ModBrowserState;

    /// Get CurseForge API key from config
    fn get_curseforge_api_key(&self) -> String;

    /// Get mods directory for target instance
    fn get_mods_dir(&self) -> Option<PathBuf>;

    /// Show the mod browser UI
    fn show_mod_browser(&mut self, ui: &mut egui::Ui, ctx: &egui::Context);
}

/// Render mod browser UI components
pub fn render_mod_browser(
    ui: &mut egui::Ui,
    state: &mut ModBrowserState,
    curseforge_api_key: &str,
    ctx: &egui::Context,
) {
    // Search bar
    ui.horizontal(|ui| {
        ui.label("🔍");
        let response = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("Search mods...")
                .desired_width(300.0),
        );

        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            // Trigger search
            start_search(state, curseforge_api_key, ctx);
        }

        if ui.button("Search").clicked() {
            start_search(state, curseforge_api_key, ctx);
        }
    });

    ui.add_space(8.0);

    // Filters
    ui.horizontal(|ui| {
        ui.label("Source:");
        ui.selectable_value(&mut state.source, ModSourceFilter::Modrinth, "Modrinth");
        if !curseforge_api_key.is_empty() {
            ui.selectable_value(&mut state.source, ModSourceFilter::CurseForge, "CurseForge");
        } else {
            ui.add_enabled(false, egui::Button::new("CurseForge (No API Key)"));
        }

        ui.separator();

        ui.label("Version:");
        ui.add(
            egui::TextEdit::singleline(&mut state.game_version)
                .hint_text("e.g. 1.20.1")
                .desired_width(80.0),
        );

        ui.label("Loader:");
        egui::ComboBox::from_id_salt("loader_filter")
            .selected_text(if state.loader.is_empty() {
                "Any"
            } else {
                &state.loader
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.loader, String::new(), "Any");
                ui.selectable_value(&mut state.loader, "fabric".to_string(), "Fabric");
                ui.selectable_value(&mut state.loader, "forge".to_string(), "Forge");
                ui.selectable_value(&mut state.loader, "quilt".to_string(), "Quilt");
                ui.selectable_value(&mut state.loader, "neoforge".to_string(), "NeoForge");
            });
    });

    ui.add_space(8.0);
    ui.separator();

    // Status messages
    if let Some(ref error) = state.error {
        ui.colored_label(egui::Color32::RED, format!("❌ {}", error));
    }
    if let Some(ref success) = state.success_message {
        ui.colored_label(egui::Color32::GREEN, format!("✅ {}", success));
    }
    if let Some((ref filename, downloaded, total)) = state.download_progress {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!(
                "Downloading {}: {} / {}",
                filename,
                format_size(downloaded),
                format_size(total)
            ));
        });
    }

    // Loading indicator
    if state.loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Searching...");
        });
        return;
    }

    // Results
    if let Some(ref results) = state.results.clone() {
        ui.label(format!("Found {} mods", results.total_hits));
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for mod_result in &results.hits {
                    render_mod_card(ui, mod_result, state);
                }
            });
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Enter a search query to find mods");
        });
    }
}

/// Render a single mod card
fn render_mod_card(ui: &mut egui::Ui, mod_result: &ModSearchResult, state: &mut ModBrowserState) {
    egui::Frame::none()
        .fill(ui.visuals().extreme_bg_color)
        .rounding(8.0)
        .inner_margin(12.0)
        .outer_margin(egui::Margin::symmetric(0.0, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Icon placeholder
                ui.add_sized([48.0, 48.0], egui::Label::new("📦"));

                ui.vertical(|ui| {
                    // Title and source
                    ui.horizontal(|ui| {
                        ui.heading(&mod_result.name);
                        ui.label(format!("({})", mod_result.source));
                    });

                    // Author and downloads
                    ui.horizontal(|ui| {
                        if !mod_result.author.is_empty() {
                            ui.label(format!("by {}", mod_result.author));
                            ui.label("•");
                        }
                        ui.label(format!("⬇ {}", format_downloads(mod_result.downloads)));
                    });

                    // Description (truncated)
                    let desc = if mod_result.description.len() > 150 {
                        format!("{}...", &mod_result.description[..150])
                    } else {
                        mod_result.description.clone()
                    };
                    ui.label(desc);

                    // Actions
                    ui.horizontal(|ui| {
                        if ui.button("View Versions").clicked() {
                            state.selected_mod = Some(mod_result.clone());
                            // Would trigger version fetch here
                        }
                        if ui.link("Open Page").clicked() {
                            let _ = open::that(&mod_result.page_url);
                        }
                    });
                });
            });
        });
}

/// Start a search operation
fn start_search(state: &mut ModBrowserState, _curseforge_api_key: &str, _ctx: &egui::Context) {
    if state.query.trim().is_empty() {
        state.error = Some("Please enter a search query".to_string());
        return;
    }

    state.loading = true;
    state.error = None;
    state.success_message = None;
    state.search_requested = true;
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format download count for display
fn format_downloads(downloads: u64) -> String {
    if downloads >= 1_000_000 {
        format!("{:.1}M", downloads as f64 / 1_000_000.0)
    } else if downloads >= 1_000 {
        format!("{:.1}K", downloads as f64 / 1_000.0)
    } else {
        downloads.to_string()
    }
}
