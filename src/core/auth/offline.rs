//! Offline authentication
//!
//! For playing without Microsoft account (offline mode).

use super::microsoft::MinecraftProfile;
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

    /// Create an offline profile
    pub fn create_profile(&self) -> Result<MinecraftProfile> {
        // Generate a fake UUID for offline mode (based on username hash)
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(format!("OfflinePlayer:{}", self.username));
        let hash = hasher.finalize();
        let uuid = format!("{:x}", hash)[..32].to_string();

        Ok(MinecraftProfile {
            id: uuid,
            name: self.username.clone(),
        })
    }
}
