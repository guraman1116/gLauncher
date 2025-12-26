//! Offline authentication
//!
//! For playing without Microsoft account.

use super::{AuthResult, Authenticator};
use anyhow::Result;

/// Offline authenticator (no real authentication)
pub struct OfflineAuth {
    username: String,
}

impl OfflineAuth {
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
        }
    }
}

impl Authenticator for OfflineAuth {
    fn authenticate(&self) -> Result<AuthResult> {
        // Generate a fake UUID for offline mode
        let uuid = format!("offline-{}", self.username);

        Ok(AuthResult {
            username: self.username.clone(),
            uuid,
            access_token: String::new(),
        })
    }

    fn refresh(&self) -> Result<AuthResult> {
        self.authenticate()
    }

    fn logout(&self) -> Result<()> {
        Ok(())
    }
}
