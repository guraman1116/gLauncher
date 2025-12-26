//! Microsoft authentication
//!
//! OAuth 2.0 flow for Microsoft accounts.

use super::{AuthResult, Authenticator};
use anyhow::Result;

/// Microsoft authenticator
pub struct MicrosoftAuth {
    // TODO: Add OAuth client credentials
}

impl MicrosoftAuth {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for MicrosoftAuth {
    fn default() -> Self {
        Self::new()
    }
}

impl Authenticator for MicrosoftAuth {
    fn authenticate(&self) -> Result<AuthResult> {
        // TODO: Implement Microsoft OAuth flow
        // 1. Open browser for Microsoft login
        // 2. Exchange code for token
        // 3. Get Xbox Live token
        // 4. Get Minecraft token
        // 5. Get Minecraft profile

        anyhow::bail!("Microsoft authentication not yet implemented")
    }

    fn refresh(&self) -> Result<AuthResult> {
        // TODO: Refresh tokens
        anyhow::bail!("Token refresh not yet implemented")
    }

    fn logout(&self) -> Result<()> {
        // TODO: Clear stored tokens
        Ok(())
    }
}
