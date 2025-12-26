//! Authentication module
//!
//! Handles Microsoft and offline authentication.

mod microsoft;
mod offline;

pub use microsoft::MicrosoftAuth;
pub use offline::OfflineAuth;

/// Authentication result
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
}

/// Authentication trait
pub trait Authenticator {
    fn authenticate(&self) -> anyhow::Result<AuthResult>;
    fn refresh(&self) -> anyhow::Result<AuthResult>;
    fn logout(&self) -> anyhow::Result<()>;
}
