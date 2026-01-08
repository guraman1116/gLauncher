//! GUI type definitions
//!
//! Common types used across GUI modules.

use crate::core::auth::DeviceCodeResponse;
use crate::core::instance::ModLoader;
use crate::core::mods::search::{ModVersion, SearchResults};
use crate::core::update::UpdateStatus;
use crate::core::version::VersionManifest;

/// Current view in the application
#[derive(Default, PartialEq, Clone)]
pub enum View {
    #[default]
    Instances,
    Mods,
    Settings,
    Accounts,
}

/// Login state machine
#[derive(Clone)]
pub enum LoginState {
    Idle,
    WaitingForCode,
    ShowingCode(DeviceCodeData),
    Authenticating,
}

impl Default for LoginState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Device code data for Microsoft authentication
#[derive(Clone)]
pub struct DeviceCodeData {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u32,
}

/// New instance creation form
#[derive(Default)]
pub struct NewInstanceForm {
    pub name: String,
    pub version: String,
    pub loader: ModLoader,
    pub loader_version: String,
    pub available_versions: Vec<String>,
    pub available_loader_versions: Vec<String>,
    pub include_snapshots: bool,
    pub loading_loader_versions: bool,
}

/// Async result types from background tasks
pub enum AsyncResult {
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
    /// Mod search results from Modrinth/CurseForge
    ModSearchResults(SearchResults),
    /// Mod versions for a selected mod
    ModVersions(Vec<ModVersion>),
    /// Mod download completed successfully
    ModDownloaded(String),
    /// Mod search/download error
    ModError(String),
}
