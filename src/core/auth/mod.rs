//! Authentication module
//!
//! Handles Microsoft and offline authentication.

mod manager;
pub mod microsoft;
mod offline;

pub use manager::AccountManager;
pub use microsoft::{Account, AccountType, DeviceCodeResponse, MicrosoftAuth, MinecraftProfile};
pub use offline::OfflineAuth;
