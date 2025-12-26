//! Account manager
//!
//! Manages multiple accounts with secure storage using OS keychain.

use super::microsoft::{Account, MicrosoftAuth};
use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "glauncher";
const KEYRING_USER: &str = "accounts";

/// Stored accounts data
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountsData {
    pub accounts: Vec<Account>,
    pub active_uuid: Option<String>,
}

/// Account manager for handling multiple accounts
pub struct AccountManager {
    auth: MicrosoftAuth,
    data: AccountsData,
}

impl AccountManager {
    /// Create a new account manager and load existing accounts
    pub fn new() -> Result<Self> {
        let data = Self::load_accounts().unwrap_or_default();
        Ok(Self {
            auth: MicrosoftAuth::new(),
            data,
        })
    }

    /// Get the keyring entry for account storage
    fn get_keyring_entry() -> Result<Entry> {
        Entry::new(KEYRING_SERVICE, KEYRING_USER).context("Failed to access keychain")
    }

    /// Load accounts from keyring
    fn load_accounts() -> Result<AccountsData> {
        let entry = Self::get_keyring_entry()?;
        match entry.get_password() {
            Ok(json) => {
                let data: AccountsData =
                    serde_json::from_str(&json).context("Failed to parse stored accounts")?;
                Ok(data)
            }
            Err(keyring::Error::NoEntry) => Ok(AccountsData::default()),
            Err(e) => Err(anyhow::anyhow!("Failed to load accounts: {}", e)),
        }
    }

    /// Save accounts to keyring
    fn save_accounts(&self) -> Result<()> {
        let entry = Self::get_keyring_entry()?;
        let json = serde_json::to_string(&self.data).context("Failed to serialize accounts")?;
        entry
            .set_password(&json)
            .context("Failed to save to keychain")?;
        Ok(())
    }

    /// Get all accounts
    pub fn accounts(&self) -> &[Account] {
        &self.data.accounts
    }

    /// Get the active account
    pub fn active_account(&self) -> Option<&Account> {
        self.data
            .active_uuid
            .as_ref()
            .and_then(|uuid| self.data.accounts.iter().find(|a| a.profile.id == *uuid))
    }

    /// Get active account mutably
    pub fn active_account_mut(&mut self) -> Option<&mut Account> {
        let uuid = self.data.active_uuid.clone();
        uuid.and_then(move |uuid| self.data.accounts.iter_mut().find(|a| a.profile.id == uuid))
    }

    /// Set the active account by UUID
    pub fn set_active(&mut self, uuid: &str) -> Result<()> {
        if !self.data.accounts.iter().any(|a| a.profile.id == uuid) {
            anyhow::bail!("Account not found: {}", uuid);
        }

        // Update is_active flags
        for account in &mut self.data.accounts {
            account.is_active = account.profile.id == uuid;
        }

        self.data.active_uuid = Some(uuid.to_string());
        self.save_accounts()?;
        Ok(())
    }

    /// Start login process - returns device code info for user
    pub async fn start_login(&self) -> Result<super::microsoft::DeviceCodeResponse> {
        self.auth.start_device_flow().await
    }

    /// Complete login after user has authenticated
    pub async fn complete_login(
        &mut self,
        device_code: &super::microsoft::DeviceCodeResponse,
    ) -> Result<Account> {
        let account = self.auth.authenticate(device_code).await?;

        // Check if account already exists
        if let Some(existing) = self
            .data
            .accounts
            .iter_mut()
            .find(|a| a.profile.id == account.profile.id)
        {
            // Update existing account
            existing.ms_refresh_token = account.ms_refresh_token.clone();
            existing.mc_access_token = account.mc_access_token.clone();
            existing.profile = account.profile.clone();
        } else {
            // Add new account
            self.data.accounts.push(account.clone());
        }

        // Set as active
        self.data.active_uuid = Some(account.profile.id.clone());
        for acc in &mut self.data.accounts {
            acc.is_active = acc.profile.id == account.profile.id;
        }

        self.save_accounts()?;
        Ok(account)
    }

    /// Refresh the active account's tokens
    pub async fn refresh_active(&mut self) -> Result<()> {
        let account = self.active_account().context("No active account")?.clone();

        let refreshed = self.auth.refresh_account(&account).await?;

        // Update stored account
        if let Some(acc) = self
            .data
            .accounts
            .iter_mut()
            .find(|a| a.profile.id == refreshed.profile.id)
        {
            acc.ms_refresh_token = refreshed.ms_refresh_token;
            acc.mc_access_token = refreshed.mc_access_token;
            acc.profile = refreshed.profile;
        }

        self.save_accounts()?;
        Ok(())
    }

    /// Remove an account
    pub fn remove_account(&mut self, uuid: &str) -> Result<()> {
        let initial_len = self.data.accounts.len();
        self.data.accounts.retain(|a| a.profile.id != uuid);

        if self.data.accounts.len() == initial_len {
            anyhow::bail!("Account not found: {}", uuid);
        }

        // Update active if removed
        if self.data.active_uuid.as_deref() == Some(uuid) {
            self.data.active_uuid = self.data.accounts.first().map(|a| a.profile.id.clone());
            if let Some(ref uuid) = self.data.active_uuid {
                for acc in &mut self.data.accounts {
                    acc.is_active = acc.profile.id == *uuid;
                }
            }
        }

        self.save_accounts()?;
        Ok(())
    }

    /// Remove all accounts (logout all)
    pub fn logout_all(&mut self) -> Result<()> {
        self.data.accounts.clear();
        self.data.active_uuid = None;

        // Delete from keyring
        if let Ok(entry) = Self::get_keyring_entry() {
            let _ = entry.delete_credential();
        }

        Ok(())
    }

    /// Get account for launching (with valid token)
    pub async fn get_launch_account(&mut self) -> Result<&Account> {
        // Try to refresh if needed
        if let Err(e) = self.refresh_active().await {
            tracing::warn!("Failed to refresh token: {}", e);
        }

        self.active_account()
            .context("No active account. Please login first.")
    }
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            auth: MicrosoftAuth::new(),
            data: AccountsData::default(),
        })
    }
}
