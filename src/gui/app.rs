//! Main GUI application
//!
//! egui application state and rendering.

use crate::core::auth::{AccountManager, DeviceCodeResponse};
use crate::core::instance::{Instance, InstanceManager, ModLoader};
use crate::core::version::{self, VersionInfo, VersionManifest, VersionType};
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
    /// Error message to display
    error_message: Option<String>,
    /// Success message to display
    success_message: Option<String>,
    /// Show instance creation dialog
    show_create_dialog: bool,
    /// New instance form
    new_instance: NewInstanceForm,
    /// Version manifest (cached)
    version_manifest: Option<VersionManifest>,
    /// Loading state
    is_loading: bool,
    /// Status message
    status_message: String,
}

#[derive(Default)]
struct NewInstanceForm {
    name: String,
    version: String,
    loader: ModLoader,
    available_versions: Vec<String>,
    include_snapshots: bool,
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

enum AsyncResult {
    DeviceCode(DeviceCodeResponse),
    LoginSuccess(String),
    LoginError(String),
    VersionManifest(VersionManifest),
    InstanceCreated(String),
    Error(String),
}

impl LauncherApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let instance_manager = InstanceManager::new();
        let instances = instance_manager.list().unwrap_or_default();

        let mut app = Self {
            account_manager: AccountManager::default(),
            instance_manager,
            instances,
            selected_instance: None,
            current_view: View::Instances,
            login_state: LoginState::Idle,
            async_receiver: None,
            error_message: None,
            success_message: None,
            show_create_dialog: false,
            new_instance: NewInstanceForm::default(),
            version_manifest: None,
            is_loading: false,
            status_message: "Ready".to_string(),
        };

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
            match instance_manager.create(&name, &version, loader) {
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
                        let instance = &self.instances[i];
                        self.status_message = format!("Launching {}...", instance.info.name);
                        // TODO: Implement actual launch
                        self.success_message = Some(format!(
                            "Launch not yet implemented for: {}",
                            instance.info.name
                        ));
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
                        });
                });

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

    fn show_accounts(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Accounts");
        ui.separator();

        let current_state = self.login_state.clone();

        match current_state {
            LoginState::Idle => {
                let account_info: Vec<_> = self
                    .account_manager
                    .accounts()
                    .iter()
                    .map(|a| (a.profile.name.clone(), a.profile.id.clone(), a.is_active))
                    .collect();

                let has_accounts = !account_info.is_empty();

                if !has_accounts {
                    ui.label("No accounts linked.");
                    ui.add_space(10.0);
                } else {
                    for (name, id, is_active) in &account_info {
                        ui.horizontal(|ui| {
                            let label = if *is_active {
                                format!("✓ {} ({})", name, id)
                            } else {
                                format!("  {} ({})", name, id)
                            };
                            ui.label(label);
                        });
                    }
                    ui.add_space(10.0);
                }

                if ui.button("🔐 Login with Microsoft").clicked() {
                    self.start_login(ctx);
                }

                if has_accounts {
                    ui.add_space(5.0);
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
