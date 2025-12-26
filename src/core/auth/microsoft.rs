//! Microsoft authentication
//!
//! OAuth 2.0 Device Code flow for Microsoft accounts.
//! Flow: Microsoft Login -> Xbox Live -> XSTS -> Minecraft -> Profile

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Microsoft OAuth endpoints
const MS_DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

/// Xbox Live endpoints
const XBOX_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

/// Minecraft endpoints  
const MC_AUTH_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Azure AD Client ID for Minecraft
/// This is the public client ID used by official launchers
const CLIENT_ID: &str = "00000000402b5328";
const SCOPE: &str = "XboxLive.signin offline_access";

/// Device code response from Microsoft
#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
    pub message: String,
}

/// Token response from Microsoft
#[derive(Debug, Deserialize)]
pub struct MsTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u32,
    pub token_type: String,
}

/// Xbox Live authentication response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveResponse {
    pub token: String,
    pub display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
pub struct DisplayClaims {
    pub xui: Vec<XuiClaim>,
}

#[derive(Debug, Deserialize)]
pub struct XuiClaim {
    pub uhs: String,
}

/// Minecraft authentication response
#[derive(Debug, Deserialize)]
pub struct MinecraftAuthResponse {
    pub access_token: String,
    pub expires_in: u32,
}

/// Minecraft profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

/// Complete account data for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub profile: MinecraftProfile,
    pub ms_refresh_token: String,
    pub mc_access_token: String,
    #[serde(default)]
    pub is_active: bool,
}

/// Microsoft authenticator using Device Code flow
pub struct MicrosoftAuth {
    client: reqwest::Client,
}

impl MicrosoftAuth {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Start the device code authentication flow
    /// Returns the device code response with user instructions
    pub async fn start_device_flow(&self) -> Result<DeviceCodeResponse> {
        let params = [("client_id", CLIENT_ID), ("scope", SCOPE)];

        let response = self
            .client
            .post(MS_DEVICE_CODE_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to request device code")?;

        let device_code: DeviceCodeResponse = response
            .json()
            .await
            .context("Failed to parse device code response")?;

        Ok(device_code)
    }

    /// Poll for the Microsoft token after user has authenticated
    pub async fn poll_for_token(
        &self,
        device_code: &str,
        interval: u32,
    ) -> Result<MsTokenResponse> {
        let params = [
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ];

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(interval as u64)).await;

            let response = self.client.post(MS_TOKEN_URL).form(&params).send().await?;

            let status = response.status();
            let body: serde_json::Value = response.json().await?;

            if status.is_success() {
                let token: MsTokenResponse = serde_json::from_value(body)?;
                return Ok(token);
            }

            // Check for pending or other errors
            if let Some(error) = body.get("error").and_then(|e| e.as_str()) {
                match error {
                    "authorization_pending" => continue,
                    "slow_down" => {
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    "expired_token" => anyhow::bail!("Device code expired. Please try again."),
                    "authorization_declined" => anyhow::bail!("User declined authorization."),
                    _ => anyhow::bail!("Authentication error: {}", error),
                }
            }
        }
    }

    /// Refresh Microsoft token using refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<MsTokenResponse> {
        let params = [
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
            ("scope", SCOPE),
        ];

        let response = self
            .client
            .post(MS_TOKEN_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to refresh token")?;

        let token: MsTokenResponse = response
            .json()
            .await
            .context("Failed to parse refresh token response")?;

        Ok(token)
    }

    /// Authenticate with Xbox Live using Microsoft access token
    pub async fn xbox_live_auth(&self, ms_access_token: &str) -> Result<XboxLiveResponse> {
        let body = serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        });

        let response = self
            .client
            .post(XBOX_AUTH_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to authenticate with Xbox Live")?;

        let xbox_response: XboxLiveResponse = response
            .json()
            .await
            .context("Failed to parse Xbox Live response")?;

        Ok(xbox_response)
    }

    /// Get XSTS token using Xbox Live token
    pub async fn xsts_auth(&self, xbox_token: &str) -> Result<XboxLiveResponse> {
        let body = serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        });

        let response = self
            .client
            .post(XSTS_AUTH_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to get XSTS token")?;

        let status = response.status();

        if !status.is_success() {
            let error: serde_json::Value = response.json().await?;
            if let Some(xerr) = error.get("XErr").and_then(|e| e.as_u64()) {
                match xerr {
                    2148916233 => {
                        anyhow::bail!("This Microsoft account doesn't have an Xbox account.")
                    }
                    2148916235 => anyhow::bail!("Xbox Live is not available in your country."),
                    2148916238 => {
                        anyhow::bail!("This account belongs to a child without Xbox family.")
                    }
                    _ => anyhow::bail!("Xbox authentication error: {}", xerr),
                }
            }
            anyhow::bail!("XSTS authentication failed");
        }

        let xsts_response: XboxLiveResponse = response
            .json()
            .await
            .context("Failed to parse XSTS response")?;

        Ok(xsts_response)
    }

    /// Authenticate with Minecraft using XSTS token
    pub async fn minecraft_auth(
        &self,
        xsts_token: &str,
        user_hash: &str,
    ) -> Result<MinecraftAuthResponse> {
        let body = serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", user_hash, xsts_token)
        });

        let response = self
            .client
            .post(MC_AUTH_URL)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to authenticate with Minecraft")?;

        let mc_response: MinecraftAuthResponse = response
            .json()
            .await
            .context("Failed to parse Minecraft auth response")?;

        Ok(mc_response)
    }

    /// Get Minecraft profile (UUID and username)
    pub async fn get_profile(&self, mc_access_token: &str) -> Result<MinecraftProfile> {
        let response = self
            .client
            .get(MC_PROFILE_URL)
            .header("Authorization", format!("Bearer {}", mc_access_token))
            .send()
            .await
            .context("Failed to get Minecraft profile")?;

        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("This account doesn't own Minecraft Java Edition.");
        }

        if !status.is_success() {
            anyhow::bail!("Failed to get profile: HTTP {}", status);
        }

        let profile: MinecraftProfile = response
            .json()
            .await
            .context("Failed to parse Minecraft profile")?;

        Ok(profile)
    }

    /// Complete authentication flow
    /// Returns Account on success
    pub async fn authenticate(&self, device_code: &DeviceCodeResponse) -> Result<Account> {
        tracing::info!("Waiting for user to authenticate...");

        // Step 1: Poll for Microsoft token
        let ms_token = self
            .poll_for_token(&device_code.device_code, device_code.interval)
            .await?;
        tracing::info!("Microsoft authentication successful");

        // Step 2: Xbox Live authentication
        let xbox_response = self.xbox_live_auth(&ms_token.access_token).await?;
        let user_hash = xbox_response
            .display_claims
            .xui
            .first()
            .map(|x| x.uhs.clone())
            .context("No user hash in Xbox response")?;
        tracing::info!("Xbox Live authentication successful");

        // Step 3: XSTS token
        let xsts_response = self.xsts_auth(&xbox_response.token).await?;
        tracing::info!("XSTS authentication successful");

        // Step 4: Minecraft authentication
        let mc_auth = self
            .minecraft_auth(&xsts_response.token, &user_hash)
            .await?;
        tracing::info!("Minecraft authentication successful");

        // Step 5: Get profile
        let profile = self.get_profile(&mc_auth.access_token).await?;
        tracing::info!("Got Minecraft profile: {} ({})", profile.name, profile.id);

        Ok(Account {
            profile,
            ms_refresh_token: ms_token.refresh_token,
            mc_access_token: mc_auth.access_token,
            is_active: true,
        })
    }

    /// Refresh an existing account
    pub async fn refresh_account(&self, account: &Account) -> Result<Account> {
        // Refresh Microsoft token
        let ms_token = self.refresh_token(&account.ms_refresh_token).await?;

        // Re-authenticate through Xbox and Minecraft
        let xbox_response = self.xbox_live_auth(&ms_token.access_token).await?;
        let user_hash = xbox_response
            .display_claims
            .xui
            .first()
            .map(|x| x.uhs.clone())
            .context("No user hash")?;

        let xsts_response = self.xsts_auth(&xbox_response.token).await?;
        let mc_auth = self
            .minecraft_auth(&xsts_response.token, &user_hash)
            .await?;
        let profile = self.get_profile(&mc_auth.access_token).await?;

        Ok(Account {
            profile,
            ms_refresh_token: ms_token.refresh_token,
            mc_access_token: mc_auth.access_token,
            is_active: account.is_active,
        })
    }
}

impl Default for MicrosoftAuth {
    fn default() -> Self {
        Self::new()
    }
}
