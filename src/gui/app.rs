//! Main GUI application
//!
//! egui application state and rendering.

use crate::core::auth::{Account, AccountManager, DeviceCodeResponse};
use crate::core::fabric::FabricManager;
use crate::core::forge::ForgeManager;
use crate::core::instance::{Instance, InstanceManager, ModLoader};
use crate::core::launch::{LaunchResult, launch_instance_async};
use crate::core::mods::{ModManager, format_size};
use crate::core::version::{self, VersionManifest, VersionType};
use anyhow::Context;
use eframe::egui;
use std::sync::mpsc;

/// Main launcher application state
pub struct LauncherApp {
    /// Account manager
    account_manager: AccountManager,
    /// Instance manager
    instance_manager: InstanceManager,
    /// Loaded instances
    instances: Vec<Instance>,
    /// Currently selected instance index
    selected_instance: Option<usize>,
    /// Current view
    current_view: View,
    /// Login state
    login_state: LoginState,
    /// Channel for receiving async results
    async_receiver: Option<mpsc::Receiver<AsyncResult>>,
    /// Launch progress message to display
    launch_progress: Option<String>,

    /// Update status
    update_status: Option<UpdateStatus>,

    /// Sender for async tasks
    tx: mpsc::Sender<AsyncResult>,

    /// Error message to display
    error_message: Option<String>,
    /// Success message to display
    success_message: Option<String>,
    /// Show instance creation dialog
    show_create_dialog: bool,
    /// Show instance settings dialog
    show_settings_dialog: bool,
    /// Instance being edited in settings
    settings_instance: Option<Instance>,
    /// New instance form
    new_instance: NewInstanceForm,
    /// Version manifest (cached)
    version_manifest: Option<VersionManifest>,
    /// Loading state
    is_loading: bool,
    /// Status message
    status_message: String,
    /// Offline account username input
    offline_username: String,
}

#[derive(Default)]
struct NewInstanceForm {
    name: String,
    version: String,
    loader: ModLoader,
    loader_version: String,
    available_versions: Vec<String>,
    available_loader_versions: Vec<String>,
    include_snapshots: bool,
    loading_loader_versions: bool,
}

#[derive(Default, PartialEq)]
enum View {
    #[default]
    Instances,
    Settings,
    Accounts,
}

#[derive(Clone)]
enum LoginState {
    Idle,
    WaitingForCode,
    ShowingCode(DeviceCodeData),
    Authenticating,
}

#[derive(Clone)]
struct DeviceCodeData {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u32,
}

impl Default for LoginState {
    fn default() -> Self {
        Self::Idle
    }
}

use crate::core::update::UpdateStatus;

enum AsyncResult {
    DeviceCode(DeviceCodeResponse),
    LoginSuccess(String),
    LoginError(String),
    VersionManifest(VersionManifest),
    LoaderVersions(Vec<String>),
    InstanceCreated(String),
    LaunchProgress(String),
    LaunchSuccess,
    UpdateCheck(UpdateStatus),
    UpdateSuccess(String),
    UpdateError(String),
    Error(String),
}

impl LauncherApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let instance_manager = InstanceManager::new();
        let instances = instance_manager.list().unwrap_or_default();

        let (tx, rx) = mpsc::channel();

        let mut app = Self {
            account_manager: AccountManager::default(),
            instance_manager,
            instances,
            selected_instance: None,
            current_view: View::Instances,
            login_state: LoginState::Idle,
            async_receiver: Some(rx),
            launch_progress: None,
            update_status: None, // Initialize update status
            tx: tx.clone(),      // Add tx field
            error_message: None,
            success_message: None,
            show_create_dialog: false,
            show_settings_dialog: false,
            settings_instance: None,
            new_instance: NewInstanceForm::default(),
            version_manifest: None,
            is_loading: false,
            status_message: "Ready".to_string(),
            offline_username: String::new(),
        };

        // Start update check
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            use crate::core::update::UpdateManager;
            let status = UpdateManager::check_for_updates().await;
            let _ = tx_clone.send(AsyncResult::UpdateCheck(status));
        });

        // Auto-select first instance if available
        if !app.instances.is_empty() {
            app.selected_instance = Some(0);
        }

        app
    }

    fn refresh_instances(&mut self) {
        self.instances = self.instance_manager.list().unwrap_or_default();
        if self
            .selected_instance
            .map_or(false, |i| i >= self.instances.len())
        {
            self.selected_instance = if self.instances.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    fn fetch_versions(&mut self, ctx: &egui::Context) {
        self.is_loading = true;
        self.status_message = "Fetching versions...".to_string();

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match version::fetch_manifest().await {
                    Ok(manifest) => {
                        let _ = tx.send(AsyncResult::VersionManifest(manifest));
                    }
                    Err(e) => {
                        let _ = tx.send(AsyncResult::Error(e.to_string()));
                    }
                }
            });
            ctx.request_repaint();
        });
    }

    fn create_instance(&mut self, ctx: &egui::Context) {
        let name = self.new_instance.name.clone();
        let version = self.new_instance.version.clone();
        let loader = self.new_instance.loader.clone();
        let loader_version = if self.new_instance.loader_version.is_empty() {
            None
        } else {
            Some(self.new_instance.loader_version.clone())
        };

        if name.is_empty() || version.is_empty() {
            self.error_message = Some("Please fill in all fields".to_string());
            return;
        }

        self.is_loading = true;
        self.status_message = format!("Creating instance {}...", name);

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();
        let instance_manager = InstanceManager::new();

        std::thread::spawn(move || {
            match instance_manager.create(&name, &version, loader, loader_version) {
                Ok(_) => {
                    let _ = tx.send(AsyncResult::InstanceCreated(name));
                }
                Err(e) => {
                    let _ = tx.send(AsyncResult::Error(e.to_string()));
                }
            }
            ctx.request_repaint();
        });
    }

    fn fetch_loader_versions(&mut self, ctx: &egui::Context) {
        let loader = self.new_instance.loader.clone();
        let mc_version = self.new_instance.version.clone();

        if loader == ModLoader::Vanilla || mc_version.is_empty() {
            return;
        }

        self.new_instance.loading_loader_versions = true;

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let versions = match loader {
                    ModLoader::Fabric => {
                        // Fetch Fabric loader versions
                        match FabricManager::get_loader_versions().await {
                            Ok(loaders) => loaders
                                .into_iter()
                                .filter(|l| l.stable)
                                .take(20)
                                .map(|l| l.version)
                                .collect::<Vec<_>>(),
                            Err(e) => {
                                let _ = tx.send(AsyncResult::Error(e.to_string()));
                                return;
                            }
                        }
                    }
                    ModLoader::Forge => {
                        // Fetch Forge versions for this MC version
                        let data_dir = crate::config::config_dir();
                        let java_path = std::path::PathBuf::from("java"); // Dummy, not used for fetch
                        let forge_manager = ForgeManager::new(&data_dir, &java_path);
                        match forge_manager.get_versions_for_mc(&mc_version).await {
                            Ok(versions) => versions
                                .into_iter()
                                .take(20)
                                .map(|v| v.forge_version)
                                .collect::<Vec<_>>(),
                            Err(e) => {
                                let _ = tx.send(AsyncResult::Error(e.to_string()));
                                return;
                            }
                        }
                    }
                    _ => vec![],
                };
                let _ = tx.send(AsyncResult::LoaderVersions(versions));
            });
            ctx.request_repaint();
        });
    }

    fn start_launch(&mut self, instance: Instance, ctx: &egui::Context) {
        println!("=== START_LAUNCH CALLED ===");
        println!("Instance: {} {}", instance.info.name, instance.info.version);

        self.is_loading = true;
        self.status_message = format!("Launching {}...", instance.info.name);
        self.error_message = None;

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();
        let account = self.account_manager.active_account().cloned();

        println!("Account: {:?}", account.as_ref().map(|a| &a.profile.name));

        std::thread::spawn(move || {
            println!("=== SPAWN THREAD STARTED ===");
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                println!("=== ASYNC BLOCK STARTED ===");
                if let Err(e) = launch_instance(instance, account, tx.clone()).await {
                    println!("=== LAUNCH ERROR: {} ===", e);
                    let _ = tx.send(AsyncResult::Error(e.to_string()));
                }
            });
            ctx.request_repaint();
        });
    }

    fn start_login(&mut self, ctx: &egui::Context) {
        self.login_state = LoginState::WaitingForCode;
        self.error_message = None;

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let auth = crate::core::auth::MicrosoftAuth::new();
                match auth.start_device_flow().await {
                    Ok(device_code) => {
                        let _ = tx.send(AsyncResult::DeviceCode(device_code));
                    }
                    Err(e) => {
                        let _ = tx.send(AsyncResult::LoginError(e.to_string()));
                    }
                }
            });
            ctx.request_repaint();
        });
    }

    fn continue_login(&mut self, data: &DeviceCodeData, ctx: &egui::Context) {
        self.login_state = LoginState::Authenticating;

        let (tx, rx) = mpsc::channel();
        self.async_receiver = Some(rx);

        let ctx = ctx.clone();
        let device_code = data.device_code.clone();
        let interval = data.interval;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let dc = crate::core::auth::DeviceCodeResponse {
                    device_code,
                    user_code: String::new(),
                    verification_uri: String::new(),
                    expires_in: 900,
                    interval,
                    message: String::new(),
                };

                let mut manager = crate::core::auth::AccountManager::new().unwrap();
                match manager.complete_login(&dc).await {
                    Ok(account) => {
                        let _ = tx.send(AsyncResult::LoginSuccess(account.profile.name));
                    }
                    Err(e) => {
                        let _ = tx.send(AsyncResult::LoginError(e.to_string()));
                    }
                }
            });
            ctx.request_repaint();
        });
    }

    fn check_async_results(&mut self) {
        if let Some(rx) = &self.async_receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    AsyncResult::DeviceCode(dc) => {
                        self.login_state = LoginState::ShowingCode(DeviceCodeData {
                            device_code: dc.device_code,
                            user_code: dc.user_code,
                            verification_uri: dc.verification_uri,
                            interval: dc.interval,
                        });
                    }
                    AsyncResult::LoginSuccess(username) => {
                        self.login_state = LoginState::Idle;
                        self.async_receiver = None;
                        self.account_manager = AccountManager::default();
                        self.success_message = Some(format!("Logged in as {}", username));
                        self.status_message = "Ready".to_string();
                    }
                    AsyncResult::LoginError(e) => {
                        self.login_state = LoginState::Idle;
                        self.async_receiver = None;
                        self.error_message = Some(e);
                        self.status_message = "Ready".to_string();
                    }
                    AsyncResult::VersionManifest(manifest) => {
                        // Update available versions
                        let versions: Vec<String> = manifest
                            .versions
                            .iter()
                            .filter(|v| {
                                v.version_type == VersionType::Release
                                    || (self.new_instance.include_snapshots
                                        && v.version_type == VersionType::Snapshot)
                            })
                            .take(50)
                            .map(|v| v.id.clone())
                            .collect();

                        self.new_instance.available_versions = versions;
                        if self.new_instance.version.is_empty()
                            && !self.new_instance.available_versions.is_empty()
                        {
                            self.new_instance.version =
                                self.new_instance.available_versions[0].clone();
                        }
                        self.version_manifest = Some(manifest);
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                        self.async_receiver = None;
                    }
                    AsyncResult::InstanceCreated(name) => {
                        self.refresh_instances();
                        self.show_create_dialog = false;
                        self.new_instance = NewInstanceForm::default();
                        self.success_message = Some(format!("Created instance: {}", name));
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                        self.async_receiver = None;
                    }
                    AsyncResult::Error(e) => {
                        self.error_message = Some(e);
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                        self.async_receiver = None;
                    }
                    AsyncResult::LaunchProgress(msg) => {
                        self.status_message = msg;
                    }
                    AsyncResult::UpdateCheck(status) => {
                        match &status {
                            UpdateStatus::UpdateAvailable { latest, .. } => {
                                self.success_message =
                                    Some(format!("⭐ New update available: v{}", latest));
                            }
                            UpdateStatus::CheckFailed(e) => {
                                tracing::error!("Update check failed: {}", e);
                            }
                            _ => {}
                        }
                        self.update_status = Some(status);
                    }
                    AsyncResult::UpdateSuccess(version) => {
                        self.success_message =
                            Some(format!("Updated to version {}! Please restart.", version));
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                    }
                    AsyncResult::UpdateError(e) => {
                        self.error_message = Some(format!("Update failed: {}", e));
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                    }
                    AsyncResult::LaunchSuccess => {
                        self.is_loading = false;
                        self.status_message = "Ready".to_string();
                        self.success_message = Some("Minecraft launched!".to_string());
                        self.async_receiver = None;
                    }
                    AsyncResult::LoaderVersions(versions) => {
                        self.new_instance.available_loader_versions = versions;
                        if self.new_instance.loader_version.is_empty()
                            && !self.new_instance.available_loader_versions.is_empty()
                        {
                            self.new_instance.loader_version =
                                self.new_instance.available_loader_versions[0].clone();
                        }
                        self.new_instance.loading_loader_versions = false;
                    }
                }
            }
        }
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for async results
        self.check_async_results();

        // Clear old messages
        if self.success_message.is_some() {
            // Auto-clear after some time (simplified: just clear on next frame)
        }

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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(account) = self.account_manager.active_account() {
                        ui.label(format!("👤 {}", account.profile.name));
                    }
                });
            });
        });

        // Bottom panel - Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(err) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, format!("❌ {}", err));
                    if ui.small_button("✕").clicked() {
                        self.error_message = None;
                    }
                } else if let Some(msg) = &self.success_message {
                    ui.colored_label(egui::Color32::GREEN, format!("✅ {}", msg));
                    if ui.small_button("✕").clicked() {
                        self.success_message = None;
                    }
                } else if self.is_loading {
                    ui.spinner();
                    ui.label(&self.status_message);
                } else {
                    ui.label(&self.status_message);
                }

                if let Some(UpdateStatus::UpdateAvailable { latest, .. }) = &self.update_status {
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        format!("⬆️ v{} available", latest),
                    );
                    if ui.button("Update Now").clicked() {
                        self.is_loading = true;
                        self.status_message = "Updating...".to_string();
                        let tx = self.tx.clone();

                        tokio::spawn(async move {
                            use crate::core::update::UpdateManager;
                            match UpdateManager::update() {
                                Ok(_) => {
                                    let _ =
                                        tx.send(AsyncResult::UpdateSuccess("latest".to_string()));
                                }
                                Err(e) => {
                                    let _ = tx.send(AsyncResult::UpdateError(e.to_string()));
                                }
                            }
                        });
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("v0.1.0");
                });
            });
        });

        // Central panel - Main content
        egui::CentralPanel::default().show(ctx, |ui| match self.current_view {
            View::Instances => self.show_instances(ui, ctx),
            View::Accounts => self.show_accounts(ui, ctx),
            View::Settings => self.show_settings(ui),
        });

        // Instance creation dialog
        if self.show_create_dialog {
            self.show_create_instance_dialog(ctx);
        }

        // Instance settings dialog
        if self.show_settings_dialog {
            self.show_instance_settings_dialog(ctx);
        }

        // Request repaint while waiting
        if self.is_loading
            || matches!(
                self.login_state,
                LoginState::WaitingForCode | LoginState::Authenticating
            )
        {
            ctx.request_repaint();
        }
    }
}

impl LauncherApp {
    fn show_instances(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading("Instances");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕ New").clicked() {
                    self.show_create_dialog = true;
                    if self.version_manifest.is_none() {
                        self.fetch_versions(ctx);
                    }
                }
                if ui.button("🔄").clicked() {
                    self.refresh_instances();
                }
            });
        });
        ui.separator();

        // Check if logged in
        if self.account_manager.active_account().is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label("⚠️ Please login to your Microsoft account first.");
                ui.add_space(10.0);
                if ui.button("Go to Accounts").clicked() {
                    self.current_view = View::Accounts;
                }
            });
            return;
        }

        if self.instances.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label("No instances yet.");
                ui.label("Click '➕ New' to create one.");
            });
            return;
        }

        // Instance list
        let mut selected = self.selected_instance;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, instance) in self.instances.iter().enumerate() {
                let is_selected = selected == Some(i);
                let response = ui.selectable_label(
                    is_selected,
                    format!(
                        "📦 {} - {} {}",
                        instance.info.name, instance.info.version, instance.info.loader
                    ),
                );
                if response.clicked() {
                    selected = Some(i);
                }
            }
        });
        self.selected_instance = selected;

        // Bottom actions
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let can_launch = self.selected_instance.is_some() && !self.is_loading;

                if ui
                    .add_enabled(
                        can_launch,
                        egui::Button::new("▶ Launch").min_size(egui::vec2(100.0, 30.0)),
                    )
                    .clicked()
                {
                    if let Some(i) = self.selected_instance {
                        let instance = self.instances[i].clone();
                        self.start_launch(instance, ctx);
                    }
                }

                if ui
                    .add_enabled(
                        self.selected_instance.is_some(),
                        egui::Button::new("🗑 Delete"),
                    )
                    .clicked()
                {
                    if let Some(i) = self.selected_instance {
                        let name = self.instances[i].info.name.clone();
                        if let Err(e) = self.instance_manager.delete(&name) {
                            self.error_message = Some(e.to_string());
                        } else {
                            self.success_message = Some(format!("Deleted: {}", name));
                            self.refresh_instances();
                        }
                    }
                }

                if ui
                    .add_enabled(
                        self.selected_instance.is_some(),
                        egui::Button::new("⚙️ Settings"),
                    )
                    .clicked()
                {
                    if let Some(i) = self.selected_instance {
                        self.settings_instance = Some(self.instances[i].clone());
                        self.show_settings_dialog = true;
                    }
                }
            });
        });
    }

    fn show_create_instance_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("New Instance")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(300.0);

                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_instance.name);
                });

                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.label("Version:");
                    if self.is_loading && self.new_instance.available_versions.is_empty() {
                        ui.spinner();
                        ui.label("Loading...");
                    } else {
                        let old_version = self.new_instance.version.clone();
                        egui::ComboBox::from_id_salt("version_select")
                            .selected_text(&self.new_instance.version)
                            .show_ui(ui, |ui| {
                                for v in &self.new_instance.available_versions {
                                    ui.selectable_value(
                                        &mut self.new_instance.version,
                                        v.clone(),
                                        v,
                                    );
                                }
                            });
                        // If version changed, reset loader versions
                        if old_version != self.new_instance.version {
                            self.new_instance.available_loader_versions.clear();
                            self.new_instance.loader_version.clear();
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.new_instance.include_snapshots,
                        "Include snapshots",
                    );
                });

                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.label("Loader:");
                    let old_loader = self.new_instance.loader.clone();
                    egui::ComboBox::from_id_salt("loader_select")
                        .selected_text(format!("{}", self.new_instance.loader))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.new_instance.loader,
                                ModLoader::Vanilla,
                                "Vanilla",
                            );
                            ui.selectable_value(
                                &mut self.new_instance.loader,
                                ModLoader::Fabric,
                                "Fabric",
                            );
                            ui.selectable_value(
                                &mut self.new_instance.loader,
                                ModLoader::Forge,
                                "Forge",
                            );
                        });
                    // If loader changed, clear loader versions and trigger fetch
                    if old_loader != self.new_instance.loader {
                        self.new_instance.available_loader_versions.clear();
                        self.new_instance.loader_version.clear();
                    }
                });

                // Loader version selection (only for Fabric/Forge)
                if self.new_instance.loader != ModLoader::Vanilla {
                    ui.horizontal(|ui| {
                        ui.label("Loader Version:");

                        // Fetch loader versions if empty and MC version is selected
                        if self.new_instance.available_loader_versions.is_empty()
                            && !self.new_instance.version.is_empty()
                            && !self.new_instance.loading_loader_versions
                        {
                            self.fetch_loader_versions(ctx);
                        }

                        if self.new_instance.loading_loader_versions {
                            ui.spinner();
                            ui.label("Loading...");
                        } else if self.new_instance.available_loader_versions.is_empty() {
                            ui.label("(select MC version)");
                        } else {
                            egui::ComboBox::from_id_salt("loader_version_select")
                                .selected_text(&self.new_instance.loader_version)
                                .show_ui(ui, |ui| {
                                    for v in &self.new_instance.available_loader_versions {
                                        ui.selectable_value(
                                            &mut self.new_instance.loader_version,
                                            v.clone(),
                                            v,
                                        );
                                    }
                                });
                        }
                    });
                }

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_create_dialog = false;
                        self.new_instance = NewInstanceForm::default();
                    }

                    let can_create = !self.new_instance.name.is_empty()
                        && !self.new_instance.version.is_empty()
                        && !self.is_loading;

                    if ui
                        .add_enabled(can_create, egui::Button::new("Create"))
                        .clicked()
                    {
                        self.create_instance(ctx);
                    }
                });
            });
    }

    fn show_instance_settings_dialog(&mut self, _ctx: &egui::Context) {
        // Take instance to avoid borrow conflicts
        let mut instance = match self.settings_instance.take() {
            Some(i) => i,
            None => return,
        };

        // Track original name for rename
        let original_name = instance.info.name.clone();
        let mut should_close = false;
        let mut save_result: Option<Result<(), String>> = None;

        egui::Window::new(format!("⚙️ {} Settings", instance.info.name))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(_ctx, |ui| {
                ui.set_min_width(400.0);

                ui.heading("Instance Info");
                ui.add_space(5.0);

                // Instance name
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut instance.info.name);
                });

                // MC Version (display only)
                ui.horizontal(|ui| {
                    ui.label("Minecraft:");
                    ui.label(&instance.info.version);
                });

                // Loader type
                ui.horizontal(|ui| {
                    ui.label("Loader:");
                    egui::ComboBox::from_id_salt("settings_loader_select")
                        .selected_text(format!("{}", instance.info.loader))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut instance.info.loader,
                                ModLoader::Vanilla,
                                "Vanilla",
                            );
                            ui.selectable_value(
                                &mut instance.info.loader,
                                ModLoader::Fabric,
                                "Fabric",
                            );
                            ui.selectable_value(
                                &mut instance.info.loader,
                                ModLoader::Forge,
                                "Forge",
                            );
                        });
                });

                // Loader version (only for Fabric/Forge)
                if instance.info.loader != ModLoader::Vanilla {
                    ui.horizontal(|ui| {
                        ui.label("Loader Version:");
                        let mut loader_version =
                            instance.info.loader_version.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut loader_version).changed() {
                            instance.info.loader_version = if loader_version.is_empty() {
                                None
                            } else {
                                Some(loader_version)
                            };
                        }
                    });
                    ui.label("(Leave empty for recommended version)");
                }

                ui.add_space(10.0);
                ui.heading("Memory");
                ui.add_space(5.0);

                let memory_options = ["Auto", "2G", "4G", "6G", "8G", "12G", "16G"];

                ui.horizontal(|ui| {
                    ui.label("Min Memory:");
                    egui::ComboBox::from_id_salt("min_memory")
                        .selected_text(if instance.java.min_memory.is_empty() {
                            "Auto"
                        } else {
                            &instance.java.min_memory
                        })
                        .show_ui(ui, |ui| {
                            for opt in &memory_options {
                                let value = if *opt == "Auto" { "" } else { *opt };
                                ui.selectable_value(
                                    &mut instance.java.min_memory,
                                    value.to_string(),
                                    *opt,
                                );
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Max Memory:");
                    egui::ComboBox::from_id_salt("max_memory")
                        .selected_text(if instance.java.max_memory.is_empty() {
                            "Auto"
                        } else {
                            &instance.java.max_memory
                        })
                        .show_ui(ui, |ui| {
                            for opt in &memory_options {
                                let value = if *opt == "Auto" { "" } else { *opt };
                                ui.selectable_value(
                                    &mut instance.java.max_memory,
                                    value.to_string(),
                                    *opt,
                                );
                            }
                        });
                });

                ui.add_space(10.0);
                ui.heading("Display");
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.label("Resolution:");
                    ui.add(
                        egui::DragValue::new(&mut instance.game.resolution_width)
                            .range(640..=3840)
                            .speed(10),
                    );
                    ui.label("x");
                    ui.add(
                        egui::DragValue::new(&mut instance.game.resolution_height)
                            .range(480..=2160)
                            .speed(10),
                    );
                });

                ui.checkbox(&mut instance.game.fullscreen, "Fullscreen");

                ui.add_space(10.0);
                ui.heading("Mods");
                ui.add_space(5.0);

                // Get mods directory
                let game_dir = self.instance_manager.get_game_dir(&instance.info.name);
                let mods_dir = game_dir.join("mods");
                let mod_manager = ModManager::new(&mods_dir);

                // Open folder button
                if ui.button("📁 Open Mods Folder").clicked() {
                    if let Err(e) = mod_manager.open_folder() {
                        tracing::error!("Failed to open mods folder: {}", e);
                    }
                }

                ui.add_space(5.0);

                // List mods
                match mod_manager.list_mods() {
                    Ok(mods) => {
                        if mods.is_empty() {
                            ui.label("No mods installed");
                        } else {
                            ui.label(format!("{} mod(s) installed", mods.len()));
                            ui.add_space(3.0);

                            egui::ScrollArea::vertical()
                                .max_height(150.0)
                                .show(ui, |ui| {
                                    for mod_info in &mods {
                                        ui.horizontal(|ui| {
                                            // Enable/disable checkbox
                                            let mut enabled = mod_info.enabled;
                                            if ui.checkbox(&mut enabled, "").changed() {
                                                if let Err(e) = mod_manager.toggle_mod(mod_info) {
                                                    tracing::error!("Failed to toggle mod: {}", e);
                                                }
                                            }

                                            // Mod info
                                            ui.label(&mod_info.name);
                                            ui.label(
                                                egui::RichText::new(&mod_info.version)
                                                    .weak()
                                                    .small(),
                                            );
                                            ui.label(
                                                egui::RichText::new(format_size(mod_info.size))
                                                    .weak()
                                                    .small(),
                                            );
                                        });
                                    }
                                });
                        }
                    }
                    Err(e) => {
                        ui.label(format!("Error listing mods: {}", e));
                    }
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }

                    if ui.button("Save").clicked() {
                        save_result = Some(Ok(()));
                    }
                });
            });

        if should_close {
            self.show_settings_dialog = false;
            // Don't put instance back - it's discarded
        } else if save_result.is_some() {
            // Handle rename if name changed
            if original_name != instance.info.name {
                if let Err(e) = self
                    .instance_manager
                    .rename(&original_name, &instance.info.name)
                {
                    self.error_message = Some(format!("Failed to rename: {}", e));
                    self.show_settings_dialog = false;
                    return;
                }
            }
            // Save the instance
            if let Err(e) = self.instance_manager.save(&instance) {
                self.error_message = Some(format!("Failed to save: {}", e));
            } else {
                self.success_message = Some(format!("Settings saved for {}", instance.info.name));
                self.refresh_instances();
            }
            self.show_settings_dialog = false;
        } else {
            // Put instance back
            self.settings_instance = Some(instance);
        }
    }

    fn show_accounts(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Accounts");
        ui.separator();

        let current_state = self.login_state.clone();

        match current_state {
            LoginState::Idle => {
                use crate::core::auth::AccountType;
                let account_info: Vec<_> = self
                    .account_manager
                    .accounts()
                    .iter()
                    .map(|a| (a.profile.name.clone(), a.account_type.clone(), a.is_active))
                    .collect();

                let has_accounts = !account_info.is_empty();

                if !has_accounts {
                    ui.label("No accounts linked.");
                    ui.add_space(10.0);
                } else {
                    for (name, acc_type, is_active) in &account_info {
                        ui.horizontal(|ui| {
                            let type_icon = match acc_type {
                                AccountType::Microsoft => "🔐",
                                AccountType::Offline => "👤",
                            };
                            let label = if *is_active {
                                format!("✓ {} {} (active)", type_icon, name)
                            } else {
                                format!("  {} {}", type_icon, name)
                            };
                            ui.label(label);
                        });
                    }
                    ui.add_space(10.0);
                }

                if ui.button("🔐 Login with Microsoft").clicked() {
                    self.start_login(ctx);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);
                ui.label("Or play offline:");

                ui.horizontal(|ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut self.offline_username);
                    if ui
                        .add_enabled(
                            !self.offline_username.is_empty(),
                            egui::Button::new("➕ Add"),
                        )
                        .clicked()
                    {
                        match self
                            .account_manager
                            .add_offline_account(&self.offline_username)
                        {
                            Ok(account) => {
                                self.success_message = Some(format!(
                                    "Added offline account: {}",
                                    account.profile.name
                                ));
                                self.offline_username.clear();
                            }
                            Err(e) => {
                                self.error_message = Some(e.to_string());
                            }
                        }
                    }
                });

                if has_accounts {
                    ui.add_space(10.0);
                    if ui.button("🚪 Logout All").clicked() {
                        if let Err(e) = self.account_manager.logout_all() {
                            self.error_message = Some(e.to_string());
                        } else {
                            self.success_message = Some("Logged out".to_string());
                        }
                    }
                }
            }

            LoginState::WaitingForCode => {
                ui.label("⏳ Getting login code...");
                ui.spinner();
            }

            LoginState::ShowingCode(ref data) => {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading("Sign in with Microsoft");
                    ui.add_space(10.0);

                    ui.label("1. Open this URL in your browser:");
                    let url = data.verification_uri.clone();
                    if ui.link(&url).clicked() {
                        let _ = open::that(&url);
                    }

                    ui.add_space(10.0);
                    ui.label("2. Enter this code:");
                    let user_code = data.user_code.clone();
                    ui.heading(&user_code);

                    if ui.button("📋 Copy code").clicked() {
                        ui.output_mut(|o| o.copied_text = user_code.clone());
                    }

                    ui.add_space(20.0);
                });

                let data_clone = data.clone();
                ui.vertical_centered(|ui| {
                    if ui.button("I've entered the code ➡").clicked() {
                        self.continue_login(&data_clone, ctx);
                    }

                    if ui.button("Cancel").clicked() {
                        self.login_state = LoginState::Idle;
                        self.async_receiver = None;
                    }
                });
            }

            LoginState::Authenticating => {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label("⏳ Authenticating with Minecraft...");
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label("Please wait...");
                });
            }
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

/// Launch an instance (runs in background thread)
async fn launch_instance(
    instance: Instance,
    account: Option<Account>,
    tx: mpsc::Sender<AsyncResult>,
) -> anyhow::Result<()> {
    println!("=== launch_instance START ===");
    let account = account.context("No account. Please login first.")?;
    println!("Account OK: {}", account.profile.name);

    // Use shared launch logic with progress callback
    let tx_clone = tx.clone();
    match launch_instance_async(&instance, &account, move |msg| {
        let _ = tx_clone.send(AsyncResult::LaunchProgress(msg.to_string()));
    })
    .await
    {
        Ok(LaunchResult::Success(_child)) => {
            tracing::info!("Minecraft process is running");
            let _ = tx.send(AsyncResult::LaunchSuccess);
        }
        Ok(LaunchResult::EarlyExit(code)) => {
            let error_msg = format!(
                "Minecraft exited unexpectedly with code: {:?}\nCheck terminal for details.",
                code
            );
            let _ = tx.send(AsyncResult::Error(format!(
                "Minecraft failed: {}",
                error_msg
            )));
        }
        Err(e) => {
            let _ = tx.send(AsyncResult::Error(e.to_string()));
        }
    }

    Ok(())
}
